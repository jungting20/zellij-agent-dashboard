use dashboard_core::{AgentEvent, EventKind, Identity, StateSignal, EVENT_SCHEMA_VERSION};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::Path;

#[derive(Deserialize)]
pub struct ClaudeHook {
    pub hook_event_name: String,
    #[serde(default)]
    pub cwd: String,
    #[serde(default)]
    pub prompt: String,
    #[serde(default)]
    pub tool_name: String,
    #[serde(default)]
    pub tool_input: Value,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub error: String,
}

impl ClaudeHook {
    pub fn signal(
        &self,
        identity: Identity,
        sequence: u64,
        at: u64,
        event_id: String,
    ) -> Option<StateSignal> {
        Some(
            AgentEvent {
                schema_version: EVENT_SCHEMA_VERSION,
                event_id,
                identity,
                tool: "claude".into(),
                kind: self.kind()?,
                sequence,
                observed_at_ms: at,
                cwd: self.cwd.clone(),
                summary: limit(&self.prompt, 4096),
                detail: self.detail(),
            }
            .into(),
        )
    }

    pub fn kind(&self) -> Option<EventKind> {
        Some(match self.hook_event_name.as_str() {
            "SessionStart" => EventKind::SessionStarted,
            "UserPromptSubmit" => EventKind::TurnStarted,
            "PreToolUse" => EventKind::ToolStarted,
            "PostToolUse" | "PostToolUseFailure" => EventKind::ToolFinished,
            "PermissionRequest" => EventKind::PermissionRequired,
            "Notification" => EventKind::InputRequired,
            "Stop" => EventKind::TurnFinished,
            "StopFailure" => EventKind::TurnFailed,
            "PreCompact" => EventKind::CompactStarted,
            "PostCompact" => EventKind::CompactFinished,
            "SessionEnd" => EventKind::SessionEnded,
            _ => return None,
        })
    }

    pub fn detail(&self) -> String {
        if !self.error.is_empty() {
            return limit(&self.error, 400);
        }
        if !self.message.is_empty() {
            return limit(&self.message, 400);
        }
        let argument = ["file_path", "command", "pattern", "path"]
            .iter()
            .find_map(|key| self.tool_input.get(*key).and_then(Value::as_str))
            .unwrap_or("");
        limit(format!("{} {}", self.tool_name, argument).trim(), 400)
    }
}

pub fn limit(s: &str, chars: usize) -> String {
    s.chars().take(chars).collect()
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

pub fn claude_config(host: &Path, dir: &Path) -> Value {
    let command = format!(
        "{} --state-dir {} hook --tool claude",
        shell_quote(&host.to_string_lossy()),
        shell_quote(&dir.to_string_lossy())
    );
    let mut hooks = serde_json::Map::new();
    // These events are available in the locally installed Claude Code 2.1.70.
    // Newer optional events are added only after version-specific verification.
    for name in [
        "SessionStart",
        "UserPromptSubmit",
        "PreToolUse",
        "PostToolUse",
        "PostToolUseFailure",
        "PermissionRequest",
        "Notification",
        "Stop",
        "PreCompact",
        "SessionEnd",
    ] {
        let matcher = if name == "Notification" {
            "permission_prompt|idle_prompt"
        } else {
            "*"
        };
        hooks.insert(
            name.into(),
            json!([{"matcher": matcher, "hooks": [{"type":"command", "command":command,
            "async":false, "timeout":5}]}]),
        );
    }
    json!({"hooks": hooks})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hook_maps_real_claude_json_and_preserves_unicode_prompt() {
        let hook: ClaudeHook = serde_json::from_value(json!({"hook_event_name":"UserPromptSubmit", "prompt":"한글\n두 번째 줄", "cwd":"/a,b"})).unwrap();
        assert_eq!(hook.kind(), Some(EventKind::TurnStarted));
        assert_eq!(hook.prompt, "한글\n두 번째 줄");
        assert_eq!(hook.cwd, "/a,b");
    }

    #[test]
    fn shell_quoting_does_not_interpret_paths_as_commands() {
        let value = claude_config(
            Path::new("/tmp/a'$(echo bad)/host"),
            Path::new("/tmp/state space"),
        );
        let cmd = value["hooks"]["SessionStart"][0]["hooks"][0]["command"]
            .as_str()
            .unwrap();
        assert!(cmd.contains("'\\''"));
        assert!(cmd.contains("'/tmp/state space'"));
        assert_eq!(
            value["hooks"]["SessionStart"][0]["hooks"][0]["async"],
            false
        );
    }
}
