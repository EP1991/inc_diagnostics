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

//! Serves S-CORE [`diag_api::sovd::DataResource`]s through the OpenSOVD gateway.
//!
//! [`SovdDataProvider`] implements [`opensovd_core::DataProvider`] on top of a
//! [`DataResourceRegistry`], so a gateway component can expose real diagnostic
//! resources instead of demo constants:
//!
//! ```ignore
//! let mut registry = DataResourceRegistry::new();
//! registry.register(metadata, my_resource)?;
//! let component = Component::new("hvac", "HVAC").with_data_provider(SovdDataProvider::new(registry));
//! ```
//!
//! The adapter is in-process Rust, no FFI. Note that `diag_api` and
//! `opensovd_providers` both define a trait called `DataResource`; this crate only
//! uses the `diag_api` one.

mod convert;
mod handle;
mod provider;
mod registry;

pub use provider::SovdDataProvider;
pub use registry::{DataResourceRegistry, PayloadFormat, RegistrationError};
