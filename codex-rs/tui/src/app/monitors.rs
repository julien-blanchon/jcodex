//! Token-free monitor management using the existing background terminal API.
use super::*;
use codex_app_server_protocol::ThreadBackgroundTerminalsListParams;
use codex_app_server_protocol::ThreadBackgroundTerminalsListResponse;
use codex_app_server_protocol::ThreadBackgroundTerminalsTerminateParams;
use codex_app_server_protocol::ThreadBackgroundTerminalsTerminateResponse;

impl App {
    pub(super) async fn manage_monitors(
        &mut self,
        app_server: &mut AppServerSession,
        thread_id: ThreadId,
        stop_process_id: Option<&str>,
    ) -> Result<()> {
        let handle = app_server.request_handle();
        if let Some(process_id) = stop_process_id {
            let response: ThreadBackgroundTerminalsTerminateResponse = handle
                .request_typed(ClientRequest::ThreadBackgroundTerminalsTerminate {
                    request_id: app_server.next_request_id(),
                    params: ThreadBackgroundTerminalsTerminateParams {
                        thread_id: thread_id.to_string(),
                        process_id: process_id.to_string(),
                    },
                })
                .await?;
            self.chat_widget.add_info_message(
                if response.terminated {
                    "Monitor stopped."
                } else {
                    "Monitor has exited or could not be stopped."
                }
                .into(),
                /*hint*/ None,
            );
        }
        let mut cursor = None;
        let mut items = Vec::new();
        loop {
            let response: ThreadBackgroundTerminalsListResponse = handle
                .request_typed(ClientRequest::ThreadBackgroundTerminalsList {
                    request_id: app_server.next_request_id(),
                    params: ThreadBackgroundTerminalsListParams {
                        thread_id: thread_id.to_string(),
                        cursor,
                        limit: Some(100),
                    },
                })
                .await?;
            items.extend(monitor_items(thread_id, response.data));
            cursor = response.next_cursor;
            if cursor.is_none() {
                break;
            }
        }
        if items.is_empty() {
            self.chat_widget.add_info_message(
                "No active monitors in this thread.".into(),
                /*hint*/ None,
            );
        } else {
            self.chat_widget.show_selection_view(monitor_menu(items));
        }
        Ok(())
    }
}

fn monitor_items(
    thread_id: ThreadId,
    terminals: Vec<codex_app_server_protocol::ThreadBackgroundTerminal>,
) -> Vec<SelectionItem> {
    let mut items = Vec::new();
    for terminal in terminals {
        let Some(description) = terminal.monitor_description else {
            continue;
        };
        let process_id = terminal.process_id;
        items.push(SelectionItem {
            name: description,
            description: Some(terminal.command),
            actions: vec![Box::new(move |tx| {
                tx.send(AppEvent::SubmitThreadOp {
                    thread_id,
                    op: AppCommand::StopMonitor {
                        process_id: process_id.clone(),
                    },
                });
            })],
            dismiss_on_select: true,
            ..Default::default()
        });
    }
    items
}

#[cfg(test)]
#[path = "monitors_tests.rs"]
mod tests;

fn monitor_menu(items: Vec<SelectionItem>) -> SelectionViewParams {
    SelectionViewParams {
        title: Some(format!("Monitors ({})", items.len())),
        subtitle: Some("Select a monitor to stop it. Esc closes this menu.".into()),
        items,
        ..Default::default()
    }
}
