use crate::{view::View, Agent, Liveness, Snapshot, Status};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NextFilter {
    All,
    PinnedOnly,
    IdleAndPinned,
    UnpinnedOnly,
    IdleAndUnpinned,
    WorkingOnly,
}

impl NextFilter {
    pub fn parse(payload: &str) -> Result<Self, String> {
        match payload {
            "" | "all" => Ok(Self::All),
            "pinned-only" => Ok(Self::PinnedOnly),
            "idle-and-pinned" => Ok(Self::IdleAndPinned),
            "unpinned-only" => Ok(Self::UnpinnedOnly),
            "idle-and-unpinned" => Ok(Self::IdleAndUnpinned),
            "working-only" => Ok(Self::WorkingOnly),
            _ => Err(format!("unknown agent-next filter: {payload}")),
        }
    }

    pub fn matches(self, agent: &Agent, pinned: bool) -> bool {
        agent.visible()
            && agent.liveness == Liveness::Live
            && match self {
                Self::All => true,
                Self::PinnedOnly => pinned,
                Self::IdleAndPinned => pinned && agent.status == Status::Idle,
                Self::UnpinnedOnly => !pinned,
                Self::IdleAndUnpinned => !pinned && agent.status == Status::Idle,
                Self::WorkingOnly => agent.status == Status::Working,
            }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NextRequest {
    pub filter: NextFilter,
    pub session: String,
    pub pane_id: Option<u32>,
}

impl View {
    /// Walk display order once, wrapping after the last row. The current row
    /// remains the anchor even when it no longer matches the requested filter.
    pub fn next_agent<'a>(
        &self,
        snapshot: &'a Snapshot,
        filter: NextFilter,
        current_id: Option<&str>,
    ) -> Option<&'a Agent> {
        let rows = self.rows(snapshot);
        let start = rows
            .iter()
            .position(|a| Some(a.identity.agent_id.as_str()) == current_id)
            .map(|at| at + 1)
            .unwrap_or(0);
        (0..rows.len())
            .map(|offset| rows[(start + offset) % rows.len()])
            .find(|a| filter.matches(a, super::view::hierarchy_root(snapshot, a).pinned))
    }

    pub fn select_next_working(&mut self, snapshot: &Snapshot) {
        if let Some(agent) = self.next_agent(
            snapshot,
            NextFilter::WorkingOnly,
            self.selected_id.as_deref(),
        ) {
            let id = agent.identity.agent_id.clone();
            let pinned = super::view::hierarchy_root(snapshot, agent).pinned;
            self.focus_panel(snapshot, pinned);
            self.selected_id = Some(id);
            self.reconcile_selection(snapshot);
        }
    }
}
