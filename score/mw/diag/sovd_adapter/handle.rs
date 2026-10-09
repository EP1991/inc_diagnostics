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

//! Turns the `diag_api` read/write handles into values an `async fn` can return.
//!
//! A handle is either `Ready` (the resource answered synchronously) or `Pending`
//! (a boxed `Send` future, also used for closures). Both resolve by `.await`.

use diag_api::sovd::data_resource::{ReadValueHandle, ReadValueReply, WriteValueHandle};
use diag_api::sovd::DataError as DiagDataError;
use diag_api::Result as DiagResult;

pub(crate) async fn resolve_read(handle: ReadValueHandle) -> DiagResult<ReadValueReply> {
    match handle {
        ReadValueHandle::Ready(result) => result,
        ReadValueHandle::Pending(future) => future.await,
    }
}

pub(crate) async fn resolve_write(handle: WriteValueHandle) -> Result<(), DiagDataError> {
    match handle {
        WriteValueHandle::Ready(result) => result,
        WriteValueHandle::Pending(future) => future.await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use diag_api::sovd::{ErrorCode, GenericError};
    use diag_api::ReplyMessagePayload;

    fn reply(text: &str) -> ReadValueReply {
        ReadValueReply {
            data: ReplyMessagePayload::from_string(text.to_string()),
            errors: None,
        }
    }

    fn data_error() -> DiagDataError {
        DiagDataError::from_error(GenericError::from_code(ErrorCode::ErrorResponse, "no".to_string()))
    }

    #[tokio::test]
    async fn read_ready() {
        let got = resolve_read(ReadValueHandle::ready(reply("r"))).await.unwrap();
        assert_eq!(got.data, ReplyMessagePayload::from_string("r".to_string()));
    }

    #[tokio::test]
    async fn read_future() {
        let handle = ReadValueHandle::from_future(async { Ok(reply("f")) });
        let got = resolve_read(handle).await.unwrap();
        assert_eq!(got.data, ReplyMessagePayload::from_string("f".to_string()));
    }

    #[tokio::test]
    async fn read_closure() {
        let handle = ReadValueHandle::from_closure(|| Ok(reply("c")));
        let got = resolve_read(handle).await.unwrap();
        assert_eq!(got.data, ReplyMessagePayload::from_string("c".to_string()));
    }

    #[tokio::test]
    async fn read_error() {
        let err = diag_api::Error::from_error(GenericError::from_code(ErrorCode::NotResponding, "x".to_string()));
        assert_eq!(
            resolve_read(ReadValueHandle::from_error(err.clone()))
                .await
                .unwrap_err(),
            err
        );
    }

    #[tokio::test]
    async fn write_ready_future_closure() {
        assert!(resolve_write(WriteValueHandle::ready()).await.is_ok());
        assert!(resolve_write(WriteValueHandle::from_future(async { Ok(()) }))
            .await
            .is_ok());
        assert!(resolve_write(WriteValueHandle::from_closure(|| Err(data_error())))
            .await
            .is_err());
    }
}
