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

//! Cruise control diagnostics exposed as `diag_api` data resources.
//!
//! A vehicle speed sensor can be forced to "stick" (fault injection). A
//! time-based debounce with the semantics of `score::mw::diag::dtc::Debounce::TimeBased`
//! turns a continuously failing check into a qualified fault:
//!
//! | resource                    | category    | access     |
//! |-----------------------------|-------------|------------|
//! | `vehicle_speed`             | currentData | read       |
//! | `cruise_state`              | currentData | read       |
//! | `speed_sensor_fault_status` | currentData | read       |
//! | `speed_sensor_stuck`        | storedData  | read/write |

use diag_api::sovd::data_resource::{
    DataCategory, DataResourceMetadata, ReadValueArgs, ReadValueHandle, ReadValueReply, WriteValueArgs,
    WriteValueHandle,
};
use diag_api::sovd::{DataError, DataResource, ErrorCode, GenericError};
use diag_api::{ReplyMessagePayload, RequestMessagePayload};
use diag_json::json;
use sovd_adapter::{DataResourceRegistry, RegistrationError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// State reported by the cruise control app.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CruiseState {
    /// Switched on, not holding a speed.
    Standby,
    /// Holding the set speed.
    Active,
    /// Refuses to engage, e.g. because the speed signal is implausible.
    Unavailable,
}

impl CruiseState {
    fn as_str(self) -> &'static str {
        match self {
            Self::Standby => "standby",
            Self::Active => "active",
            Self::Unavailable => "unavailable",
        }
    }
}

/// `score::mw::diag::dtc::Debounce::TimeBased`: a monitor result must hold
/// continuously for the given duration before the status changes.
#[derive(Clone, Copy, Debug)]
pub struct TimeBased {
    pub failed_duration: Duration,
    pub passed_duration: Duration,
}

/// Debounced status of the monitored condition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Passed,
    PreFailed,
    Failed,
    PrePassed,
}

impl Stage {
    fn as_str(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::PreFailed => "prefailed",
            Self::Failed => "failed",
            Self::PrePassed => "prepassed",
        }
    }
}

#[derive(Debug)]
struct Monitor {
    debounce: TimeBased,
    qualified_failed: bool,
    raw_failed: bool,
    raw_since: Instant,
}

impl Monitor {
    fn new(debounce: TimeBased, now: Instant) -> Self {
        Self {
            debounce,
            qualified_failed: false,
            raw_failed: false,
            raw_since: now,
        }
    }

    fn report(&mut self, failed: bool, now: Instant) {
        self.settle(now);
        if failed != self.raw_failed {
            self.raw_failed = failed;
            self.raw_since = now;
        }
    }

    fn settle(&mut self, now: Instant) {
        if self.raw_failed == self.qualified_failed {
            return;
        }
        let needed = if self.raw_failed {
            self.debounce.failed_duration
        } else {
            self.debounce.passed_duration
        };
        if now.duration_since(self.raw_since) >= needed {
            self.qualified_failed = self.raw_failed;
        }
    }

    fn stage(&mut self, now: Instant) -> Stage {
        self.settle(now);
        match (self.qualified_failed, self.raw_failed) {
            (false, false) => Stage::Passed,
            (false, true) => Stage::PreFailed,
            (true, true) => Stage::Failed,
            (true, false) => Stage::PrePassed,
        }
    }
}

#[derive(Debug)]
struct Sensor {
    started: Instant,
    stuck_at: Option<f64>,
    monitor: Monitor,
    state: CruiseState,
    set_speed_kmh: Option<f64>,
}

impl Sensor {
    /// Simulated speed drifts around 100 km/h; a stuck sensor repeats one value.
    fn speed_kmh(&self, now: Instant) -> f64 {
        self.stuck_at.unwrap_or_else(|| {
            let t = now.duration_since(self.started).as_secs_f64();
            ((100.0 + 5.0 * (t / 3.0).sin()) * 10.0).round() / 10.0
        })
    }

