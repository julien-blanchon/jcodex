use super::*;
use crate::context::MonitorNotification;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn monitor_mailbox_bounds_pending_events_and_reports_omissions() {
    let queue = InputQueue::new();
    for index in 0..20 {
        queue
            .enqueue_monitor_notification(MonitorNotification {
                id: "monitor".into(),
                description: "watch".into(),
                output: index.to_string(),
            })
            .await;
    }
    assert!(queue.has_trigger_turn_mailbox_items().await);
    let (items, _) = queue.drain_mailbox_input_items().await;
    assert_eq!(items.len(), 16);
    assert!(
        serde_json::to_string(&items.last())
            .unwrap()
            .contains("older queued monitor event omitted")
    );
    assert!(!queue.has_pending_mailbox_items().await);
}
