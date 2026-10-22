use crate::{StateSignal, StatusObservation, StatusSource};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const SCHEMA_VERSION: u32 = 4;
pub const EVENT_SCHEMA_VERSION: u32 = 1;
pub const STALE_AFTER_MS: u64 = 60_000;
pub const MAX_AGENT_HISTORY: usize = 256;

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
    #[serde(default)]
    pub status_source: StatusSource,
    #[serde(default)]
    pub discovered_at_ms: u64,
    #[serde(default)]
    pub last_hook_report_ms: Option<u64>,
    #[serde(default)]
    pub last_screen_report_ms: Option<u64>,
    #[serde(default)]
    pub last_screen_id: String,
    #[serde(default)]
    pub matched_rule: String,
    #[serde(default)]
    pub idle_confirmations: u8,
    #[serde(default)]
    pub last_screen_attempt_ms: u64,
    pub identity: Identity,
    pub tool: String,
    pub cwd: String,
    pub summary: String,
    pub detail: String,
    pub status: Status,
    pub status_since_ms: u64,
    /// Most recent accepted working -> idle transition in this execution.
    #[serde(default)]
    pub working_to_idle_at_ms: Option<u64>,
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
    #[serde(default)]
    pub pane: PaneInfo,
    #[serde(default)]
    pub last_instruction_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "crate::InstructionQueue::is_empty")]
    pub instruction_queue: crate::InstructionQueue,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PanePresence {
    #[default]
    Unknown,
    Present,
    Missing,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaneInfo {
    #[serde(default)]
    pub presence: PanePresence,
    #[serde(default)]
    pub observed_at_ms: u64,
    pub tab_id: Option<u32>,
    pub tab_name: String,
    pub title: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PaneOutput {
    pub agent_id: String,
    pub text: String,
}

impl Agent {
    pub fn visible(&self) -> bool {
        !self.ended
            && self.liveness != Liveness::Gone
            && self.pane.presence == PanePresence::Present
    }

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
    #[serde(default)]
    pub previous: Option<Status>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StoreData {
    #[serde(default)]
    pub scan_lease: Option<ScanLease>,
    pub schema_version: u32,
    pub revision: u64,
    pub last_scan_ms: u64,
    pub agents: BTreeMap<String, Agent>,
    pub activities: Vec<Activity>,
    #[serde(default)]
    pub requests: BTreeMap<String, crate::ActionResult>,
    #[serde(default)]
    pub launches: BTreeMap<String, crate::LaunchInfo>,
    #[serde(default)]
    pub recent_directories: Vec<String>,
}

/// IDs changed by core transitions; absent records are never implicit deletions.
#[derive(Clone, Debug, Default)]
pub struct ChangeSet {
    pub metadata: bool,
    pub agents: BTreeSet<String>,
    pub removed_agents: BTreeSet<String>,
    pub requests: BTreeSet<String>,
    pub launches: BTreeSet<String>,
    pub activities: bool,
    pub recent_directories: bool,
    pub replacement: bool,
}

/// Serialized persistence data is restored explicitly; live state is read-only
/// outside core transitions. Deref deliberately has no DerefMut implementation.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Store {
    #[serde(flatten)]
    pub(crate) data: StoreData,
    #[serde(skip)]
    pub(crate) changes: ChangeSet,
    #[serde(skip)]
    pub(crate) request_total: Option<usize>,
}

impl std::ops::Deref for Store {
    type Target = StoreData;
    fn deref(&self) -> &StoreData {
        &self.data
    }
}

impl Store {
    /// Explicit full-state restoration/import. Do not use for partially loaded state.
    pub fn restore(data: StoreData) -> Result<Self, String> {
        let mut store = Self::loaded(data, None)?;
        store.changes = ChangeSet {
            metadata: true,
            agents: store.agents.keys().cloned().collect(),
            requests: store.requests.keys().cloned().collect(),
            launches: store.launches.keys().cloned().collect(),
            activities: true,
            recent_directories: true,
            replacement: true,
            ..ChangeSet::default()
        };
        Ok(store)
    }
    /// Persistence hydration starts clean; migrations retain their own changes.
    pub fn loaded(data: StoreData, request_total: Option<usize>) -> Result<Self, String> {
        let mut store = Self {
            data,
            changes: ChangeSet::default(),
            request_total,
        };
        store.migrate()?;
        Ok(store)
    }
    pub fn changes(&self) -> &ChangeSet {
        &self.changes
    }
    pub fn request_count(&self) -> usize {
        self.request_total.unwrap_or(self.requests.len())
    }
    pub fn metadata(&self) -> StoreData {
        StoreData {
            schema_version: self.schema_version,
            revision: self.revision,
            last_scan_ms: self.last_scan_ms,
            scan_lease: self.scan_lease.clone(),
            ..StoreData::default()
        }
    }
    pub fn into_data(self) -> StoreData {
        self.data
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScanLease {
    pub token: String,
    pub expires_at_ms: u64,
}

impl Default for StoreData {
    fn default() -> Self {
        Self {
            scan_lease: None,
            schema_version: SCHEMA_VERSION,
            revision: 0,
            last_scan_ms: 0,
            agents: BTreeMap::new(),
            activities: Vec::new(),
            requests: BTreeMap::new(),
            launches: BTreeMap::new(),
            recent_directories: Vec::new(),
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
    /// v1 had only hook reports (and dashboard-generated events).
    /// Preserve those as hook-owned rather than overwrite historical state.
    pub fn migrate(&mut self) -> Result<(), String> {
        if self.data.schema_version < SCHEMA_VERSION {
            self.changes.metadata = true;
            self.changes.agents.extend(self.data.agents.keys().cloned());
        }
        if self.data.schema_version == 1 {
            for agent in self.data.agents.values_mut() {
                agent.discovered_at_ms = agent.status_since_ms;
                if agent.sequence > 0 {
                    agent.status_source = StatusSource::Hook;
                    agent.last_hook_report_ms = agent.last_report_ms;
                }
            }
            self.data.schema_version = 2;
        }
        if self.data.schema_version == 2 {
            for agent in self.data.agents.values_mut() {
                agent.pane.presence = PanePresence::Unknown;
                agent.pane.observed_at_ms = 0;
            }
            self.data.schema_version = SCHEMA_VERSION;
        }
        if self.data.schema_version == 3 {
            self.data.schema_version = SCHEMA_VERSION;
        }
        self.check_version()
    }

    pub fn check_version(&self) -> Result<(), String> {
        if self.data.schema_version != SCHEMA_VERSION {
            return Err(format!(
                "unsupported store version {}",
                self.data.schema_version
            ));
        }
        Ok(())
    }

    /// Hook identities must first be tied to a currently observed process.
    /// Events cannot create an arbitrary new incarnation or revive a closed one.
    pub fn apply(&mut self, event: &AgentEvent) -> Result<ApplyResult, String> {
        self.apply_signal(&StateSignal::Hook(event.clone()))
    }

    pub fn apply_signal(&mut self, signal: &StateSignal) -> Result<ApplyResult, String> {
        match signal {
            StateSignal::Hook(event) => self.apply_hook(event),
            StateSignal::Screen(observation) => self.apply_screen(observation),
            StateSignal::Instruction {
                identity,
                observed_at_ms,
                text,
            } => {
                self.validate_target(identity)?;
                let agent = self.data.agents.get_mut(&identity.agent_id).unwrap();
                agent.summary.clone_from(text);
                agent.last_instruction_ms = Some(*observed_at_ms);
                self.changes.agents.insert(identity.agent_id.clone());
                self.data.revision += 1;
                self.changes.metadata = true;
                Ok(ApplyResult::Applied)
            }
        }
    }

    fn apply_hook(&mut self, event: &AgentEvent) -> Result<ApplyResult, String> {
        if event.schema_version != EVENT_SCHEMA_VERSION || event.event_id.is_empty() {
            return Err("invalid event version or ID".into());
        }
        let Some(agent) = self.data.agents.get_mut(&event.identity.agent_id) else {
            return Ok(ApplyResult::Ignored);
        };
        if agent.identity != event.identity
            || agent.tool != event.tool
            || agent.liveness != Liveness::Live
            || event.sequence <= agent.sequence
            || event.event_id == agent.last_event_id
            || agent
                .last_hook_report_ms
                .is_some_and(|at| event.observed_at_ms < at)
        {
            return Ok(ApplyResult::Ignored);
        }
        self.changes.agents.insert(event.identity.agent_id.clone());
        agent.sequence = event.sequence;
        agent.status_source = StatusSource::Hook;
        agent.last_hook_report_ms = Some(event.observed_at_ms);
        agent.idle_confirmations = 0;
        agent.matched_rule.clear();
        agent.last_event_id.clone_from(&event.event_id);
        agent.last_report_ms = Some(event.observed_at_ms);
        if !event.cwd.is_empty() {
            agent.cwd.clone_from(&event.cwd);
        }
        if !event.summary.is_empty() {
            agent.summary.clone_from(&event.summary);
            agent.last_instruction_ms = Some(event.observed_at_ms);
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
        agent.instruction_queue.observe(
            next == Status::Working,
            event.kind == EventKind::TurnFinished,
            event.kind == EventKind::TurnFailed,
            event.observed_at_ms,
        );
        if next != agent.status {
            let previous = agent.status;
            agent.status = next;
            agent.status_since_ms = event.observed_at_ms;
            if previous == Status::Working && next == Status::Idle {
                agent.working_to_idle_at_ms = Some(event.observed_at_ms);
            }
            self.changes.activities = true;
            self.data.activities.push(Activity {
                agent_id: agent.identity.agent_id.clone(),
                at_ms: event.observed_at_ms,
                status: next,
                project: agent.project().into(),
                previous: Some(previous),
            });
            if self.data.activities.len() > 50 {
                self.data
                    .activities
                    .drain(..self.data.activities.len() - 50);
            }
        }
        self.data.revision += 1;
        self.changes.metadata = true;
        Ok(ApplyResult::Applied)
    }

    fn apply_screen(&mut self, observation: &StatusObservation) -> Result<ApplyResult, String> {
        if observation.observation_id.is_empty() {
            return Err("empty observation ID".into());
        }
        let Some(agent) = self.data.agents.get_mut(&observation.identity.agent_id) else {
            return Ok(ApplyResult::Ignored);
        };
        if agent.identity != observation.identity
            || agent.tool != observation.tool
            || agent.liveness != Liveness::Live
            || agent.ended
            || agent.status_source == StatusSource::Hook
            || agent.last_screen_id == observation.observation_id
            || agent
                .last_screen_report_ms
                .is_some_and(|at| observation.observed_at_ms <= at)
            || observation
                .observed_at_ms
                .saturating_sub(agent.discovered_at_ms)
                < 3000
        {
            return Ok(ApplyResult::Ignored);
        }
        self.changes
            .agents
            .insert(observation.identity.agent_id.clone());
        // Only consecutive fresh samples count, never timer repeats of one screen.
        if agent
            .last_screen_report_ms
            .is_some_and(|at| observation.observed_at_ms.saturating_sub(at) > 10_000)
        {
            agent.idle_confirmations = 0;
        }
        agent.status_source = StatusSource::Screen;
        agent.last_screen_report_ms = Some(observation.observed_at_ms);
        agent.last_screen_id.clone_from(&observation.observation_id);
        if let Some(next) = observation.status {
            agent.last_report_ms = Some(observation.observed_at_ms);
            let confirm_idle = next == Status::Idle
                && agent.status == Status::Working
                && !observation.visible_idle;
            if confirm_idle {
                agent.idle_confirmations = agent.idle_confirmations.saturating_add(1);
            } else {
                agent.idle_confirmations = 0;
            }
            if !confirm_idle || agent.idle_confirmations >= 3 {
                agent.matched_rule.clone_from(&observation.rule_id);
                agent.instruction_queue.observe(
                    next == Status::Working,
                    next == Status::Idle,
                    next == Status::Failed,
                    observation.observed_at_ms,
                );
                if next != agent.status {
                    let previous = agent.status;
                    agent.status = next;
                    agent.status_since_ms = observation.observed_at_ms;
                    if previous == Status::Working && next == Status::Idle {
                        agent.working_to_idle_at_ms = Some(observation.observed_at_ms);
                    }
                    self.changes.activities = true;
                    self.data.activities.push(Activity {
                        agent_id: agent.identity.agent_id.clone(),
                        at_ms: observation.observed_at_ms,
                        status: next,
                        project: agent.project().into(),
                        previous: Some(previous),
                    });
                    if self.data.activities.len() > 50 {
                        self.data
                            .activities
                            .drain(..self.data.activities.len() - 50);
                    }
                }
            }
        } else {
            agent.idle_confirmations = 0;
        }
        self.data.revision += 1;
        self.changes.metadata = true;
        Ok(ApplyResult::Applied)
    }

    /// A successful whole process inventory is required before calling this.
    pub fn reconcile(&mut self, found: &[FoundProcess], now_ms: u64) {
        if now_ms < self.data.last_scan_ms {
            return;
        }
        // A session rename changes its address, not the agent's execution.
        // Preserve the chosen detector and settings while rejecting old targets.
        let mut renamed = BTreeMap::new();
        for process in found {
            if self
                .data
                .agents
                .get(&process.identity.agent_id)
                .is_some_and(|agent| agent.liveness != Liveness::Gone)
            {
                continue;
            }
            let previous = self
                .data
                .agents
                .values()
                .find(|agent| {
                    let old = &agent.identity;
                    let new = &process.identity;
                    !agent.ended
                        && agent.liveness != Liveness::Gone
                        && old.session_name != new.session_name
                        && old.session_epoch == new.session_epoch
                        && old.pane_id == new.pane_id
                        && old.incarnation_id == new.incarnation_id
                        && old.pid == new.pid
                        && old.process_started == new.process_started
                })
                .cloned();
            if let Some(mut agent) = previous {
                renamed.insert(
                    agent.identity.agent_id.clone(),
                    process.identity.agent_id.clone(),
                );
                agent.identity = process.identity.clone();
                self.changes.agents.insert(agent.identity.agent_id.clone());
                self.data
                    .agents
                    .insert(agent.identity.agent_id.clone(), agent);
            }
        }
        for agent in self.data.agents.values_mut() {
            if let Some(parent) = agent.parent_id.as_ref().and_then(|id| renamed.get(id)) {
                agent.parent_id = Some(parent.clone());
                self.changes.agents.insert(agent.identity.agent_id.clone());
            }
        }
        for agent in self.data.agents.values_mut() {
            let next = if !agent.ended && found.iter().any(|p| p.identity == agent.identity) {
                Liveness::Live
            } else {
                Liveness::Gone
            };
            if agent.liveness != next {
                agent.liveness = next;
                self.changes.agents.insert(agent.identity.agent_id.clone());
            }
        }
        for process in found {
            if !self.data.agents.contains_key(&process.identity.agent_id) {
                self.changes
                    .agents
                    .insert(process.identity.agent_id.clone());
            }
            self.data
                .agents
                .entry(process.identity.agent_id.clone())
                .or_insert_with(|| Agent {
                    status_source: StatusSource::Unknown,
                    discovered_at_ms: now_ms,
                    last_hook_report_ms: None,
                    last_screen_report_ms: None,
                    last_screen_id: String::new(),
                    matched_rule: String::new(),
                    idle_confirmations: 0,
                    last_screen_attempt_ms: 0,
                    identity: process.identity.clone(),
                    tool: process.tool.clone(),
                    cwd: process.cwd.clone(),
                    summary: String::new(),
                    detail: String::new(),
                    status: Status::Found,
                    status_since_ms: now_ms,
                    working_to_idle_at_ms: None,
                    last_report_ms: None,
                    liveness: Liveness::Live,
                    sequence: 0,
                    last_event_id: String::new(),
                    ended: false,
                    pinned: false,
                    alias: String::new(),
                    parent_id: None,
                    pane: PaneInfo::default(),
                    last_instruction_ms: None,
                    instruction_queue: crate::InstructionQueue::default(),
                });
        }
        // Retain a short history without letting ended processes accumulate forever.
        let mut history: Vec<_> = self
            .data
            .agents
            .iter()
            .filter(|(_, a)| a.liveness != Liveness::Live)
            .map(|(id, a)| (a.last_report_ms.unwrap_or(a.status_since_ms), id.clone()))
            .collect();
        history.sort();
        let excess = history.len().saturating_sub(MAX_AGENT_HISTORY);
        for (index, (at, id)) in history.into_iter().enumerate() {
            if index >= excess && now_ms.saturating_sub(at) < 86_400_000 {
                continue;
            }
            self.data.agents.remove(&id);
            self.changes.agents.remove(&id);
            self.changes.removed_agents.insert(id);
        }
        self.data.last_scan_ms = now_ms;
        self.data.revision += 1;
        self.changes.metadata = true;
    }

    /// Pane observations describe reachability, never the agent's work status.
    /// Reject results for another process generation or an older pane query.
    pub fn observe_pane(&mut self, identity: &Identity, pane: PaneInfo) -> bool {
        let Some(agent) = self.data.agents.get_mut(&identity.agent_id) else {
            return false;
        };
        if agent.identity != *identity
            || agent.liveness != Liveness::Live
            || agent.ended
            || pane.observed_at_ms < agent.pane.observed_at_ms
            || pane.presence == PanePresence::Unknown
        {
            return false;
        }
        agent.pane = pane;
        self.changes.agents.insert(identity.agent_id.clone());
        self.data.revision += 1;
        self.changes.metadata = true;
        true
    }

    pub fn snapshot(&self, now_ms: u64) -> Snapshot {
        let verified = now_ms.saturating_sub(self.data.last_scan_ms) < 10_000;
        let agents = self
            .data
            .agents
            .values()
            .filter(|agent| !agent.ended && agent.liveness != Liveness::Gone)
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
            revision: self.data.revision,
            now_ms,
            last_scan_ms: self.data.last_scan_ms,
            agents,
            activities: self.data.activities.clone(),
        }
    }

    /// Setting an explicit value is idempotent when replicas repeat a request.
    pub fn set_pinned(&mut self, id: &str, pinned: bool) -> Result<(), String> {
        let agent = self
            .data
            .agents
            .get_mut(id)
            .ok_or("agent no longer exists")?;
        if agent.liveness != Liveness::Live || agent.ended {
            return Err("agent process changed or exited; refresh the list".into());
        }
        if agent.pinned != pinned {
            agent.pinned = pinned;
            self.changes.agents.insert(id.into());
            self.data.revision += 1;
            self.changes.metadata = true;
        }
        Ok(())
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

    #[test]
    fn history_is_bounded_without_removing_live_runs_and_snapshot_excludes_history() {
        let mut store = Store::default();
        for i in 0..MAX_AGENT_HISTORY + 20 {
            store.reconcile(&[found(&format!("run-{i}"))], 1000 + i as u64);
        }
        assert_eq!(store.agents.len(), MAX_AGENT_HISTORY + 1);
        assert!(!store.agents.contains_key("run-0"));
        assert!(store.changes.removed_agents.contains("run-0"));
        let snapshot = store.snapshot(2000);
        assert_eq!(snapshot.agents.len(), 1);
        assert_eq!(snapshot.agents[0].liveness, Liveness::Live);
        assert_eq!(
            store.snapshot(20_000).agents[0].liveness,
            Liveness::Unverified
        );

        let many_live: Vec<_> = (0..MAX_AGENT_HISTORY + 20)
            .map(|i| found(&format!("live-{i}")))
            .collect();
        store.reconcile(&many_live, 30_000);
        assert_eq!(store.snapshot(30_000).agents.len(), many_live.len());
        store.reconcile(&[], 86_500_000);
        assert!(store.agents.is_empty());
        assert!(store.snapshot(86_500_000).agents.is_empty());
    }

    #[test]
    fn pin_requests_are_idempotent_persist_and_reject_exited_generations() {
        let mut store = Store::default();
        store.reconcile(&[found("run")], 1000);
        store.set_pinned("run", true).unwrap();
        let revision = store.data.revision;
        store.set_pinned("run", true).unwrap();
        assert_eq!(store.data.revision, revision);
        let recovered: Store =
            serde_json::from_str(&serde_json::to_string(&store).unwrap()).unwrap();
        assert!(recovered.agents["run"].pinned);
        store.reconcile(&[found("replacement")], 1100);
        assert!(store.set_pinned("run", false).is_err());
        assert!(!store.data.agents["replacement"].pinned);
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
    fn session_rename_preserves_execution_state_and_rejects_old_targets() {
        let mut store = Store::default();
        let old = found("run");
        store.reconcile(&[old.clone()], 1000);
        store
            .apply(&event("run", 1, EventKind::TurnStarted))
            .unwrap();
        store.data.agents.get_mut("run").unwrap().pinned = true;
        store.data.agents.get_mut("run").unwrap().alias = "auth".into();
        let mut renamed = old.clone();
        renamed.identity.session_name = "renamed".into();
        renamed.identity.agent_id = "renamed-run".into();
        store.reconcile(&[renamed.clone()], 1100);
        let agent = &store.data.agents["renamed-run"];
        assert_eq!(agent.status_source, StatusSource::Hook);
        assert_eq!(agent.status, Status::Working);
        assert_eq!(agent.sequence, 1);
        assert!(agent.pinned);
        assert_eq!(agent.alias, "auth");
        assert_eq!(store.data.agents["run"].liveness, Liveness::Gone);
        assert!(store.validate_target(&old.identity).is_err());
        assert!(!store.observe_pane(
            &old.identity,
            PaneInfo {
                presence: PanePresence::Present,
                observed_at_ms: 1200,
                ..PaneInfo::default()
            }
        ));
        let mut hook = event("run", 2, EventKind::TurnFinished);
        hook.identity = renamed.identity.clone();
        hook.observed_at_ms = 1200;
        assert_eq!(store.apply(&hook), Ok(ApplyResult::Applied));
        assert_eq!(store.data.agents["renamed-run"].status, Status::Done);
        store.reconcile(&[renamed], 1300);
        assert_eq!(
            store.data.agents["renamed-run"].status_source,
            StatusSource::Hook
        );
        store.data.agents.get_mut("renamed-run").unwrap().alias = "updated".into();
        store.reconcile(&[old], 1400);
        assert_eq!(store.data.agents["run"].alias, "updated");
        assert_eq!(store.data.agents["run"].status, Status::Done);
        assert_eq!(store.data.agents["run"].status_source, StatusSource::Hook);
    }

    #[test]
    fn duplicate_and_out_of_order_events_do_not_rewind_status() {
        let mut store = Store::default();
        store.reconcile(&[found("run")], 1000);
        store
            .apply(&event("run", 2, EventKind::TurnFinished))
            .unwrap();
        let revision = store.data.revision;
        assert_eq!(
            store.apply(&event("run", 1, EventKind::TurnStarted)),
            Ok(ApplyResult::Ignored)
        );
        assert_eq!(
            store.apply(&event("run", 2, EventKind::TurnFinished)),
            Ok(ApplyResult::Ignored)
        );
        assert_eq!(store.data.revision, revision);
        assert_eq!(store.data.agents["run"].status, Status::Done);
    }

    #[test]
    fn reused_pane_rejects_events_and_settings_from_previous_process() {
        let mut store = Store::default();
        store.reconcile(&[found("old")], 1000);
        store.data.agents.get_mut("old").unwrap().pinned = true;
        store.reconcile(&[found("new")], 1100);
        assert_eq!(
            store.apply(&event("old", 1, EventKind::TurnStarted)),
            Ok(ApplyResult::Ignored)
        );
        assert_eq!(store.data.agents["old"].liveness, Liveness::Gone);
        assert_eq!(store.data.agents["new"].status, Status::Found);
        assert!(!store.data.agents["new"].pinned);
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
        assert_eq!(store.data.agents["run"].status, Status::Done);
    }

    #[test]
    fn session_end_is_not_revived_by_process_teardown_race() {
        let mut store = Store::default();
        store.reconcile(&[found("run")], 1000);
        store
            .apply(&event("run", 1, EventKind::SessionEnded))
            .unwrap();
        store.reconcile(&[found("run")], 1100);
        assert_eq!(store.data.agents["run"].liveness, Liveness::Gone);
        assert_eq!(
            store.apply(&event("run", 2, EventKind::ToolStarted)),
            Ok(ApplyResult::Ignored)
        );
    }
    fn screen(run: &str, at: u64, status: Option<Status>, visible_idle: bool) -> StateSignal {
        StateSignal::Screen(StatusObservation {
            identity: found(run).identity,
            tool: "claude".into(),
            observation_id: format!("screen-{run}-{at}"),
            observed_at_ms: at,
            status,
            visible_idle,
            rule_id: "test-rule".into(),
        })
    }

    #[test]
    fn working_to_idle_time_tracks_accepted_transitions_and_survives_reload() {
        let mut store = Store::default();
        store.reconcile(&[found("run")], 1000);
        store
            .apply_signal(&screen("run", 5000, Some(Status::Idle), true))
            .unwrap();
        assert_eq!(store.agents["run"].working_to_idle_at_ms, None);
        store
            .apply_signal(&screen("run", 6000, Some(Status::Working), false))
            .unwrap();
        store
            .apply_signal(&screen("run", 7000, Some(Status::Idle), false))
            .unwrap();
        assert_eq!(store.agents["run"].working_to_idle_at_ms, None);
        store
            .apply_signal(&screen("run", 8000, Some(Status::Idle), true))
            .unwrap();
        assert_eq!(store.agents["run"].working_to_idle_at_ms, Some(8000));
        for at in [8000, 7500, 9000] {
            store
                .apply_signal(&screen("run", at, Some(Status::Idle), true))
                .unwrap();
            assert_eq!(store.agents["run"].working_to_idle_at_ms, Some(8000));
        }
        // The timestamp is independent of the bounded activity history.
        store.data.activities.clear();
        let mut recovered: Store =
            serde_json::from_str(&serde_json::to_string(&store).unwrap()).unwrap();
        assert_eq!(recovered.agents["run"].working_to_idle_at_ms, Some(8000));
        recovered
            .apply_signal(&screen("run", 10000, Some(Status::Working), false))
            .unwrap();
        recovered
            .apply_signal(&screen("run", 11000, Some(Status::Idle), true))
            .unwrap();
        assert_eq!(recovered.agents["run"].working_to_idle_at_ms, Some(11000));
        recovered.reconcile(&[found("new-run")], 12000);
        assert_eq!(recovered.agents["new-run"].working_to_idle_at_ms, None);

        let mut old = serde_json::to_value(&store.agents["run"]).unwrap();
        old.as_object_mut().unwrap().remove("working_to_idle_at_ms");
        let old: Agent = serde_json::from_value(old).unwrap();
        assert_eq!(old.working_to_idle_at_ms, None);

        let mut hooked = Store::default();
        hooked.reconcile(&[found("hook")], 1000);
        hooked
            .apply(&event("hook", 1, EventKind::TurnStarted))
            .unwrap();
        let idle = event("hook", 2, EventKind::SessionStarted);
        hooked.apply(&idle).unwrap();
        assert_eq!(hooked.agents["hook"].working_to_idle_at_ms, Some(1002));
        assert_eq!(hooked.apply(&idle).unwrap(), ApplyResult::Ignored);
        hooked
            .apply(&event("hook", 3, EventKind::SessionStarted))
            .unwrap();
        hooked
            .apply_signal(&screen("hook", 9000, Some(Status::Working), false))
            .unwrap();
        assert_eq!(hooked.agents["hook"].working_to_idle_at_ms, Some(1002));
    }

    #[test]
    fn hook_takes_over_screen_and_persists_without_timeout_fallback() {
        let mut store = Store::default();
        store.reconcile(&[found("run")], 1000);
        assert_eq!(
            store
                .apply_signal(&screen("run", 2000, Some(Status::Working), false))
                .unwrap(),
            ApplyResult::Ignored
        );
        store
            .apply_signal(&screen("run", 5000, Some(Status::Working), false))
            .unwrap();
        // Source timestamps are independent: a first hook captured before the
        // screen sample still establishes the authoritative hook connection.
        store
            .apply(&event("run", 1, EventKind::TurnFinished))
            .unwrap();
        assert_eq!(store.data.agents["run"].status_source, StatusSource::Hook);
        let mut recovered: Store =
            serde_json::from_str(&serde_json::to_string(&store).unwrap()).unwrap();
        recovered.reconcile(&[found("run")], 100_000);
        assert_eq!(
            recovered
                .apply_signal(&screen("run", 100_000, Some(Status::Working), false))
                .unwrap(),
            ApplyResult::Ignored
        );
        assert_eq!(recovered.agents["run"].status, Status::Done);
        assert!(recovered.agents["run"].stale(100_000));
        recovered.reconcile(&[found("replacement")], 101_000);
        assert_eq!(
            recovered.agents["replacement"].status_source,
            StatusSource::Unknown
        );
    }

    #[test]
    fn screen_rejects_duplicates_reverse_order_and_reused_panes() {
        let mut store = Store::default();
        store.reconcile(&[found("run")], 1000);
        let working = screen("run", 5000, Some(Status::Working), false);
        store.apply_signal(&working).unwrap();
        assert_eq!(store.apply_signal(&working).unwrap(), ApplyResult::Ignored);
        assert_eq!(
            store
                .apply_signal(&screen("run", 4000, Some(Status::Idle), true))
                .unwrap(),
            ApplyResult::Ignored
        );
        store.reconcile(&[found("replacement")], 6000);
        assert_eq!(
            store
                .apply_signal(&screen("run", 7000, Some(Status::Idle), true))
                .unwrap(),
            ApplyResult::Ignored
        );
        assert_eq!(store.data.agents["replacement"].status, Status::Found);
    }

    #[test]
    fn fallback_idle_requires_fresh_consecutive_samples_and_overlay_resets_it() {
        let mut store = Store::default();
        store.reconcile(&[found("run")], 1000);
        store
            .apply_signal(&screen("run", 5000, Some(Status::Working), false))
            .unwrap();
        let idle = screen("run", 7000, Some(Status::Idle), false);
        store.apply_signal(&idle).unwrap();
        store.apply_signal(&idle).unwrap();
        assert_eq!(store.data.agents["run"].idle_confirmations, 1);
        store
            .apply_signal(&screen("run", 9000, None, false))
            .unwrap();
        assert_eq!(store.data.agents["run"].idle_confirmations, 0);
        for at in [11_000, 13_000] {
            store
                .apply_signal(&screen("run", at, Some(Status::Idle), false))
                .unwrap();
            assert_eq!(store.data.agents["run"].status, Status::Working);
        }
        store
            .apply_signal(&screen("run", 15_000, Some(Status::Idle), false))
            .unwrap();
        assert_eq!(store.data.agents["run"].status, Status::Idle);
        assert_eq!(store.agents["run"].working_to_idle_at_ms, Some(15_000));
        store
            .apply_signal(&screen("run", 17_000, Some(Status::Working), false))
            .unwrap();
        store
            .apply_signal(&screen("run", 19_000, Some(Status::Idle), true))
            .unwrap();
        assert_eq!(store.data.agents["run"].status, Status::Idle);
        assert_eq!(store.agents["run"].working_to_idle_at_ms, Some(19_000));
    }

    #[test]
    fn local_instruction_does_not_claim_hook_or_invent_turn_completion() {
        let mut store = Store::default();
        store.reconcile(&[found("run")], 1000);
        store
            .apply_signal(&screen("run", 5000, Some(Status::Idle), true))
            .unwrap();
        store
            .apply_signal(&StateSignal::Instruction {
                identity: found("run").identity,
                observed_at_ms: 6000,
                text: "새 지시".into(),
            })
            .unwrap();
        let agent = &store.data.agents["run"];
        assert_eq!(agent.status_source, StatusSource::Screen);
        assert_eq!(agent.status, Status::Idle);
        assert_eq!(agent.summary, "새 지시");
        assert_eq!(agent.sequence, 0);
    }

    #[test]
    fn older_process_inventory_cannot_revive_a_replaced_execution() {
        let mut store = Store::default();
        store.reconcile(&[found("old")], 1000);
        store.reconcile(&[found("new")], 2000);
        let revision = store.data.revision;
        store.reconcile(&[found("old")], 1500);
        assert_eq!(store.data.agents["old"].liveness, Liveness::Gone);
        assert_eq!(store.data.agents["new"].liveness, Liveness::Live);
        assert_eq!(store.data.revision, revision);
    }

    #[test]
    fn version_one_migration_preserves_reported_state_and_unknown_discovery() {
        let mut store = Store::default();
        store.reconcile(&[found("run"), found("manual")], 1000);
        store
            .apply(&event("run", 1, EventKind::TurnFinished))
            .unwrap();
        let mut value = serde_json::to_value(&store).unwrap();
        value["schema_version"] = 1.into();
        value.as_object_mut().unwrap().remove("scan_lease");
        for agent in value["agents"].as_object_mut().unwrap().values_mut() {
            for key in [
                "status_source",
                "discovered_at_ms",
                "last_hook_report_ms",
                "last_screen_report_ms",
                "last_screen_id",
                "matched_rule",
                "idle_confirmations",
                "last_screen_attempt_ms",
            ] {
                agent.as_object_mut().unwrap().remove(key);
            }
        }
        let mut migrated: Store = serde_json::from_value(value).unwrap();
        migrated.migrate().unwrap();
        assert_eq!(migrated.schema_version, SCHEMA_VERSION);
        assert_eq!(migrated.agents["run"].status_source, StatusSource::Hook);
        assert_eq!(migrated.agents["run"].status, Status::Done);
        assert_eq!(
            migrated.agents["manual"].status_source,
            StatusSource::Unknown
        );
        let before = serde_json::to_string(&migrated).unwrap();
        migrated.migrate().unwrap();
        assert_eq!(serde_json::to_string(&migrated).unwrap(), before);
    }

    #[test]
    fn pane_observations_preserve_work_and_reject_late_or_reused_generations() {
        let mut store = Store::default();
        let original = found("run");
        store.reconcile(&[original.clone()], 1000);
        store.data.agents.get_mut("run").unwrap().status = Status::Working;
        let present = PaneInfo {
            presence: PanePresence::Present,
            observed_at_ms: 2000,
            ..PaneInfo::default()
        };
        assert!(store.observe_pane(&original.identity, present.clone()));
        assert!(store.observe_pane(
            &original.identity,
            PaneInfo {
                presence: PanePresence::Missing,
                observed_at_ms: 3000,
                ..PaneInfo::default()
            }
        ));
        assert!(!store.observe_pane(&original.identity, present.clone()));
        assert_eq!(store.data.agents["run"].status, Status::Working);
        assert_eq!(store.data.agents["run"].liveness, Liveness::Live);
        assert_eq!(
            store.data.agents["run"].pane.presence,
            PanePresence::Missing
        );
        store.reconcile(&[found("replacement")], 4000);
        assert!(!store.observe_pane(&original.identity, present));
        assert_eq!(
            store.data.agents["replacement"].pane.presence,
            PanePresence::Unknown
        );
        let mut wrong = found("replacement").identity;
        wrong.incarnation_id = "old-generation".into();
        assert!(!store.observe_pane(
            &wrong,
            PaneInfo {
                presence: PanePresence::Present,
                observed_at_ms: 5000,
                ..PaneInfo::default()
            }
        ));
    }

    #[test]
    fn version_two_migration_requires_fresh_pane_evidence() {
        let mut store = Store::default();
        store.reconcile(&[found("run")], 1000);
        store.data.schema_version = 2;
        store.data.agents.get_mut("run").unwrap().pane.title = "old pane".into();
        let mut value = serde_json::to_value(&store).unwrap();
        let pane = value["agents"]["run"]["pane"].as_object_mut().unwrap();
        pane.remove("presence");
        pane.remove("observed_at_ms");
        let mut restored: Store = serde_json::from_value(value).unwrap();
        restored.migrate().unwrap();
        assert_eq!(restored.data.schema_version, SCHEMA_VERSION);
        assert_eq!(
            restored.data.agents["run"].pane.presence,
            PanePresence::Unknown
        );
        assert!(!restored.data.agents["run"].visible());
        assert_eq!(restored.data.agents["run"].pane.title, "old pane");
    }
}
