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

mod cruise;

use cruise::{CruiseDiag, TimeBased};
use opensovd_core::Component;
use opensovd_server::{Server, Topology};
use sovd_adapter::{DataResourceRegistry, SovdDataProvider};
use std::time::Duration;
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

const DEFAULT_ADDRESS: &str = "127.0.0.1:7690";

fn address() -> String {
    std::env::var("SCORE_GATEWAY_ADDRESS").unwrap_or_else(|_| DEFAULT_ADDRESS.to_owned())
}

fn millis_from_env(name: &str, default: u64) -> Duration {
    Duration::from_millis(std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default))
}

async fn topology(debounce: TimeBased) -> Result<Topology, Box<dyn std::error::Error>> {
    // Real diag_api resources, served through the DataProvider adapter.
    let cruise = CruiseDiag::new(debounce);
    let mut registry = DataResourceRegistry::new();
    cruise.register(&mut registry)?;

    let topology = Topology::new();
    topology
        .write()
        .await
        .add_component(Component::new("cruise", "Cruise Control").with_data_provider(SovdDataProvider::new(registry)));
    Ok(topology)
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let address = address();
    let listener = TcpListener::bind(&address).await?;
    let topology = topology(TimeBased {
        failed_duration: millis_from_env("CRUISE_DEBOUNCE_FAILED_MS", 5000),
        passed_duration: millis_from_env("CRUISE_DEBOUNCE_PASSED_MS", 2000),
    })
    .await?;
    let server = Server::builder()
        .base_uri(format!("http://{address}/sovd"))?
        .listener(listener)
        .topology(topology)
        .build()?;
    server.serve().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use opensovd_client::Client;
    use tokio::time::sleep;

    /// Starts the gateway on an ephemeral port and returns a connected client.
    async fn start(debounce: TimeBased) -> Client {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("listener");
        let address = listener.local_addr().expect("local address");
        let server = Server::builder()
            .base_uri(format!("http://{address}/sovd"))
            .expect("base URI")
            .listener(listener)
            .topology(topology(debounce).await.expect("topology"))
            .build()
            .expect("server");
        tokio::spawn(async move { server.serve().await.expect("server task") });

        let client = Client::connect(&format!("http://{address}/sovd/v1")).expect("client");
        for _ in 0..40 {
            if client.list_components().send().await.is_ok() {
                return client;
            }
            sleep(Duration::from_millis(25)).await;
        }
        panic!("gateway did not become reachable");
    }

    fn debounce(ms: u64) -> TimeBased {
        TimeBased {
            failed_duration: Duration::from_millis(ms),
            passed_duration: Duration::from_millis(ms),
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn serves_diag_api_resources_not_demo_data() {
        let client = start(debounce(50)).await;
        let components = client.list_components().send().await.expect("components");
        let ids: Vec<_> = components.data.items.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, ["cruise"]);

        let data = client.component("cruise").list_data().send().await.expect("data list");
        let ids: Vec<_> = data.data.items.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, ["vehicle_speed", "cruise_state", "speed_sensor_fault_status", "speed_sensor_stuck"]);
        assert!(data
            .data
            .items
            .iter()
            .all(|m| m.groups.as_deref() == Some(&["cruise".to_string()][..])));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn injected_fault_is_debounced_then_reported() {
        let client = start(debounce(200)).await;
        let cruise = client.component("cruise");
        let status = || async {
            let reply = cruise.data("speed_sensor_fault_status").read().send().await.expect("status");
            reply.data["status"].as_str().expect("status string").to_owned()
        };
        assert_eq!(status().await, "passed");

        cruise.data("speed_sensor_stuck")
            .write(&serde_json::json!({"stuck": true}))
            .expect("body")
            .send()
            .await
            .expect("inject");
        assert_eq!(status().await, "prefailed");
        sleep(Duration::from_millis(250)).await;
        assert_eq!(status().await, "failed");

        cruise.data("speed_sensor_stuck")
            .write(&serde_json::json!({"stuck": false}))
            .expect("body")
            .send()
            .await
            .expect("clear");
        sleep(Duration::from_millis(250)).await;
        assert_eq!(status().await, "passed");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn read_only_resource_rejects_writes() {
        let client = start(debounce(50)).await;
        let result = client
            .component("cruise")
            .data("vehicle_speed")
            .write(&serde_json::json!(1))
            .expect("body")
            .send()
            .await;
        assert!(result.is_err());
    }
}
