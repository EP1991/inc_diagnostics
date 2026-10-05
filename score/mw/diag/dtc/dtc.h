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

/// @file dtc.h
/// @brief DTC interface and associated types (Status, InitReason, FormatType).

#ifndef SCORE_MW_DIAG_DTC_DTC_H
#define SCORE_MW_DIAG_DTC_DTC_H

#include "score/move_only_function.hpp"
#include "score/mw/diag/diag_result.h"

#include <cstdint>

namespace score::mw::diag::dtc
{

using score::mw::diag::uds::Result;

/************************************/
/* FormatType                       */
/************************************/

/// @brief DTC number format to request via DTC::GetNumber().
enum class FormatType : std::uint8_t
{
    kObd = 0U,    ///< OBD-II (J1979).
    kUds = 1U,    ///< ISO 14229-1 (UDS).
    kJ1939 = 2U,  ///< SAE J1939.
};

/************************************/
/* InitReason                       */
/************************************/

/// @brief Reason code delivered to the DTC::OnInit() callback.
///        Reset internal debouncing state whenever the callback is invoked.
enum class InitReason : std::uint8_t
{
    kClear,             ///< DTC storage was cleared by a tester.
    kRestart,           ///< ECU restarted (power-on or warm reset).
    kReenabled,         ///< DTC monitoring re-enabled after an enable-condition change.
    kStorageReenabled,  ///< DTC storage re-enabled after a storage-condition change.
};

/// @brief Debouncing action for a DTC — passed to DTC::TriggerAction().
enum class Action : std::uint8_t
{
    kPassed = 0x00,
    kFailed = 0x01,
    kPrepassed = 0x02,
    kPrefailed = 0x03,
    kFaultDetectionCounterThresholdReached = 0x04,
    kResetTestFailed = 0x05,
    kFreezeDebouncing = 0x06,
    kResetDebouncing = 0x07,
    kPrestore = 0x08,
    kClearPrestore = 0x09,
};

/// @brief Clearing behaviour for a DTC — passed to DTC::SetClearBehaviour() and Builder::ConfigureClearBehaviour().
enum class ClearBehaviour : std::uint8_t
{
    kClearable,            ///< (Default) Tester may clear this DTC via ClearDiagnosticInformation.
    kNotClearable,         ///< Tester clear requests are ignored for this DTC.
    kReenterAfterCleared,  ///< DTC re-enters storage immediately after a tester clear.
};

/// @brief Abstract interface for triggering fault action on a single DTC.
class DTC
{
  public:
    /// @brief Trigger an action for this DTC.
    /// @param action The action to trigger (e.g. kPassed, kFailed, kPrepassed, etc.).
    /// @return Ok on success; Err if the action failed to get processed.
    [[nodiscard]] virtual Result<void> TriggerAction(Action action) = 0;

    /// @brief Set the clearing behaviour for this DTC.
    /// @param behaviour One of ClearBehaviour::kClearable (default), kNotClearable,
    ///                  or kReenterAfterCleared.
    virtual void SetClearBehaviour(ClearBehaviour behaviour) noexcept = 0;

    /// @brief Return the DTC number in the requested format.
    /// @param format The numeric format to use (OBD, UDS, or J1939).
    /// @return The DTC number on success; Err if the requested format is unsupported.
    [[nodiscard]] virtual Result<std::uint32_t> GetNumber(FormatType format) const noexcept = 0;

    /// @brief Register a callback invoked with an InitReason on startup, clear, or re-enable.
    /// @param callback Invoked with the InitReason each time the DTC is (re-)initialised.
    virtual void OnInit(score::cpp::move_only_function<void(InitReason)> callback) = 0;

    /// @brief Virtual destructor to allow safe deletion.
    virtual ~DTC() noexcept = default;

  protected:
    constexpr DTC() noexcept = default;

    constexpr DTC(DTC&&) noexcept = default;
    constexpr DTC(const DTC&) noexcept = default;
    constexpr DTC& operator=(DTC&&) & noexcept = default;
    constexpr DTC& operator=(const DTC&) & noexcept = default;
};

}  // namespace score::mw::diag::dtc

#endif  // SCORE_MW_DIAG_DTC_DTC_H
