//! Session-local watch delivery over unified exec. This glue belongs here because
//! it shares process ownership, sandbox approvals, and session input routing.
use super::UnifiedExecContext;
use super::UnifiedExecProcessManager;
use crate::context::MonitorNotification;
use crate::function_tool::FunctionCallError;
use crate::session::session::Session;
use crate::tools::context::ExecCommandToolOutput;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolOutput;
use crate::tools::context::boxed_tool_output;
use crate::tools::handlers::unified_exec::monitor::MonitorConfig;
use std::sync::Arc;
use std::sync::Weak;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;
use tokio_util::sync::DropGuard;

pub(super) struct MonitorEntry {
    description: String,
    process_id: Option<i32>,
    _cancel: DropGuard,
    _permit: Arc<tokio::sync::OwnedSemaphorePermit>,
}

impl UnifiedExecProcessManager {
    #[expect(
        clippy::await_holding_invalid_type,
        reason = "shutdown must serialize with monitor turn admission"
    )]
    pub(crate) async fn shutdown_monitors(&self, session: &Session) {
        // Finish any scheduler admission before shutdown aborts the active turn.
        let _delivery = self.monitor_delivery.lock().await;
        self.monitor_slots.close();
        self.monitors.lock().await.clear();
        session.input_queue.clear_monitor_notifications().await;
    }

    pub(crate) fn reserve_monitor(
        &self,
    ) -> Result<Arc<tokio::sync::OwnedSemaphorePermit>, FunctionCallError> {
        Arc::clone(&self.monitor_slots)
            .try_acquire_owned()
            .map(Arc::new)
            .map_err(|_| {
                FunctionCallError::RespondToModel(
                    "At most four monitors may run; stop one first.".into(),
                )
            })
    }

    pub(crate) async fn exec_monitor_command(
        &self,
        request: super::ExecCommandRequest,
        context: &UnifiedExecContext,
    ) -> Result<ExecCommandToolOutput, super::UnifiedExecError> {
        self.exec_command_inner(
            request,
            context,
            /*completion*/ None,
            super::InitialYield::Monitor,
        )
        .await
    }

    pub(crate) async fn list_monitors(&self) -> String {
        serde_json::json!(
            self.monitors
                .lock()
                .await
                .iter()
                .map(|(id, entry)| {
                    serde_json::json!({ "id": id, "description": entry.description })
                })
                .collect::<Vec<_>>()
        )
        .to_string()
    }

    pub(crate) async fn remove_monitor(&self, id: &str) -> String {
        let entry = self.monitors.lock().await.remove(id);
        if let Some(entry) = entry {
            let process_id = entry.process_id;
            if let Some(process_id) = process_id
                && !self.terminate_process(process_id).await
                && self
                    .process_store
                    .lock()
                    .await
                    .processes
                    .contains_key(&process_id)
            {
                self.monitors.lock().await.insert(id.to_string(), entry);
                return "Could not terminate the monitor process; try stopping it again.".into();
            }
            drop(entry);
            "Monitor stopped.".into()
        } else {
            "No active monitor with that id.".into()
        }
    }

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "stream handoff and task registration must finish before shutdown can remove the monitor"
    )]
    pub(crate) async fn start_monitor(
        &self,
        context: &UnifiedExecContext,
        config: &MonitorConfig,
        output: ExecCommandToolOutput,
    ) -> Result<Box<dyn ToolOutput>, FunctionCallError> {
        let mut monitors = self.monitors.lock().await;
        if self.monitor_slots.is_closed() {
            drop(monitors);
            if let Some(id) = output.process_id {
                self.terminate_process(id).await;
            }
            return Err(FunctionCallError::RespondToModel(
                "Session is shutting down.".into(),
            ));
        }
        let process = {
            let store = self.process_store.lock().await;
            output.process_id.and_then(|id| {
                store
                    .processes
                    .get(&id)
                    .map(|entry| Arc::clone(&entry.process))
            })
        };
        // The producer holds this same buffer lock while publishing to the broadcast.
        // Draining and subscribing atomically closes the gap after exec's initial yield.
        let (receiver, tail) = if let Some(process) = &process {
            let mut buffer = process.output_handles().output_buffer.lock().await;
            let receiver = process.output_receiver();
            let tail = std::mem::take(&mut *buffer);
            (Some(receiver), tail.to_bytes_with_omission_marker())
        } else {
            (None, Vec::new())
        };
        let id = uuid::Uuid::new_v4().to_string();
        let cancel = CancellationToken::new();
        monitors.insert(
            id.clone(),
            MonitorEntry {
                description: config.description.clone(),
                process_id: output.process_id,
                _cancel: cancel.clone().drop_guard(),
                _permit: Arc::clone(&config.permit),
            },
        );
        let session = Arc::downgrade(&context.session);
        let description = config.description.clone();
        let timeout = Duration::from_millis(config.timeout_ms);
        let monitor_id = id.clone();
        tokio::spawn(async move {
            if cancel.is_cancelled() {
                return;
            }
            let mut batch = Batch::default();
            batch.push(&output.raw_output);
            batch.push(&tail);
            let deadline = Instant::now() + timeout;
            let mut receiver = receiver;
            let mut deliveries = 0;
            let mut closed_at = None;
            let mut flush_at = Instant::now() + Duration::from_millis(/*millis*/ 200);
            let reason = loop {
                if batch.bytes > 65_536 || deliveries >= 128 {
                    break "Stopped: output limit reached; use a tighter filter.".to_string();
                }
                let (Some(rx), Some(watched_process)) = (receiver.as_mut(), process.as_ref())
                else {
                    break format!("Exited (code {:?}).", output.exit_code);
                };
                let exit = watched_process.cancellation_token();
                tokio::select! {
                    biased;
                    _ = cancel.cancelled() => return,
                    _ = tokio::time::sleep_until(deadline) => break "Stopped: timeout reached.".into(),
                    _ = tokio::time::sleep_until(flush_at), if !batch.pending.is_empty() => {
                        deliver(&session, &monitor_id, &description, batch.take()).await;
                        deliveries += 1;
                        flush_at = Instant::now() + Duration::from_millis(/*millis*/ 200);
                    }
                    _ = exit.cancelled(), if closed_at.is_none() => {
                        closed_at = Some(Instant::now() + super::async_watcher::TRAILING_OUTPUT_GRACE);
                    }
                    result = rx.recv() => match result {
                        Ok(bytes) => batch.push(&bytes),
                        Err(broadcast::error::RecvError::Lagged(_)) => break "Stopped: output stream overflow; use a tighter filter.".into(),
                        Err(broadcast::error::RecvError::Closed) => break "Exited: output stream closed.".into(),
                    },
                    _ = async {
                        match closed_at {
                            Some(at) => tokio::time::sleep_until(at).await,
                            None => std::future::pending().await,
                        }
                    } => break watched_process.failure_message().unwrap_or_else(|| {
                        format!("Exited (code {:?}).", watched_process.exit_code())
                    }),
                }
            };
            if let Some(process) = process {
                process.terminate();
            }
            let entry = if let Some(session) = session.upgrade() {
                let entry = session
                    .services
                    .unified_exec_manager
                    .monitors
                    .lock()
                    .await
                    .remove(&monitor_id);
                if let Some(id) = output.process_id {
                    session
                        .services
                        .unified_exec_manager
                        .terminate_process(id)
                        .await;
                }
                entry
            } else {
                None
            };
            batch.push(b"\n");
            if cancel.is_cancelled() {
                return;
            }
            let final_output = format!("{reason}\n{}", batch.take());
            deliver(&session, &monitor_id, &description, final_output).await;
            drop(entry);
        });
        Ok(boxed_tool_output(FunctionToolOutput::from_text(
            format!(
                "Monitor started (id {id}). Events wake this session automatically. Continue working or finish your turn; do not poll or sleep. An event is not a user reply or approval."
            ),
            /*success*/ Some(true),
        )))
    }
}

