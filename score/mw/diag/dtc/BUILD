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

load("@rules_cc//cc:defs.bzl", "cc_library", "cc_test")

# ---------------------------------------------------------------------------
# Libraries
# ---------------------------------------------------------------------------

cc_library(
    name = "identifier",
    srcs = ["identifier.cpp"],
    hdrs = ["identifier.h"],
    visibility = ["//score:__subpackages__"],
    deps = ["@score_baselibs//score/language/futurecpp"],
)

cc_library(
    name = "debounce",
    srcs = ["debounce.cpp"],
    hdrs = ["debounce.h"],
    visibility = ["//score:__subpackages__"],
    deps = ["@score_baselibs//score/language/futurecpp"],
)

cc_library(
    name = "dtc",
    srcs = ["dtc.cpp"],
    hdrs = ["dtc.h"],
    visibility = ["//score:__subpackages__"],
    deps = [
        "//score/mw/diag:diag_result",
        "@score_baselibs//score/language/futurecpp",
    ],
)

# ---------------------------------------------------------------------------
# Mock libraries (testonly)
# ---------------------------------------------------------------------------

cc_library(
    name = "mocks",
    testonly = True,
    srcs = ["mocks.cpp"],
    hdrs = [
        "dtc_mock.h",
    ],
    deps = [
        ":dtc",
        "@googletest//:gtest",
    ],
)

# ---------------------------------------------------------------------------
# Tests
# ---------------------------------------------------------------------------

cc_test(
    name = "debounce_test",
    timeout = "short",
    srcs = ["debounce_test.cpp"],
    tags = ["unit"],
    deps = [
        ":debounce",
        "@googletest//:gtest_main",
    ],
)

cc_test(
    name = "identifier_test",
    timeout = "short",
    srcs = ["identifier_test.cpp"],
    tags = ["unit"],
    deps = [
        ":identifier",
        "@googletest//:gtest_main",
        "@score_baselibs//score/language/futurecpp:futurecpp_test_support",
    ],
)

test_suite(
    name = "unit_tests",
    tests = [
        ":debounce_test",
        ":identifier_test",
    ],
    visibility = ["//score/mw/diag:__pkg__"],
)
