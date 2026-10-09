// *******************************************************************************
// Copyright (c) 2026 Contributors to the Eclipse Foundation
//
// See the NOTICE file(s) distributed with this work for additional
// information regarding copyright ownership.
//
// This program and the accompanying materials are made available under the
// terms of the Apache License Version 2.0 which is available at
// <https://www.apache.org/licenses/LICENSE-2.0>
//
// SPDX-License-Identifier: Apache-2.0
// *******************************************************************************

use crate::registry::{DataResourceRegistry, Entry};
use crate::{convert, handle};
use async_trait::async_trait;
use diag_api::sovd::data_resource::{ReadValueArgs, WriteValueArgs};
use opensovd_core::{Data, DataError, DataFilter, DataProvider, DataScope, Metadata};
use serde_json::Value;

type Result<T> = std::result::Result<T, DataError>;

/// An `opensovd_core::DataProvider` serving the resources of a [`DataResourceRegistry`].
pub struct SovdDataProvider {
    registry: DataResourceRegistry,
}

impl SovdDataProvider {
    #[must_use]
    pub fn new(registry: DataResourceRegistry) -> Self {
        Self { registry }
    }

    fn entry(&self, data_id: &str) -> Result<&Entry> {
        self.registry
            .get(data_id)
            .ok_or_else(|| DataError::NotFound(data_id.to_string()))
    }
}

impl From<DataResourceRegistry> for SovdDataProvider {
    fn from(registry: DataResourceRegistry) -> Self {
        Self::new(registry)
    }
}

fn matches(meta: &Metadata, filter: &DataFilter) -> bool {
    let scope_ok = match &filter.scope {
        None => true,
        Some(DataScope::Categories(categories)) => categories.contains(&meta.category),
        Some(DataScope::Groups(groups)) => groups.iter().any(|g| meta.groups.contains(g)),
    };
    let tag_ok = filter.tags.is_empty() || filter.tags.iter().any(|t| meta.tags.contains(t));
    scope_ok && tag_ok
}

#[async_trait]
impl DataProvider for SovdDataProvider {
    async fn list(&self, filter: DataFilter) -> Result<Vec<Metadata>> {
        Ok(self
            .registry
            .entries()
            .map(|e| convert::metadata(&e.metadata, &e.tags))
            .filter(|m| matches(m, &filter))
            .collect())
    }

    async fn read(&self, data_id: &str, include_schema: bool) -> Result<Data> {
        let entry = self.entry(data_id)?;
        let args = ReadValueArgs::new(convert::reply_encoding(entry.format, include_schema));
        // Create the handle under the lock, await it without.
        let pending = {
            let resource = entry
                .resource
                .lock()
                .map_err(|_| DataError::Internal(format!("resource '{data_id}' is poisoned")))?;
            resource.read(args)
        };
        let reply = handle::resolve_read(pending).await.map_err(convert::read_error)?;

        let (data, schema) = convert::reply_value(reply.data)?;
        // opensovd_core::Data has no error list; any reply with errors is a failure.
        // We cannot return partial success, so fail the entire read if any resource-level
        // errors are present, regardless of whether data was also returned.
        if let Some(errors) = reply.errors.filter(|e| !e.is_empty()) {
            return Err(DataError::Internal(convert::reply_errors(&errors)));
        }
        Ok(Data {
            data,
            schema: schema.filter(|_| include_schema),
        })
    }

