# *******************************************************************************
# Copyright (c) 2026 Contributors to the Eclipse Foundation
#
# See the NOTICE file(s) distributed with this work for additional
# information regarding copyright ownership.
#
# This program and the accompanying materials are made available under the
# terms of the Apache License Version 2.0 which is available at
# https://www.apache.org/licenses/LICENSE-2.0
#
# SPDX-License-Identifier: Apache-2.0
# *******************************************************************************

load("@rules_rust//rust:defs.bzl", "rust_library", "rust_test")

# Serves diag_api::sovd::DataResource instances through opensovd_core::DataProvider,
# so the OpenSOVD gateway can expose real S-CORE diagnostic data.
rust_library(
    name = "sovd_adapter",
    srcs = [
        "convert.rs",
        "handle.rs",
        "lib.rs",
        "provider.rs",
        "registry.rs",
    ],
    # diag_api and opensovd_core come from different crate universes, so each
    # side has its own serde_json; the diag_api one is renamed to diag_json.
    aliases = {
        "@score_crate_index//:serde_json": "diag_json",
    },
    crate_root = "lib.rs",
    edition = "2021",
    proc_macro_deps = [
        "@score_crate_index//:async-trait",
    ],
    visibility = ["//score:__subpackages__"],
    deps = [
        "//score/mw/diag/api",
        "@opensovd_core//:opensovd-core",
        "@opensovd_core//:serde_json",
        "@score_crate_index//:indexmap",
        "@score_crate_index//:serde_json",
    ],
)

rust_test(
    name = "sovd_adapter_test",
    crate = ":sovd_adapter",
    edition = "2021",
    deps = [
        "@score_crate_index//:tokio",
    ],
)
