//! Covers silent idle watches, real output wakeup, lifecycle operations, and early exit.
use codex_features::Feature;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use core_test_support::TestTargetOs;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::test_codex::TestCodexHarness;
use core_test_support::test_codex::test_codex;
use core_test_support::test_target_os;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::json;

fn call(id: &str, args: serde_json::Value) -> String {
    sse(vec![
        ev_function_call(id, "monitor", &args.to_string()),
        ev_completed(id),
    ])
}

fn done(id: &str) -> String {
    sse(vec![ev_assistant_message(id, "done"), ev_completed(id)])
}

async fn harness() -> anyhow::Result<TestCodexHarness> {
    TestCodexHarness::with_auto_env_builder(test_codex().with_config(|config| {
        assert!(config.features.enable(Feature::Monitor).is_ok());
    }))
    .await
}

fn watcher() -> &'static str {
    match test_target_os() {
        TestTargetOs::Linux | TestTargetOs::MacOs => {
            "while ! test -f signal; do sleep 0.05; done; echo WATCH_READY; while ! test -f finish; do sleep 0.05; done"
        }
        TestTargetOs::Windows => {
            "while (!(Test-Path signal)) { Start-Sleep -Milliseconds 50 }; Write-Output WATCH_READY; while (!(Test-Path finish)) { Start-Sleep -Milliseconds 50 }"
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn monitor_wakes_idle_session_and_can_be_listed_and_stopped() -> anyhow::Result<()> {
    let harness = harness().await?;
    let started = mount_sse_sequence(
        harness.server(),
        vec![
            call(
                "start",
                json!({ "command": watcher(), "description": "signal" }),
            ),
            done("m1"),
        ],
    )
    .await;
    harness.submit("watch for signal").await?;
    let start_output = started.requests()[1]
        .function_call_output_text("start")
        .unwrap();
    let id = start_output
        .split("id ")
        .nth(/*n*/ 1)
        .unwrap()
        .split(')')
        .next()
        .unwrap();
    insta::assert_snapshot!("monitor_started", start_output.replace(id, "MONITOR_ID"));
    // A silent script leaves the model idle. No polling tool calls or synthetic turns.
    tokio::time::sleep(std::time::Duration::from_millis(/*millis*/ 300)).await;
    assert_eq!(started.requests().len(), 2);
    let event = mount_sse_once(harness.server(), done("m2")).await;
    harness.write_file("signal", "ready").await?;
    wait_for_event(&harness.test().codex, |ev| {
        matches!(ev, EventMsg::TurnComplete(_))
    })
    .await;
    let request = event.single_request();
    let notifications: Vec<_> = request
        .message_input_texts("user")
        .into_iter()
        .filter(|text| text.starts_with("<monitor_notification>"))
        .collect();
    assert_eq!(notifications.len(), 1);
    assert!(notifications[0].contains("WATCH_READY"));
    assert!(notifications[0].len() < 900);
    let operations = mount_sse_sequence(
        harness.server(),
        vec![
            call("list", json!({"action": "list"})),
            call("stop", json!({"action": "stop", "id": id})),
            call("empty", json!({"action": "list"})),
            done("m3"),
        ],
    )
    .await;
    harness.submit("list and stop the watch").await?;
    let requests = operations.requests();
    let listed: serde_json::Value =
        serde_json::from_str(&requests[1].function_call_output_text("list").unwrap())?;
    assert_eq!(listed, json!([{ "id": id, "description": "signal" }]));
    assert_eq!(
        requests[2].function_call_output_text("stop").as_deref(),
        Some("Monitor stopped.")
    );
    assert_eq!(
        requests[3].function_call_output_text("empty").as_deref(),
        Some("[]")
    );
    assert!(
        harness
            .test()
            .codex
            .list_background_terminals()
            .await
            .is_empty()
    );
    harness.test().codex.submit(Op::Shutdown).await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn monitor_delivers_immediate_output_and_exit_during_active_turn() -> anyhow::Result<()> {
    let harness = harness().await?;
    let pause = match test_target_os() {
        TestTargetOs::Linux | TestTargetOs::MacOs => "sleep 0.5",
        TestTargetOs::Windows => "Start-Sleep -Milliseconds 500",
    };
    let mock = mount_sse_sequence(
        harness.server(),
        vec![
            call(
                "start",
                json!({ "command": "echo IMMEDIATE_EVENT", "description": "one shot" }),
            ),
            sse(vec![
                ev_function_call(
                    "work",
                    "exec_command",
                    &json!({"cmd": pause, "yield_time_ms": 1000}).to_string(),
                ),
                ev_completed("r2"),
            ]),
            done("m2"),
        ],
    )
    .await;
    harness.submit("watch immediate event").await?;
    assert!(mock.requests().iter().any(|r| {
        r.message_input_texts("user")
            .iter()
            .any(|t| t.contains("untrusted_output") && t.contains("IMMEDIATE_EVENT"))
    }));
    harness.test().codex.submit(Op::Shutdown).await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn monitor_timeout_wakes_session_and_removes_watch() -> anyhow::Result<()> {
    let harness = harness().await?;
    let mock = mount_sse_sequence(
        harness.server(),
        vec![
            call(
                "start",
                json!({ "command": watcher(), "description": "timeout", "timeout_ms": 500 }),
            ),
            done("m1"),
            call("list", json!({"action": "list"})),
            done("m2"),
        ],
    )
    .await;
    harness.submit("watch briefly").await?;
    wait_for_event(&harness.test().codex, |ev| {
        matches!(ev, EventMsg::TurnComplete(_))
    })
    .await;
    let requests = mock.requests();
    assert!(
        requests[2]
            .message_input_texts("user")
            .iter()
            .any(|t| t.contains("timeout reached"))
    );
    assert_eq!(
        requests[3].function_call_output_text("list").as_deref(),
        Some("[]")
    );
    harness.test().codex.submit(Op::Shutdown).await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn monitor_limit_rejects_before_execution_and_shutdown_reaps_watches() -> anyhow::Result<()> {
    let harness = harness().await?;
    let mut responses = (0..4)
        .map(|index| {
            call(
                &format!("start{index}"),
                json!({ "command": watcher(), "description": "silent" }),
            )
        })
        .collect::<Vec<_>>();
    let command = match test_target_os() {
        TestTargetOs::Linux | TestTargetOs::MacOs => "touch should_not_run",
        TestTargetOs::Windows => "New-Item should_not_run",
    };
    responses.extend([
        call(
            "overflow",
            json!({"command": command, "description": "overflow"}),
        ),
        done("finished"),
    ]);
    let mock = mount_sse_sequence(harness.server(), responses).await;
    harness.submit("create watchers").await?;
    assert!(
        mock.requests()[5]
            .function_call_output_text("overflow")
            .unwrap()
            .contains("At most four monitors")
    );
    assert!(!harness.path_exists("should_not_run").await?);
    assert_eq!(
        harness.test().codex.list_background_terminals().await.len(),
        4
    );
    harness.test().codex.submit(Op::Shutdown).await?;
    wait_for_event(&harness.test().codex, |ev| {
        matches!(ev, EventMsg::ShutdownComplete)
    })
    .await;
    assert!(
        harness
            .test()
            .codex
            .list_background_terminals()
            .await
            .is_empty()
    );
    Ok(())
}
