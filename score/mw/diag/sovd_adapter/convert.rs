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

//! Conversions between `diag_api` types and `opensovd_core` data types.
//!
//! The two sides are built against different `serde_json` crates (S-CORE's crate
//! index and opensovd-core's own), so their `Value` types are distinct and JSON is
//! converted explicitly at this boundary.

use crate::registry::PayloadFormat;
use diag_api::sovd::data_resource::DataResourceMetadata;
use diag_api::sovd::DataError as DiagDataError;
use diag_api::{ErrorCode, JsonSchemaRequired, ReplyMessageEncoding, ReplyMessagePayload, RequestMessagePayload};
use opensovd_core::{DataError, Metadata};
use serde_json::Value;

pub(crate) fn metadata(meta: &DataResourceMetadata) -> Metadata {
    Metadata {
        id: meta.id.clone(),
        name: meta.name.clone(),
        // Both sides use the ISO 17978-3 spellings: identData, currentData, ...
        category: meta.category.to_string(),
        translation_id: meta.translation_id.clone(),
        groups: meta.groups.clone().unwrap_or_default(),
        // DataResourceMetadata has no tags.
        tags: Vec::new(),
        // A diag_api resource only hands out its schema with a read reply.
        schema: None,
        is_readable: true,
        is_writable: !meta.read_only,
    }
}

pub(crate) fn reply_encoding(format: PayloadFormat, include_schema: bool) -> ReplyMessageEncoding {
    match format {
        PayloadFormat::Json if include_schema => ReplyMessageEncoding::JSON(JsonSchemaRequired::Yes),
        PayloadFormat::Json => ReplyMessageEncoding::JSON(JsonSchemaRequired::No),
        PayloadFormat::Utf8 => ReplyMessageEncoding::UTF8,
        PayloadFormat::Binary => ReplyMessageEncoding::Binary,
    }
}

/// Converts a `diag_api` JSON value into an `opensovd_core` one.
pub(crate) fn to_sovd_json(value: &diag_json::Value) -> Result<Value, DataError> {
    serde_json::from_str(&value.to_string())
        .map_err(|e| DataError::Internal(format!("invalid JSON from resource: {e}")))
}

/// Converts an `opensovd_core` JSON value into a `diag_api` one.
pub(crate) fn to_diag_json(value: &Value) -> Result<diag_json::Value, DataError> {
    diag_json::from_str(&value.to_string()).map_err(|e| DataError::Internal(format!("invalid JSON for resource: {e}")))
}

/// Splits a reply payload into the JSON value and the schema, if one came with it.
pub(crate) fn reply_value(payload: ReplyMessagePayload) -> Result<(Value, Option<Value>), DataError> {
    Ok(match payload {
        ReplyMessagePayload::JSON(value, schema) => {
            (to_sovd_json(&value)?, schema.as_ref().map(to_sovd_json).transpose()?)
        },
        ReplyMessagePayload::UTF8(text) => (Value::String(text), None),
        ReplyMessagePayload::Binary(bytes) => (Value::String(to_hex(&bytes)), None),
    })
}

/// Builds the request payload a resource of `format` expects from a JSON body.
pub(crate) fn request_payload(value: Value, format: PayloadFormat) -> Result<RequestMessagePayload, DataError> {
    match format {
        PayloadFormat::Json => Ok(RequestMessagePayload::JSON(to_diag_json(&value)?)),
        PayloadFormat::Utf8 => match value {
            Value::String(text) => Ok(RequestMessagePayload::UTF8(text)),
            other => Err(DataError::Internal(format!("expected a JSON string, got {other}"))),
        },
        PayloadFormat::Binary => match value {
            Value::String(hex) => from_hex(&hex)
                .map(RequestMessagePayload::Binary)
                .ok_or_else(|| DataError::Internal(format!("expected an even-length hex string, got {hex:?}"))),
            other => Err(DataError::Internal(format!("expected a hex string, got {other}"))),
        },
    }
}

pub(crate) fn read_error(err: diag_api::Error) -> DataError {
    DataError::Internal(match err.code {
        ErrorCode::SOVD(generic) => format!("{}: {}", generic.sovd_error, generic.message_text),
        ErrorCode::UDS(nrc) => format!("UDS negative response 0x{:02X}", u8::from(nrc)),
    })
}

pub(crate) fn write_error(err: DiagDataError) -> DataError {
    DataError::Internal(match err.error {
        Some(generic) => format!("{}: {}", generic.sovd_error, generic.message_text),
        None => format!("write failed at '{}'", err.path),
    })
}

/// Joins the per-element errors of a read reply into one message.
pub(crate) fn reply_errors(errors: &[DiagDataError]) -> String {
    errors
        .iter()
        .map(|e| match &e.error {
            Some(generic) => format!("{} {}: {}", e.path, generic.sovd_error, generic.message_text),
            None => e.path.clone(),
        })
        .collect::<Vec<_>>()
        .join("; ")
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02X}")).collect()
}

