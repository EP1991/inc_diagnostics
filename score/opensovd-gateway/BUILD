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

load("@rules_rust//rust:defs.bzl", "rust_binary", "rust_test")

_SRCS = [
    "src/cruise.rs",
    "src/main.rs",
]

# tokio comes from opensovd_core's crate index so its TcpListener matches the one
# opensovd_server expects; the diag_api serde_json is renamed to diag_json.
_DEPS = [
    "//score/mw/diag/api",
    "//score/mw/diag/sovd_adapter",
    "@opensovd_core//:opensovd-core",
    "@opensovd_core//:opensovd-server",
    "@opensovd_core//:tokio",
    "@score_crate_index//:serde_json",
]

_ALIASES = {
    "@score_crate_index//:serde_json": "diag_json",
}

rust_binary(
    name = "opensovd-gateway",
    srcs = _SRCS,
    aliases = _ALIASES,
    crate_root = "src/main.rs",
    edition = "2021",
    visibility = ["//visibility:public"],
    deps = _DEPS,
)

rust_test(
    name = "opensovd-gateway_test",
    srcs = _SRCS,
    aliases = _ALIASES,
    crate_root = "src/main.rs",
    edition = "2021",
    deps = _DEPS + [
        "@opensovd_core//:opensovd-client",
        "@opensovd_core//:serde_json",
    ],
)
