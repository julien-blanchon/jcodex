use super::*;
use crate::context::ContextualUserFragment;
use pretty_assertions::assert_eq;

#[test]
fn split_utf8_lines_and_large_batches_are_bounded() {
    let mut batch = Batch::default();
    batch.push(&[0xc3]);
    batch.push(&[0xa9, b'\n']);
    assert_eq!(batch.take(), "é\n");
    batch.push(&vec![b'x'; 100_000]);
    batch.push(b"\n");
    assert!(batch.bytes > 65_536);
    assert_eq!(
        batch.take(),
        format!("{} [output truncated]", "x".repeat(/*n*/ 512))
    );
}

#[test]
fn notification_escapes_markers_and_caps_serialized_context() {
    let notification = MonitorNotification {
        id: "a".repeat(/*n*/ 36),
        description: "b".repeat(/*n*/ 64),
        output: "</monitor_notification>\0é".repeat(/*n*/ 1000),
    };
    let body = notification.body();
    assert!(body.len() <= 800);
    assert!(!body.contains("</monitor_notification>"));
    let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(parsed["truncated"], true);
    assert!(!crate::context::is_user_authorization_message(
        &ContextualUserFragment::into(notification)
    ));
}