    /// Refresh the sensor state: report the current fault condition, settle the
    /// debounce, and update cruise_state accordingly. Called on every access.
    fn refresh(&mut self, now: Instant) {
        self.monitor.report(self.stuck_at.is_some(), now);
        let stage = self.monitor.stage(now);
        self.state = match (stage, self.state) {
            (Stage::Failed, _) => CruiseState::Unavailable,
            (Stage::Passed, CruiseState::Unavailable) => CruiseState::Standby,
            (_, state) => state,
        };
    }
}

/// Shared state of cruise control diagnostics; each resource holds a handle to it.
#[derive(Clone, Debug)]
pub struct CruiseDiag(Arc<Mutex<Sensor>>);

impl CruiseDiag {
    #[must_use]
    pub fn new(debounce: TimeBased) -> Self {
        let now = Instant::now();
        Self(Arc::new(Mutex::new(Sensor {
            started: now,
            stuck_at: None,
            monitor: Monitor::new(debounce, now),
            state: CruiseState::Active,
            set_speed_kmh: Some(100.0),
        })))
    }

    fn with<T>(&self, f: impl FnOnce(&mut Sensor, Instant) -> T) -> diag_api::Result<T> {
        let mut sensor = self.0.lock().map_err(|_| diag_api::Error::mutex_poisoned())?;
        let now = Instant::now();
        sensor.refresh(now);
        Ok(f(&mut sensor, now))
    }

    /// Registers the four cruise control resources in `registry`.
    ///
    /// # Errors
    /// [`RegistrationError`] if one of the ids is already taken.
    pub fn register(&self, registry: &mut DataResourceRegistry) -> Result<(), RegistrationError> {
        let meta = |id: &str, name: &str, category, read_only| DataResourceMetadata {
            id: id.to_string(),
            name: name.to_string(),
            translation_id: None,
            read_only,
            category,
            groups: Some(vec!["cruise".to_string()]),
        };
        registry.register(
            meta("vehicle_speed", "Vehicle speed", DataCategory::CurrentData, true),
            VehicleSpeed(self.clone()),
        )?;
        registry.register(
            meta("cruise_state", "Cruise control state", DataCategory::CurrentData, true),
            StateResource(self.clone()),
        )?;
        registry.register(
            meta(
                "speed_sensor_fault_status",
                "Vehicle speed sensor fault status",
                DataCategory::CurrentData,
                true,
            ),
            FaultStatus(self.clone()),
        )?;
        registry.register(
            meta(
                "speed_sensor_stuck",
                "Fault injection: vehicle speed sensor stuck",
                DataCategory::StoredData,
                false,
            ),
            StuckInjection(self.clone()),
        )
    }
}

fn json_reply(value: diag_json::Value) -> ReadValueHandle {
    ReadValueHandle::ready(ReadValueReply {
        data: ReplyMessagePayload::from_json(value, None),
        errors: None,
    })
}

fn as_handle(result: diag_api::Result<diag_json::Value>) -> ReadValueHandle {
    match result {
        Ok(value) => json_reply(value),
        Err(err) => ReadValueHandle::from_error(err),
    }
}

struct VehicleSpeed(CruiseDiag);

impl DataResource for VehicleSpeed {
    fn read(&self, _input: ReadValueArgs) -> ReadValueHandle {
        as_handle(self.0.with(|s, now| json!({ "value": s.speed_kmh(now), "unit": "km/h" })))
    }
}

struct StateResource(CruiseDiag);

impl DataResource for StateResource {
    fn read(&self, _input: ReadValueArgs) -> ReadValueHandle {
        as_handle(self.0.with(|s, _| {
            json!({ "state": s.state.as_str(), "set_speed": s.set_speed_kmh })
        }))
    }
}

struct FaultStatus(CruiseDiag);