    async fn write(&self, data_id: &str, value: Value) -> Result<()> {
        let entry = self.entry(data_id)?;
        if entry.metadata.read_only {
            return Err(DataError::ReadOnly);
        }
        let args = WriteValueArgs {
            user_data: Some(convert::request_payload(value, entry.format)?),
            ..WriteValueArgs::default()
        };
        let pending = {
            let mut resource = entry
                .resource
                .lock()
                .map_err(|_| DataError::Internal(format!("resource '{data_id}' is poisoned")))?;
            resource.write(args)
        };
        handle::resolve_write(pending).await.map_err(convert::write_error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::PayloadFormat;
    use opensovd_core::DataScope;
    use diag_api::sovd::data_resource::{
        DataCategory, DataResourceMetadata, ReadValueHandle, ReadValueReply, WriteValueHandle,
    };
    use diag_api::sovd::{DataError as DiagDataError, DataResource, ErrorCode, GenericError};
    use diag_api::uds::{DataResourceAdapter, ReadDataByIdentifier, WriteDataByIdentifier};
    use diag_api::{JsonSchemaRequired, ReplyMessageEncoding, ReplyMessagePayload, RequestMessagePayload};
    use serde_json::json;
    use std::sync::{Arc, Mutex};

    fn meta(id: &str, category: DataCategory, read_only: bool, groups: &[&str]) -> DataResourceMetadata {
        DataResourceMetadata {
            id: id.to_string(),
            name: id.to_string(),
            translation_id: None,
            read_only,
            category,
            groups: (!groups.is_empty()).then(|| groups.iter().map(|g| (*g).to_string()).collect()),
        }
    }

    /// A JSON resource whose value lives behind a shared cell, answering asynchronously.
    struct Cell(Arc<Mutex<diag_json::Value>>);

    impl DataResource for Cell {
        fn read(&self, input: ReadValueArgs) -> ReadValueHandle {
            let value = self.0.lock().unwrap().clone();
            let schema = (input.reply_encoding == ReplyMessageEncoding::JSON(JsonSchemaRequired::Yes))
                .then(|| diag_json::json!({"type": "number"}));
            ReadValueHandle::from_future(async move {
                Ok(ReadValueReply {
                    data: ReplyMessagePayload::from_json(value, schema),
                    errors: None,
                })
            })
        }

        fn write(&mut self, input: WriteValueArgs) -> WriteValueHandle {
            match input.user_data {
                Some(RequestMessagePayload::JSON(v)) => {
                    *self.0.lock().unwrap() = v;
                    WriteValueHandle::ready()
                },
                _ => WriteValueHandle::from_error(DiagDataError::from_error(GenericError::from_code(
                    ErrorCode::IncompleteRequest,
                    "json only".to_string(),
                ))),
            }
        }
    }

    struct Failing;

    impl DataResource for Failing {
        fn read(&self, _input: ReadValueArgs) -> ReadValueHandle {
            ReadValueHandle::from_error(diag_api::Error::from_error(GenericError::from_code(
                ErrorCode::NotResponding,
                "sensor offline".to_string(),
            )))
        }
    }

    struct Rdbi(Vec<u8>);

    impl ReadDataByIdentifier for Rdbi {
        fn read(&self) -> diag_api::Result<Vec<u8>> {
            Ok(self.0.clone())
        }
    }

    struct Wdbi(Arc<Mutex<Vec<u8>>>);

    impl WriteDataByIdentifier for Wdbi {
        fn write(&mut self, input: &[u8]) -> diag_api::Result<()> {
            *self.0.lock().unwrap() = input.to_vec();
            Ok(())
        }
    }

    fn provider() -> (SovdDataProvider, Arc<Mutex<diag_json::Value>>) {
        let cell = Arc::new(Mutex::new(diag_json::json!(21.5)));
        let mut registry = DataResourceRegistry::new();
        // Register cabin_temp with tags for tag filtering tests
        registry
            .register_with_tags(
                meta("cabin_temp", DataCategory::CurrentData, false, &["hvac"]),
                vec!["sensor".to_string(), "temperature".to_string()],
                Cell(cell.clone()),
            )
            .unwrap();
        registry
            .register(meta("broken", DataCategory::SysInfo, true, &[]), Failing)
            .unwrap();
        (SovdDataProvider::new(registry), cell)
    }

    #[tokio::test]
    async fn list_all_in_registration_order() {
        let (p, _) = provider();
        let ids: Vec<_> = p
            .list(DataFilter::default())
            .await
            .unwrap()
            .into_iter()
            .map(|m| m.id)
            .collect();
        assert_eq!(ids, ["cabin_temp", "broken"]);
    }

    #[tokio::test]
    async fn list_filters_by_category_group_and_tag() {
        let (p, _) = provider();
        let by_cat = DataFilter {
            scope: Some(DataScope::Categories(vec!["sysInfo".to_string()])),
            ..DataFilter::default()
        };
        assert_eq!(p.list(by_cat).await.unwrap()[0].id, "broken");
        let by_group = DataFilter {
            scope: Some(DataScope::Groups(vec!["hvac".to_string()])),
            ..DataFilter::default()
        };
        assert_eq!(p.list(by_group).await.unwrap()[0].id, "cabin_temp");
        // Filter by tag should match cabin_temp which has "sensor" tag
        let by_tag = DataFilter {
            tags: vec!["sensor".to_string()],
            ..DataFilter::default()
        };
        assert_eq!(p.list(by_tag).await.unwrap()[0].id, "cabin_temp");
        // Unknown tag should return empty
        let unknown_tag = DataFilter {
            tags: vec!["unknown".to_string()],
            ..DataFilter::default()
        };
        assert!(p.list(unknown_tag).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn categories_and_groups_come_from_metadata() {
        let (p, _) = provider();
        let cats: Vec<_> = p.categories().await.unwrap().into_iter().map(|c| c.category).collect();
        assert_eq!(cats, ["currentData", "sysInfo"]);
        let groups: Vec<_> = p.groups(None).await.unwrap().into_iter().map(|g| g.id).collect();
        assert_eq!(groups, ["hvac"]);
    }

    #[tokio::test]
    async fn read_value_and_schema_on_request() {
        let (p, _) = provider();
        let plain = p.read("cabin_temp", false).await.unwrap();
        assert_eq!(plain.data, json!(21.5));
        assert!(plain.schema.is_none());
        let with_schema = p.read("cabin_temp", true).await.unwrap();
        assert_eq!(with_schema.schema, Some(json!({"type": "number"})));
    }

    #[tokio::test]
    async fn read_unknown_id_is_not_found() {
        let (p, _) = provider();
        assert!(matches!(p.read("nope", false).await, Err(DataError::NotFound(id)) if id == "nope"));
    }

    #[tokio::test]
    async fn read_error_is_internal_with_message() {
        let (p, _) = provider();
        let err = p.read("broken", false).await.unwrap_err();
        assert!(matches!(&err, DataError::Internal(m) if m.contains("sensor offline")));
    }

    #[tokio::test]
    async fn write_reaches_the_resource() {
        let (p, cell) = provider();
        p.write("cabin_temp", json!(19.0)).await.unwrap();
        assert_eq!(*cell.lock().unwrap(), diag_json::json!(19.0));
        assert_eq!(p.read("cabin_temp", false).await.unwrap().data, json!(19.0));
    }

    #[tokio::test]
    async fn write_read_only_is_rejected_before_the_resource() {
        let (p, _) = provider();
        assert!(matches!(p.write("broken", json!(1)).await, Err(DataError::ReadOnly)));
        assert!(matches!(p.write("nope", json!(1)).await, Err(DataError::NotFound(_))));
    }

    #[tokio::test]
    async fn uds_adapter_served_without_extra_code() {
        let written = Arc::new(Mutex::new(Vec::new()));
        let uds = DataResourceAdapter::new()
            .with_rdbi(Rdbi(vec![0x12, 0xAB]))
            .with_wdbi(Wdbi(written.clone()));
        let mut registry = DataResourceRegistry::new();
        registry
            .register_with_format(
                meta("did_f190", DataCategory::IdentData, false, &[]),
                PayloadFormat::Binary,
                uds,
            )
            .unwrap();
        let p = SovdDataProvider::from(registry);

        assert_eq!(p.read("did_f190", false).await.unwrap().data, json!("12AB"));
        p.write("did_f190", json!("C0FFEE")).await.unwrap();
        assert_eq!(*written.lock().unwrap(), vec![0xC0, 0xFF, 0xEE]);
    }

    #[tokio::test]
    async fn uds_adapter_registered_as_json_reports_the_mismatch() {
        let mut registry = DataResourceRegistry::new();
        registry
            .register(
                meta("did", DataCategory::IdentData, true, &[]),
                DataResourceAdapter::new().with_rdbi(Rdbi(vec![1])),
            )
            .unwrap();
        let err = SovdDataProvider::new(registry).read("did", false).await.unwrap_err();
        assert!(matches!(&err, DataError::Internal(m) if m.contains("binary")));
    }
}
