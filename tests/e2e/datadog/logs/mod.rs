use serde_json::Value;
use tracing::info;

use super::*;
use std::time::Duration;
use vector::test_util::trace_init;

const LOGS_ENDPOINT: &str = "/api/v2/logs";
const MAX_RETRIES: usize = 10;
const WAIT_INTERVAL: Duration = Duration::from_secs(1);

fn expected_log_events() -> usize {
    std::env::var("EXPECTED_LOG_EVENTS")
        .map(|n_expected| {
            n_expected
                .parse::<usize>()
                .expect("EXPECTED_LOG_EVENTS should be an unsigned integer.")
        })
        .unwrap_or(1000)
}

// Asserts that each log event has the hostname and timestamp fields, and
// Removes them from the log so that comparison can more easily be made.
// @return the number of log entries in the payload.
fn assert_timestamp_hostname(payloads: &mut [Value]) -> usize {
    let mut n_log_events = 0;

    payloads.iter_mut().for_each(|payload_array| {
        payload_array
            .as_array_mut()
            .expect("should be array")
            .iter_mut()
            .for_each(|log_val| {
                n_log_events += 1;

                let log = log_val
                    .as_object_mut()
                    .expect("log entries should be objects");

                // timestamp is available in the flog generated logs as a datetime but
                // there does not appear to be a way to add a custom parser in the Agent
                // to handle it.
                assert!(log.remove("timestamp").is_some());
                assert!(log.remove("hostname").is_some());
            })
    });

    n_log_events
}

// runs assertions that each set of payloads should be true to regardless
// of the pipeline
fn common_assertions(payloads: &mut [Value]) {
    assert!(!payloads.is_empty());

    let n_log_events = assert_timestamp_hostname(payloads);

    info!("log events received: {n_log_events}");

    assert_eq!(n_log_events, expected_log_events());
}

// reduces the payload down to just the log data
fn reduce_to_data(payloads: Vec<FakeIntakePayload<Value>>) -> Vec<Value> {
    payloads
        .into_iter()
        .map(|mut payload| payload.data.take())
        .collect()
}

#[tokio::test]
async fn validate() {
    trace_init();

    // Retry until we have log payloads or hit max retries.
    // This is to ensure events flow through to fakeintake before asking for them.
    info!("getting log payloads from agent-only pipeline");
    let mut agent_payloads = Vec::new();
    for _ in 0..MAX_RETRIES {
        agent_payloads = get_fakeintake_payloads::<FakeIntakeResponseJson>(
            &fake_intake_agent_address(),
            LOGS_ENDPOINT,
        )
        .await
        .payloads;

        if !agent_payloads.is_empty() {
            break;
        }

        info!("No valid payloads yet, retrying...");
        tokio::time::sleep(WAIT_INTERVAL).await;
    }

    // If we still don't have valid payloads after retries, fail the test
    assert!(
        !agent_payloads.is_empty(),
        "Failed to get valid log payloads from agent pipeline after {MAX_RETRIES} retries"
    );

    // The logs endpoint receives an empty healthcheck payload in the beginning
    agent_payloads.retain(|raw_payload| !raw_payload.data.as_array().unwrap().is_empty());

    let mut agent_payloads = reduce_to_data(agent_payloads);
    common_assertions(&mut agent_payloads);

    info!("getting log payloads from agent-vector pipeline");
    let vector_payloads = get_fakeintake_payloads::<FakeIntakeResponseJson>(
        &fake_intake_vector_address(),
        LOGS_ENDPOINT,
    )
    .await
    .payloads;

    let mut vector_payloads = reduce_to_data(vector_payloads);

    common_assertions(&mut vector_payloads);

    assert_eq!(agent_payloads, vector_payloads);
}
