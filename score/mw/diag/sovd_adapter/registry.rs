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

use diag_api::sovd::data_resource::DataResourceMetadata;
use diag_api::sovd::DataResource;
use indexmap::IndexMap;
use std::sync::Mutex;

/// Wire format a resource produces and accepts.
///
/// `diag_api` resources choose their own encoding: the UDS adapters in
/// `diag_api::uds` only speak [`PayloadFormat::Binary`], application resources
/// usually speak JSON. The provider asks each resource for its own format.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PayloadFormat {
    /// Values are JSON; the resource can also supply a JSON schema.
    #[default]
    Json,
    /// Values are UTF-8 text, served as a JSON string.
    Utf8,
    /// Values are raw bytes, served as an upper-case hex string (`"0A1B"`).
    Binary,
}

/// Why a resource could not be registered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistrationError {
    /// A resource with this id is already registered.
    DuplicateId(String),
    /// The id is empty.
    EmptyId,
}

impl std::fmt::Display for RegistrationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateId(id) => write!(f, "data resource '{id}' is already registered"),
            Self::EmptyId => write!(f, "data resource id must not be empty"),
        }
    }
}

impl std::error::Error for RegistrationError {}

/// One registered resource.
///
/// `DataResource::write` takes `&mut self` while `DataProvider` methods take
/// `&self`, so the resource sits behind a `Mutex`. The lock is only held while a
/// read/write *handle* is created, never across an `.await`.
pub(crate) struct Entry {
    pub(crate) metadata: DataResourceMetadata,
    pub(crate) format: PayloadFormat,
    pub(crate) resource: Mutex<Box<dyn DataResource + Send>>,
}

/// Owns the `diag_api` resources a gateway component serves, keyed by id.
///
/// `DataResource` cannot enumerate itself, so the registry keeps each resource
/// next to its metadata. Registration order is preserved in listings.
#[derive(Default)]
pub struct DataResourceRegistry {
    entries: IndexMap<String, Entry>,
}

impl DataResourceRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a resource that reads and writes JSON.
    ///
    /// # Errors
    /// [`RegistrationError`] if the id is empty or already taken.
    pub fn register(
        &mut self,
        metadata: DataResourceMetadata,
        resource: impl DataResource + Send + 'static,
    ) -> Result<(), RegistrationError> {
        self.register_with_format(metadata, PayloadFormat::Json, resource)
    }

    /// Register a resource with an explicit payload format, e.g.
    /// [`PayloadFormat::Binary`] for a `diag_api::uds::DataResourceAdapter`.
    ///
    /// # Errors
    /// [`RegistrationError`] if the id is empty or already taken.
    pub fn register_with_format(
        &mut self,
        metadata: DataResourceMetadata,
        format: PayloadFormat,
        resource: impl DataResource + Send + 'static,
    ) -> Result<(), RegistrationError> {
        if metadata.id.is_empty() {
            return Err(RegistrationError::EmptyId);
        }
        if self.entries.contains_key(&metadata.id) {
            return Err(RegistrationError::DuplicateId(metadata.id));
        }
        self.entries.insert(
            metadata.id.clone(),
            Entry {
                metadata,
                format,
                resource: Mutex::new(Box::new(resource)),
            },
        );
        Ok(())
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(crate) fn get(&self, id: &str) -> Option<&Entry> {
        self.entries.get(id)
    }

    pub(crate) fn entries(&self) -> impl Iterator<Item = &Entry> {
        self.entries.values()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use diag_api::sovd::data_resource::{DataCategory, ReadValueArgs, ReadValueHandle};

    struct Dummy;

    impl DataResource for Dummy {
        fn read(&self, _input: ReadValueArgs) -> ReadValueHandle {
            unreachable!("not read in registry tests")
        }
    }

    fn meta(id: &str) -> DataResourceMetadata {
        DataResourceMetadata {
            id: id.to_string(),
            name: id.to_string(),
            translation_id: None,
            read_only: true,
            category: DataCategory::CurrentData,
            groups: None,
        }
    }

    #[test]
    fn register_keeps_insertion_order() {
        let mut registry = DataResourceRegistry::new();
        registry.register(meta("b"), Dummy).unwrap();
        registry.register(meta("a"), Dummy).unwrap();
        let ids: Vec<_> = registry.entries().map(|e| e.metadata.id.as_str()).collect();
        assert_eq!(ids, ["b", "a"]);
        assert_eq!(registry.len(), 2);
    }

    #[test]
    fn register_rejects_duplicate_id() {
        let mut registry = DataResourceRegistry::new();
        registry.register(meta("x"), Dummy).unwrap();
        assert_eq!(
            registry.register(meta("x"), Dummy),
            Err(RegistrationError::DuplicateId("x".to_string()))
        );
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn register_rejects_empty_id() {
        let mut registry = DataResourceRegistry::new();
        assert_eq!(registry.register(meta(""), Dummy), Err(RegistrationError::EmptyId));
        assert!(registry.is_empty());
    }

    #[test]
    fn default_format_is_json() {
        let mut registry = DataResourceRegistry::new();
        registry.register(meta("j"), Dummy).unwrap();
        registry
            .register_with_format(meta("b"), PayloadFormat::Binary, Dummy)
            .unwrap();
        assert_eq!(registry.get("j").unwrap().format, PayloadFormat::Json);
        assert_eq!(registry.get("b").unwrap().format, PayloadFormat::Binary);
        assert!(registry.get("missing").is_none());
    }
}
