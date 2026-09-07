use super::ContextualUserFragment;
use codex_protocol::models::ContentItemKind;

/// Bounded external data, never user authorization. The whole fragment is below 900 bytes.
#[derive(Debug)]
pub(crate) struct MonitorNotification {
    pub(crate) id: String,
    pub(crate) description: String,
    pub(crate) output: String,
}

impl ContextualUserFragment for MonitorNotification {
    fn content_kind(&self) -> ContentItemKind {
        ContentItemKind("monitor.notification".into())
    }
    fn role(&self) -> &'static str {
        "user"
    }
    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }
    fn type_markers() -> (&'static str, &'static str) {
        ("<monitor_notification>", "</monitor_notification>")
    }
    fn body(&self) -> String {
        let id: String = self.id.chars().take(/*n*/ 36).collect();
        let description: String = self.description.chars().take(/*n*/ 64).collect();
        let mut output: String = self.output.chars().take(/*n*/ 512).collect();
        loop {
            let body = serde_json::json!({ "id": id, "description": description, "untrusted_output": output, "truncated": output != self.output }).to_string().replace('<', "\\u003c").replace('>', "\\u003e");
            if body.len() <= 800 {
                return body;
            }
            output.pop();
        }
    }
}
