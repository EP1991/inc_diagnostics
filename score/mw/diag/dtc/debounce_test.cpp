/********************************************************************************
 * Copyright (c) 2026 Contributors to the Eclipse Foundation
 *
 * See the NOTICE file(s) distributed with this work for additional
 * information regarding copyright ownership.
 *
 * This program and the accompanying materials are made available under the
 * terms of the Apache License Version 2.0 which is available at
 * https://www.apache.org/licenses/LICENSE-2.0
 *
 * SPDX-License-Identifier: Apache-2.0
 ********************************************************************************/

/// @file debounce_test.cpp
/// @brief Unit tests for score/mw/diag/dtc/debounce.h
///        Covers: Debounce::TimeBased IsValid; Debounce::CounterBased IsValid; Debounce GetAlgorithm.

#include "score/mw/diag/dtc/debounce.h"

#include <gtest/gtest.h>

namespace score::mw::diag::dtc
{
namespace
{

// ---------------------------------------------------------------------------
// TimeBased — IsValid
// ---------------------------------------------------------------------------

TEST(TimeBasedTest, IsValidReturnsTrueForPositiveDurations)
{
    EXPECT_TRUE((Debounce::TimeBased{std::chrono::milliseconds{200}, std::chrono::milliseconds{100}}.IsValid()));
}

TEST(TimeBasedTest, IsValidReturnsFalseWhenFailedMsIsZero)
{
    EXPECT_FALSE((Debounce::TimeBased{std::chrono::milliseconds{0}, std::chrono::milliseconds{100}}.IsValid()));
}

TEST(TimeBasedTest, IsValidReturnsFalseWhenPassedMsIsZero)
{
    EXPECT_FALSE((Debounce::TimeBased{std::chrono::milliseconds{200}, std::chrono::milliseconds{0}}.IsValid()));
}

// ---------------------------------------------------------------------------
// CounterBased — IsValid
// ---------------------------------------------------------------------------

TEST(CounterBasedTest, IsValidReturnsTrueForValidConfig)
{
    EXPECT_TRUE((Debounce::CounterBased{10, -5, 2U, 1U, 0, 0, false, false}.IsValid()));
}

TEST(CounterBasedTest, IsValidReturnsFalseWhenFailedThresholdIsZero)
{
    EXPECT_FALSE((Debounce::CounterBased{0, -5, 2U, 1U, 0, 0, false, false}.IsValid()));
}

TEST(CounterBasedTest, IsValidReturnsFalseWhenPassedThresholdIsZero)
{
    EXPECT_FALSE((Debounce::CounterBased{10, 0, 2U, 1U, 0, 0, false, false}.IsValid()));
}

TEST(CounterBasedTest, IsValidReturnsFalseWhenFailedStepsizeIsZero)
{
    EXPECT_FALSE((Debounce::CounterBased{10, -5, 0U, 1U, 0, 0, false, false}.IsValid()));
}

TEST(CounterBasedTest, IsValidReturnsFalseWhenPassedStepsizeIsZero)
{
    EXPECT_FALSE((Debounce::CounterBased{10, -5, 2U, 0U, 0, 0, false, false}.IsValid()));
}

// ---------------------------------------------------------------------------
// Debounce — GetAlgorithm
// ---------------------------------------------------------------------------

TEST(DebounceTest, GetAlgorithmHoldsTimeBasedWhenConstructedWithTimeBased)
{
    const Debounce d{Debounce::TimeBased{std::chrono::milliseconds{200}, std::chrono::milliseconds{100}}};
    ASSERT_TRUE(std::holds_alternative<Debounce::TimeBased>(d.GetAlgorithm()));
    const auto& t = std::get<Debounce::TimeBased>(d.GetAlgorithm());
    EXPECT_EQ(t.failed_duration, std::chrono::milliseconds{200});
    EXPECT_EQ(t.passed_duration, std::chrono::milliseconds{100});
}

TEST(DebounceTest, GetAlgorithmHoldsCounterBasedWhenConstructedWithCounterBased)
{
    const Debounce::CounterBased cfg{10, -5, 2U, 1U, 0, 0, false, false};
    const Debounce d{cfg};
    ASSERT_TRUE(std::holds_alternative<Debounce::CounterBased>(d.GetAlgorithm()));
    const auto& c = std::get<Debounce::CounterBased>(d.GetAlgorithm());
    EXPECT_EQ(c.failed_threshold, 10);
    EXPECT_EQ(c.passed_threshold, -5);
    EXPECT_EQ(c.failed_stepsize, 2U);
    EXPECT_EQ(c.passed_stepsize, 1U);
    EXPECT_EQ(c.failed_jump_value, 0);
    EXPECT_EQ(c.passed_jump_value, 0);
    EXPECT_FALSE(c.use_jump_to_failed);
    EXPECT_FALSE(c.use_jump_to_passed);
}

TEST(DebounceTest, GetAlgorithmHoldsNoneWhenDefault)
{
    EXPECT_TRUE(std::holds_alternative<Debounce::None>(Debounce{}.GetAlgorithm()));
}

}  // namespace
}  // namespace score::mw::diag::dtc
