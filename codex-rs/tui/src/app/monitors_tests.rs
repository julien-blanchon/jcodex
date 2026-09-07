use super::*;
use crate::bottom_pane::ListSelectionView;
use crate::render::renderable::Renderable;
use codex_app_server_protocol::ThreadBackgroundTerminal;
use pretty_assertions::assert_eq;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

#[test]
fn monitor_menu_lists_only_watches_and_stops_the_owning_thread() {
    let thread_id = ThreadId::new();
    let terminal = |label| ThreadBackgroundTerminal {
        monitor_description: label,
        item_id: "call".into(),
        process_id: "42".into(),
        command: "tail -F server.log".into(),
        cwd: AbsolutePathBuf::from_absolute_path(std::env::temp_dir())
            .unwrap()
            .into(),
        os_pid: None,
        cpu_percent: None,
        rss_kb: None,
    };
    let items = monitor_items(
        thread_id,
        vec![terminal(None), terminal(Some("Server errors".into()))],
    );
    assert_eq!(items.len(), 1);
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let tx = crate::app_event_sender::AppEventSender::new(tx);
    (items[0].actions[0])(&tx);
    assert!(
        matches!(rx.try_recv().unwrap(), AppEvent::SubmitThreadOp { thread_id: owner, op: AppCommand::StopMonitor { process_id } } if owner == thread_id && process_id == "42")
    );
    let view = ListSelectionView::new(
        monitor_menu(items),
        tx,
        crate::keymap::RuntimeKeymap::defaults().list,
    );
    let area = Rect::new(0, 0, 80, view.desired_height(80));
    let mut buffer = Buffer::empty(area);
    view.render(area, &mut buffer);
    let rendered = (0..area.height)
        .map(|y| {
            (0..area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n");
    insta::assert_snapshot!("monitor_menu", rendered);
}
