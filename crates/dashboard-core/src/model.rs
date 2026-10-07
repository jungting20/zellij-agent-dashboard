use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const SCHEMA_VERSION: u32 = 1;
pub const STALE_AFTER_MS: u64 = 60_000;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Failed,
    Waiting,
    Done,
    Compact,
    Working,
    Idle,
    #[default]
    Found,
}

impl Status {
    pub fn label(self) -> &'static str {
        match self {
            Self::Failed => "failed",
            Self::Waiting => "waiting",
            Self::Done => "done",
            Self::Compact => "compact",
            Self::Working => "working",
            Self::Idle => "idle",
            Self::Found => "found",
        }
    }

    pub fn rank(self) -> u8 {
        match self {
            Self::Failed => 0,
            Self::Waiting => 1,
            Self::Done => 2,
            Self::Compact => 3,
            Self::Working => 4,
            Self::Idle => 5,
            Self::Found => 6,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Liveness {
    Live,
    Gone,
    #[default]
    Unverified,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Identity {
    pub agent_id: String,
    pub session_name: String,
    pub session_epoch: String,
    pub pane_id: u32,
    pub incarnation_id: String,
    pub pid: u32,
    pub process_started: String,
}

impl Identity {
    pub fn slot(&self) -> (&str, u32) {
        (&self.session_name, self.pane_id)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Agent {
    pub identity: Identity,
    pub tool: String,
    pub cwd: String,
    pub summary: String,
    pub detail: String,
    pub status: Status,
    pub status_since_ms: u64,
    pub last_report_ms: Option<u64>,
    pub liveness: Liveness,
    pub sequence: u64,
    pub last_event_id: String,
    #[serde(default)]
    pub ended: bool,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub alias: String,
    #[serde(default)]
    pub parent_id: Option<String>,
}

impl Agent {
    pub fn stale(&self, now_ms: u64) -> bool {
        self.last_report_ms
            .is_some_and(|at| now_ms.saturating_sub(at) >= STALE_AFTER_MS)
    }

    pub fn project(&self) -> &str {
        self.cwd
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .filter(|s| !s.is_empty())
            .unwrap_or("unknown")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    SessionStarted,
    TurnStarted,
    ToolStarted,
    ToolFinished,
    InputRequired,
    PermissionRequired,
    TurnFinished,
    TurnFailed,
    CompactStarted,
    CompactFinished,
    SessionEnded,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentEvent {
    pub schema_version: u32,
    pub event_id: String,
    pub identity: Identity,
    pub tool: String,
    pub kind: EventKind,
    pub sequence: u64,
    pub observed_at_ms: u64,
    #[serde(default)]
    pub cwd: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub detail: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Activity {
    pub agent_id: String,
    pub at_ms: u64,
    pub status: Status,
    pub project: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Store {
    pub schema_version: u32,
    pub revision: u64,
    pub last_scan_ms: u64,
    pub agents: BTreeMap<String, Agent>,
    pub activities: Vec<Activity>,
}

impl Default for Store {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            revision: 0,
            last_scan_ms: 0,
            agents: BTreeMap::new(),
            activities: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Snapshot {
    pub schema_version: u32,
    pub revision: u64,
    pub now_ms: u64,
    pub last_scan_ms: u64,
    pub agents: Vec<Agent>,
    pub activities: Vec<Activity>,
}

#[derive(Clone, Debug)]
pub struct FoundProcess {
    pub identity: Identity,
    pub tool: String,
    pub cwd: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplyResult {
    Applied,
    Ignored,
}

impl Store {
    pub fn check_version(&self) -> Result<(), String> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(format!("unsupported store version {}", self.schema_version));
        }
        Ok(())
    }

    /// Hook identities must first be tied to a currently observed process.
    /// Events cannot create an arbitrary new incarnation or revive a closed one.
    pub fn apply(&mut self, event: &AgentEvent) -> Result<ApplyResult, String> {
        if event.schema_version != SCHEMA_VERSION || event.event_id.is_empty() {
            return Err("invalid event version or ID".into());
        }
        let Some(agent) = self.agents.get_mut(&event.identity.agent_id) else {
            return Ok(ApplyResult::Ignored);
        };
        if agent.identity != event.identity
            || agent.tool != event.tool
            || agent.liveness != Liveness::Live
            || event.sequence <= agent.sequence
            || event.event_id == agent.last_event_id
            || agent
                .last_report_ms
                .is_some_and(|at| event.observed_at_ms < at)
        {
            return Ok(ApplyResult::Ignored);
        }
        agent.sequence = event.sequence;
        agent.last_event_id.clone_from(&event.event_id);
        agent.last_report_ms = Some(event.observed_at_ms);
        if !event.cwd.is_empty() {
            agent.cwd.clone_from(&event.cwd);
        }
        if !event.summary.is_empty() {
            agent.summary.clone_from(&event.summary);
        }
        agent.detail.clone_from(&event.detail);
        let next = match event.kind {
            EventKind::SessionStarted => Status::Idle,
            EventKind::TurnStarted | EventKind::ToolStarted | EventKind::CompactFinished => {
                Status::Working
            }
            // A late completion from a parallel tool cannot reopen a completed turn.
            EventKind::ToolFinished if matches!(agent.status, Status::Done | Status::Failed) => {
                agent.status
            }
            EventKind::ToolFinished => Status::Working,
            EventKind::InputRequired | EventKind::PermissionRequired => Status::Waiting,
            EventKind::TurnFinished => Status::Done,
            EventKind::TurnFailed => Status::Failed,
            EventKind::CompactStarted => Status::Compact,
            EventKind::SessionEnded => {
                agent.ended = true;
                agent.liveness = Liveness::Gone;
                agent.status
            }
        };
        if next != agent.status {
            agent.status = next;
            agent.status_since_ms = event.observed_at_ms;
            self.activities.push(Activity {
                agent_id: agent.identity.agent_id.clone(),
                at_ms: event.observed_at_ms,
                status: next,
                project: agent.project().into(),
            });
            if self.activities.len() > 50 {
                self.activities.drain(..self.activities.len() - 50);
            }
        }
        self.revision += 1;
        Ok(ApplyResult::Applied)
    }

    /// A successful whole process inventory is required before calling this.
    pub fn reconcile(&mut self, found: &[FoundProcess], now_ms: u64) {
        for agent in self.agents.values_mut() {
            agent.liveness = if !agent.ended && found.iter().any(|p| p.identity == agent.identity) {
                Liveness::Live
            } else {
                Liveness::Gone
            };
        }
        for process in found {
            self.agents
                .entry(process.identity.agent_id.clone())
                .or_insert_with(|| Agent {
                    identity: process.identity.clone(),
                    tool: process.tool.clone(),
                    cwd: process.cwd.clone(),
                    summary: String::new(),
                    detail: String::new(),
                    status: Status::Found,
                    status_since_ms: now_ms,
                    last_report_ms: None,
                    liveness: Liveness::Live,
                    sequence: 0,
                    last_event_id: String::new(),
                    ended: false,
                    pinned: false,
                    alias: String::new(),
                    parent_id: None,
                });
        }
        // Retain a short history without letting ended processes accumulate forever.
        self.agents.retain(|_, a| {
            a.liveness == Liveness::Live
                || now_ms.saturating_sub(a.last_report_ms.unwrap_or(a.status_since_ms)) < 86_400_000
        });
        self.last_scan_ms = now_ms;
        self.revision += 1;
    }

    pub fn snapshot(&self, now_ms: u64) -> Snapshot {
        let verified = now_ms.saturating_sub(self.last_scan_ms) < 10_000;
        let agents = self
            .agents
            .values()
            .cloned()
            .map(|mut agent| {
                if !verified && agent.liveness == Liveness::Live {
                    agent.liveness = Liveness::Unverified;
                }
                agent
            })
            .collect();
        Snapshot {
            schema_version: SCHEMA_VERSION,
            revision: self.revision,
            now_ms,
            last_scan_ms: self.last_scan_ms,
            agents,
            activities: self.activities.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found(run: &str) -> FoundProcess {
        FoundProcess {
            identity: Identity {
                agent_id: run.into(),
                session_name: "세션 이름".into(),
                session_epoch: "server-1".into(),
                pane_id: 1,
                incarnation_id: run.into(),
                pid: 42,
                process_started: run.into(),
            },
            tool: "claude".into(),
            cwd: "/project/api".into(),
        }
    }

    fn event(run: &str, sequence: u64, kind: EventKind) -> AgentEvent {
        AgentEvent {
            schema_version: 1,
            event_id: format!("event-{run}-{sequence}"),
            identity: found(run).identity,
            tool: "claude".into(),
            kind,
            sequence,
            observed_at_ms: 1000 + sequence,
            cwd: String::new(),
            summary: String::new(),
            detail: String::new(),
        }
    }

    #[test]
    fn duplicate_and_out_of_order_events_do_not_rewind_status() {
        let mut store = Store::default();
        store.reconcile(&[found("run")], 1000);
        store
            .apply(&event("run", 2, EventKind::TurnFinished))
            .unwrap();
        let revision = store.revision;
        assert_eq!(
            store.apply(&event("run", 1, EventKind::TurnStarted)),
            Ok(ApplyResult::Ignored)
        );
        assert_eq!(
            store.apply(&event("run", 2, EventKind::TurnFinished)),
            Ok(ApplyResult::Ignored)
        );
        assert_eq!(store.revision, revision);
        assert_eq!(store.agents["run"].status, Status::Done);
    }

    #[test]
    fn reused_pane_rejects_events_and_settings_from_previous_process() {
        let mut store = Store::default();
        store.reconcile(&[found("old")], 1000);
        store.agents.get_mut("old").unwrap().pinned = true;
        store.reconcile(&[found("new")], 1100);
        assert_eq!(
            store.apply(&event("old", 1, EventKind::TurnStarted)),
            Ok(ApplyResult::Ignored)
        );
        assert_eq!(store.agents["old"].liveness, Liveness::Gone);
        assert_eq!(store.agents["new"].status, Status::Found);
        assert!(!store.agents["new"].pinned);
    }

    #[test]
    fn reports_age_without_claiming_completion_or_death() {
        let mut store = Store::default();
        store.reconcile(&[found("run")], 1000);
        store
            .apply(&event("run", 1, EventKind::TurnStarted))
            .unwrap();
        let snapshot = store.snapshot(70_000);
        assert!(snapshot.agents[0].stale(snapshot.now_ms));
        assert_eq!(snapshot.agents[0].status, Status::Working);
        assert_eq!(snapshot.agents[0].liveness, Liveness::Unverified);
    }

    #[test]
    fn restarting_store_preserves_state_but_requires_new_process_evidence() {
        let mut store = Store::default();
        store.reconcile(&[found("run")], 1000);
        store
            .apply(&event("run", 1, EventKind::TurnFinished))
            .unwrap();
        let decoded: Store = serde_json::from_str(&serde_json::to_string(&store).unwrap()).unwrap();
        assert_eq!(
            decoded.snapshot(30_000).agents[0].liveness,
            Liveness::Unverified
        );
        assert_eq!(decoded.agents["run"].status, Status::Done);
    }

    #[test]
    fn late_tool_completion_does_not_reopen_done_turn() {
        let mut store = Store::default();
        store.reconcile(&[found("run")], 1000);
        store
            .apply(&event("run", 1, EventKind::TurnFinished))
            .unwrap();
        store
            .apply(&event("run", 2, EventKind::ToolFinished))
            .unwrap();
        assert_eq!(store.agents["run"].status, Status::Done);
    }

    #[test]
    fn session_end_is_not_revived_by_process_teardown_race() {
        let mut store = Store::default();
        store.reconcile(&[found("run")], 1000);
        store
            .apply(&event("run", 1, EventKind::SessionEnded))
            .unwrap();
        store.reconcile(&[found("run")], 1100);
        assert_eq!(store.agents["run"].liveness, Liveness::Gone);
        assert_eq!(
            store.apply(&event("run", 2, EventKind::ToolStarted)),
            Ok(ApplyResult::Ignored)
        );
    }
}