fn from_hex(hex: &str) -> Option<Vec<u8>> {
    if hex.len() % 2 != 0 {
        return None;
    }
    (0..hex.len())
        .step_by(2)
        .map(|i| hex.get(i..i + 2).and_then(|pair| u8::from_str_radix(pair, 16).ok()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use diag_api::sovd::data_resource::DataCategory;
    use diag_api::sovd::{ErrorCode as SovdCode, GenericError};
    use diag_api::uds::NegativeResponseCode;
    use serde_json::json;

    #[test]
    fn metadata_maps_every_field() {
        let meta = DataResourceMetadata {
            id: "cabin_temp".to_string(),
            name: "Cabin temperature".to_string(),
            translation_id: Some("t-1".to_string()),
            read_only: false,
            category: DataCategory::CurrentData,
            groups: Some(vec!["hvac".to_string()]),
        };
        let m = metadata(&meta);
        assert_eq!(m.id, "cabin_temp");
        assert_eq!(m.name, "Cabin temperature");
        assert_eq!(m.category, "currentData");
        assert_eq!(m.translation_id.as_deref(), Some("t-1"));
        assert_eq!(m.groups, ["hvac"]);
        assert!(m.tags.is_empty());
        assert!(m.is_readable && m.is_writable);
    }

    #[test]
    fn metadata_read_only_and_no_groups() {
        let meta = DataResourceMetadata {
            id: "vin".to_string(),
            name: "VIN".to_string(),
            translation_id: None,
            read_only: true,
            category: DataCategory::Custom("x-score-demo".to_string()),
            groups: None,
        };
        let m = metadata(&meta);
        assert_eq!(m.category, "x-score-demo");
        assert!(m.groups.is_empty());
        assert!(!m.is_writable);
    }

    #[test]
    fn encoding_follows_format_and_schema_flag() {
        assert_eq!(
            reply_encoding(PayloadFormat::Json, true),
            ReplyMessageEncoding::JSON(JsonSchemaRequired::Yes)
        );
        assert_eq!(
            reply_encoding(PayloadFormat::Json, false),
            ReplyMessageEncoding::JSON(JsonSchemaRequired::No)
        );
        assert_eq!(reply_encoding(PayloadFormat::Utf8, true), ReplyMessageEncoding::UTF8);
        assert_eq!(
            reply_encoding(PayloadFormat::Binary, true),
            ReplyMessageEncoding::Binary
        );
    }

    #[test]
    fn reply_value_per_payload_kind() {
        let schema = json!({"type": "number"});
        assert_eq!(
            reply_value(ReplyMessagePayload::from_json(
                diag_json::json!(21.5),
                Some(diag_json::json!({"type": "number"}))
            ))
            .unwrap(),
            (json!(21.5), Some(schema))
        );
        assert_eq!(
            reply_value(ReplyMessagePayload::from_string("ok".to_string())).unwrap(),
            (json!("ok"), None)
        );
        assert_eq!(
            reply_value(ReplyMessagePayload::from_byte_vector(vec![0x0A, 0xFF])).unwrap(),
            (json!("0AFF"), None)
        );
    }

    #[test]
    fn request_payload_per_format() {
        assert_eq!(
            request_payload(json!({"a": 1}), PayloadFormat::Json).unwrap(),
            RequestMessagePayload::JSON(diag_json::json!({"a": 1}))
        );
        assert_eq!(
            request_payload(json!("hi"), PayloadFormat::Utf8).unwrap(),
            RequestMessagePayload::UTF8("hi".to_string())
        );
        assert_eq!(
            request_payload(json!("0aFF"), PayloadFormat::Binary).unwrap(),
            RequestMessagePayload::Binary(vec![0x0A, 0xFF])
        );
    }

    #[test]
    fn request_payload_rejects_wrong_shapes() {
        assert!(request_payload(json!(1), PayloadFormat::Utf8).is_err());
        assert!(request_payload(json!(1), PayloadFormat::Binary).is_err());
        assert!(request_payload(json!("ABC"), PayloadFormat::Binary).is_err());
        assert!(request_payload(json!("ZZ"), PayloadFormat::Binary).is_err());
    }

    #[test]
    fn read_error_formats_sovd_and_uds() {
        let sovd = diag_api::Error::from_error(GenericError::from_code(
            SovdCode::NotResponding,
            "ECU silent".to_string(),
        ));
        assert!(read_error(sovd).to_string().contains("ECU silent"));
        let uds = diag_api::Error::from_nrc(NegativeResponseCode::RequestOutOfRange);
        assert!(read_error(uds).to_string().contains("0x31"));
    }

    #[test]
    fn write_error_with_and_without_detail() {
        let detailed = DiagDataError::from_error(GenericError::from_code(
            SovdCode::IncompleteRequest,
            "binary only".to_string(),
        ));
        assert!(write_error(detailed).to_string().contains("binary only"));
        assert!(write_error(DiagDataError::new("/value".to_string()))
            .to_string()
            .contains("/value"));
    }
}