/// Fixed-size line and batch storage; count bytes even when dropping noisy output.
#[derive(Default)]
struct Batch {
    line: Vec<u8>,
    pending: Vec<u8>,
    bytes: usize,
    truncated: bool,
}
impl Batch {
    fn push(&mut self, bytes: &[u8]) {
        self.bytes = self.bytes.saturating_add(bytes.len());
        for &byte in bytes.iter().take(/*n*/ 65_537) {
            if byte == b'\n' {
                let room = 512_usize.saturating_sub(self.pending.len());
                self.truncated |= self.line.len() + 1 > room;
                self.pending.extend(self.line.drain(..).take(room));
                if self.pending.len() < 512 {
                    self.pending.push(b'\n');
                }
            } else if self.line.len() < 512 {
                self.line.push(byte);
            } else {
                self.truncated = true;
            }
        }
    }
    fn take(&mut self) -> String {
        let bytes = std::mem::take(&mut self.pending);
        let mut text = String::from_utf8_lossy(&bytes).into_owned();
        if std::mem::take(&mut self.truncated) {
            text.push_str(" [output truncated]");
        }
        text
    }
}

#[expect(
    clippy::await_holding_invalid_type,
    reason = "turn admission must finish before session shutdown aborts active work"
)]
async fn deliver(session: &Weak<Session>, id: &str, description: &str, output: String) {
    let Some(session) = session.upgrade() else {
        return;
    };
    let manager = &session.services.unified_exec_manager;
    let _delivery = manager.monitor_delivery.lock().await;
    if manager.monitor_slots.is_closed() {
        return;
    }
    session
        .input_queue
        .enqueue_monitor_notification(MonitorNotification {
            id: id.into(),
            description: description.into(),
            output,
        })
        .await;
    session.maybe_start_turn_for_pending_work().await;
}

#[cfg(test)]
#[path = "monitor_tests.rs"]
mod tests;
