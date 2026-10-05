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

/// @file debounce.h
/// @brief Debouncing configuration types for DTC monitoring.

#ifndef SCORE_MW_DIAG_DTC_DEBOUNCE_H
#define SCORE_MW_DIAG_DTC_DEBOUNCE_H

#include "score/assert.hpp"

#include <chrono>
#include <cstdint>
#include <variant>

namespace score::mw::diag::dtc
{

/// @brief Debouncing configuration for a DTC instance.
///
/// Holds at most one algorithm — time-based (TimeBased) or counter-based (CounterBased).
/// Default-constructed means no middleware debouncing: every TriggerAction() call is passed through.
class Debounce final
{
  public:
    /// @brief Alias for std::monostate — represents pass-through (no debouncing).
    using None = std::monostate;

    /************************************/
    /* Time-based debouncing            */
    /************************************/

    /// @brief Time-based debouncing: fault must persist for @p failed_duration to qualify as Failed,
    ///        or healthy for @p passed_duration to qualify as Passed.
    ///        Both values are application-specific and must be set explicitly.
    struct TimeBased
    {
        std::chrono::milliseconds failed_duration;  ///< Continuous fail duration to qualify as Failed.
        std::chrono::milliseconds passed_duration;  ///< Continuous pass duration to qualify as Passed.

        /// @brief Returns true if both durations are greater than zero.
        /// @return true if failed_duration > 0 and passed_duration > 0; false otherwise.
        [[nodiscard]] constexpr bool IsValid() const noexcept
        {
            return (failed_duration.count() > 0) && (passed_duration.count() > 0);
        }
    };

    /************************************/
    /* Counter-based debouncing         */
    /************************************/

    /// @brief Counter-based debouncing parameters.
    ///        All thresholds, step sizes and jump values are application-specific and must be set explicitly.
    struct CounterBased
    {
        std::int16_t failed_threshold;   ///< Counter threshold to qualify as Failed (must be positive).
        std::int16_t passed_threshold;   ///< Counter threshold to qualify as Passed (must be negative).
        std::uint16_t failed_stepsize;   ///< Counter increment per Failed report.
        std::uint16_t passed_stepsize;   ///< Counter decrement per Passed report.
        std::int16_t failed_jump_value;  ///< Jump value on first Failed (if use_jump_to_failed).
        std::int16_t passed_jump_value;  ///< Jump value on first Passed (if use_jump_to_passed).
        bool use_jump_to_failed;         ///< Apply jump-to-failed on the first Failed report.
        bool use_jump_to_passed;         ///< Apply jump-to-passed on the first Passed report.

        /// @brief Returns true if thresholds and step sizes are semantically valid.
        /// @return true if failed_threshold > 0, passed_threshold < 0, and both step sizes > 0.
        [[nodiscard]] constexpr bool IsValid() const noexcept
        {
            return (failed_threshold > 0) && (passed_threshold < 0) && (failed_stepsize > 0U) && (passed_stepsize > 0U);
        }
    };

    /************************************/
    /* Algorithm variant                */
    /************************************/

    /// @brief Selects the debouncing algorithm — pass-through (none), time-based, or counter-based.
    ///        None (std::monostate) = pass-through: the middleware applies no algorithm and forwards
    ///        every TriggerAction() call as-is.
    using Algorithm = std::variant<None, TimeBased, CounterBased>;

    /************************************/
    /* Construction                     */
    /************************************/

    /// @brief Default: no debouncing — every TriggerAction() call is passed through; IsSet() returns false.
    constexpr Debounce() noexcept = default;

    /// @brief Configure time-based debouncing.
    /// @param timer Time-based algorithm parameters; must satisfy TimeBased::IsValid().
    constexpr Debounce(TimeBased timer) noexcept : algorithm_{std::in_place_type<TimeBased>, timer} {}

    /// @brief Configure counter-based debouncing.
    /// @param counter Counter-based algorithm parameters; must satisfy CounterBased::IsValid().
    constexpr Debounce(CounterBased counter) noexcept : algorithm_{std::in_place_type<CounterBased>, counter} {}

    /************************************/
    /* Queries                          */
    /************************************/

    /// @brief Returns true if a debouncing algorithm (TimeBased or CounterBased) has been configured.
    /// @return true if a TimeBased or CounterBased is active; false in case of no debouncing (pass-through).
    [[nodiscard]] constexpr bool IsSet() const noexcept
    {
        return !std::holds_alternative<None>(algorithm_);
    }

    /// @brief Returns the configured algorithm variant.
    ///        Inspect with std::holds_alternative<TimeBased> or std::get<TimeBased> / std::get<Counter>.
    /// @return const reference to the Algorithm variant; holds None when IsSet() == false.
    [[nodiscard]] constexpr const Algorithm& GetAlgorithm() const noexcept
    {
        return algorithm_;
    }

  private:
    Algorithm algorithm_{};
};

}  // namespace score::mw::diag::dtc

#endif  // SCORE_MW_DIAG_DTC_DEBOUNCE_H
