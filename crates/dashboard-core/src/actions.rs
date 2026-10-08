use crate::{Identity, Liveness, Store};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionRequest {
    pub request_id: String,
    pub action: Action,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Action {
    Input {
        target: Identity,
        text: String,
    },
    Close {
        target: Identity,
    },
    Alias {
        target: Identity,
        alias: String,
    },
    Launch {
        session: String,
        epoch: String,
        cwd: String,
        tool: String,
    },
    Worktree {
        parent: Identity,
        branch: String,
        tool: String,
    },
    Lazygit {
        target: Identity,
    },
    Editor {
        target: Identity,
        text: String,
    },
    ShellChildren {
        parent: Identity,
        command: String,
    },
    Merge {
        parent: Identity,
        child: Identity,
    },
}

impl Action {
    pub fn target(&self) -> Option<&Identity> {
        match self {
            Self::Launch { .. } => None,
            Self::Worktree { parent, .. }
            | Self::ShellChildren { parent, .. }
            | Self::Merge { parent, .. } => Some(parent),
            Self::Input { target, .. }
            | Self::Close { target }
            | Self::Alias { target, .. }
            | Self::Lazygit { target }
            | Self::Editor { target, .. } => Some(target),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequestState {
    Pending,
    Succeeded,
    Failed,
    Uncertain,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActionResult {
    pub request: ActionRequest,
    pub state: RequestState,
    pub at_ms: u64,
    pub message: String,
    #[serde(default)]
    pub pane_id: Option<u32>,
    #[serde(default)]
    pub path: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LaunchInfo {
    pub parent_id: Option<String>,
    pub session: String,
    pub epoch: String,
    pub cwd: String,
    pub tool: String,
    pub pane_id: Option<u32>,
    pub agent_id: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Catalog {
    pub directories: Vec<String>,
    pub tools: Vec<String>,
    pub sessions: Vec<(String, String)>,
}

impl Store {
    pub fn validate_target(&self, target: &Identity) -> Result<(), String> {
        let agent = self
            .agents
            .get(&target.agent_id)
            .ok_or("agent no longer exists")?;
        if agent.identity != *target || agent.liveness != Liveness::Live || agent.ended {
            return Err("agent process changed or exited; refresh the list".into());
        }
        Ok(())
    }

    /// Claim and durably save before any side effect. A pending claim is never
    /// executed again: a crash can make delivery uncertain, not safely retryable.
    pub fn claim(&mut self, request: &ActionRequest, at_ms: u64) -> Result<bool, String> {
        if request.request_id.is_empty() || request.request_id.len() > 160 {
            return Err("invalid request ID".into());
        }
        if let Some(previous) = self.requests.get(&request.request_id) {
            if previous.request != *request {
                return Err("request ID reused with different action".into());
            }
            return Ok(false);
        }
        if self.requests.len() >= 4096 {
            return Err("request history full; archive the state after closing dashboards".into());
        }
        if let Some(target) = request.action.target() {
            self.validate_target(target)?;
        }
        if let Action::Close { target } = &request.action {
            let mut id = target.agent_id.as_str();
            let mut seen = std::collections::BTreeSet::new();
            while seen.insert(id) {
                let Some(agent) = self.agents.get(id) else {
                    break;
                };
                if agent.pinned {
                    return Err("고정된 에이전트는 먼저 고정을 해제해야 종료할 수 있습니다".into());
                }
                let Some(parent) = &agent.parent_id else {
                    break;
                };
                id = parent;
            }
        }
        self.requests.insert(
            request.request_id.clone(),
            ActionResult {
                request: request.clone(),
                state: RequestState::Pending,
                at_ms,
                message: "처리 결과 대기 중 · 확인 없이 자동 재전송하지 않습니다".into(),
                pane_id: None,
                path: String::new(),
            },
        );
        self.revision += 1;
        Ok(true)
    }

    pub fn remember_directory(&mut self, cwd: &str) {
        if cwd.is_empty() {
            return;
        }
        self.recent_directories.retain(|p| p != cwd);
        self.recent_directories.insert(0, cwd.into());
        self.recent_directories.truncate(100);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pending_claim_survives_restart_and_payload_collision_is_rejected() {
        let mut store = Store::default();
        let mut request = ActionRequest {
            request_id: "req-1".into(),
            action: Action::Launch {
                session: "s".into(),
                epoch: "e".into(),
                cwd: "/tmp".into(),
                tool: "pi".into(),
            },
        };
        assert!(store.claim(&request, 1).unwrap());
        let mut restored: Store =
            serde_json::from_str(&serde_json::to_string(&store).unwrap()).unwrap();
        assert!(!restored.claim(&request, 2).unwrap());
        assert_eq!(restored.requests["req-1"].state, RequestState::Pending);
        request.action = Action::Launch {
            session: "s".into(),
            epoch: "e".into(),
            cwd: "/other".into(),
            tool: "pi".into(),
        };
        assert!(restored.claim(&request, 3).is_err());
    }
}