impl DataResource for FaultStatus {
    fn read(&self, _input: ReadValueArgs) -> ReadValueHandle {
        as_handle(self.0.with(|s, now| {
            // refresh() already called in with(), so state is up to date
            let stage = s.monitor.stage(now);
            json!({
                "fault": "VehicleSpeedSensorStuck",
                "status": stage.as_str(),
                "test_failed": s.monitor.raw_failed,
                "confirmed": stage == Stage::Failed,
            })
        }))
    }
}

struct StuckInjection(CruiseDiag);

impl DataResource for StuckInjection {
    fn read(&self, _input: ReadValueArgs) -> ReadValueHandle {
        as_handle(self.0.with(|s, _| json!({ "stuck": s.stuck_at.is_some() })))
    }

    fn write(&mut self, input: WriteValueArgs) -> WriteValueHandle {
        let stuck = match input.user_data {
            Some(RequestMessagePayload::JSON(body)) => body.get("stuck").and_then(diag_json::Value::as_bool),
            _ => None,
        };
        let Some(stuck) = stuck else {
            return WriteValueHandle::from_error(DataError::from_error(GenericError::from_code(
                ErrorCode::IncompleteRequest,
                "expected a JSON body {\"stuck\": true|false}".to_string(),
            )));
        };
        match self.0.with(|s, now| {
            s.stuck_at = stuck.then(|| s.speed_kmh(now));
            s.monitor.report(stuck, now);
        }) {
            Ok(()) => WriteValueHandle::ready(),
            Err(_) => WriteValueHandle::from_error(DataError::from_error(GenericError::from_code(
                ErrorCode::SovdServerFailure,
                "cruise diag state is poisoned".to_string(),
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: Duration = Duration::from_millis(1);

    fn monitor() -> (Monitor, Instant) {
        let t0 = Instant::now();
        let debounce = TimeBased {
            failed_duration: 100 * MS,
            passed_duration: 50 * MS,
        };
        (Monitor::new(debounce, t0), t0)
    }

    #[test]
    fn failed_only_after_failed_duration() {
        let (mut m, t0) = monitor();
        m.report(true, t0);
        assert_eq!(m.stage(t0 + 99 * MS), Stage::PreFailed);
        assert_eq!(m.stage(t0 + 100 * MS), Stage::Failed);
    }

    #[test]
    fn short_glitch_never_qualifies() {
        let (mut m, t0) = monitor();
        m.report(true, t0);
        m.report(false, t0 + 60 * MS);
        assert_eq!(m.stage(t0 + 500 * MS), Stage::Passed);
    }

    #[test]
    fn recovery_needs_passed_duration() {
        let (mut m, t0) = monitor();
        m.report(true, t0);
        assert_eq!(m.stage(t0 + 100 * MS), Stage::Failed);
        m.report(false, t0 + 200 * MS);
        assert_eq!(m.stage(t0 + 249 * MS), Stage::PrePassed);
        assert_eq!(m.stage(t0 + 250 * MS), Stage::Passed);
    }

    #[test]
    fn injection_freezes_the_speed() {
        let diag = CruiseDiag::new(TimeBased {
            failed_duration: 100 * MS,
            passed_duration: 100 * MS,
        });
        let mut injection = StuckInjection(diag.clone());
        let args = WriteValueArgs {
            user_data: Some(RequestMessagePayload::JSON(json!({"stuck": true}))),
            ..WriteValueArgs::default()
        };
        assert!(matches!(injection.write(args), WriteValueHandle::Ready(Ok(()))));
        let (a, b) = diag
            .with(|s, now| (s.speed_kmh(now), s.speed_kmh(now + Duration::from_secs(7))))
            .unwrap();
        assert!((a - b).abs() < f64::EPSILON);
    }

    #[test]
    fn injection_rejects_bad_body() {
        let mut injection = StuckInjection(CruiseDiag::new(TimeBased {
            failed_duration: MS,
            passed_duration: MS,
        }));
        let args = WriteValueArgs {
            user_data: Some(RequestMessagePayload::JSON(json!({"stuck": "yes"}))),
            ..WriteValueArgs::default()
        };
        assert!(matches!(injection.write(args), WriteValueHandle::Ready(Err(_))));
    }
}
