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

/// @file dtc_mock.h
/// @brief GMock implementation of score::mw::diag::dtc::DTC.

#ifndef SCORE_MW_DIAG_DTC_DTC_MOCK_H
#define SCORE_MW_DIAG_DTC_DTC_MOCK_H

#include "score/mw/diag/dtc/dtc.h"

#include <gmock/gmock.h>

namespace score::mw::diag::dtc::test
{

/// Mock for score::mw::diag::dtc::DTC.
class DTCMock : public DTC
{
  public:
    MOCK_METHOD((Result<void>), TriggerAction, (Action action), (override));
    MOCK_METHOD(void, SetClearBehaviour, (ClearBehaviour behaviour), (noexcept, override));
    MOCK_METHOD((Result<std::uint32_t>), GetNumber, (FormatType format), (const, noexcept, override));
    MOCK_METHOD(void, OnInit, (score::cpp::move_only_function<void(InitReason)> callback), (override));
};

}  // namespace score::mw::diag::dtc::test

#endif  // SCORE_MW_DIAG_DTC_DTC_MOCK_H
