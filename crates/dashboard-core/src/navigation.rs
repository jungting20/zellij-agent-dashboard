use crate::{view::View, Agent, Identity, Liveness, Snapshot, Status};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

pub const MAX_NEXT_QUEUE: usize = 32;

/// Preserve accepted key order while bounding bursts and stalled host requests.
#[derive(Default)]
pub struct NextQueue(VecDeque<NavigationAction>);

impl NextQueue {
    pub fn push_back(&mut self, action: NavigationAction) -> bool {
        if self.0.len() >= MAX_NEXT_QUEUE {
            return false;
        }
        self.0.push_back(action);
        true
    }
    pub fn pop_front(&mut self) -> Option<NavigationAction> {
        self.0.pop_front()
    }
    pub fn clear(&mut self) {
        self.0.clear();
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NextFilter {
    All,
    PinnedOnly,
    IdleAndPinned,
    UnpinnedOnly,
    IdleAndUnpinned,
    WorkingOnly,
    NonIdle,
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
            "non-idle" => Ok(Self::NonIdle),
            _ => Err(format!("unknown agent-next filter: {payload}")),
        }
    }

    pub fn matches(self, agent: &Agent, pinned: bool) -> bool {
        agent.visible()
            && agent.liveness == Liveness::Live
            && match self {
                Self::All => true,
                Self::PinnedOnly => pinned,
                Self::IdleAndPinned => {
                    pinned && matches!(agent.status, Status::Idle | Status::Waiting)
                }
                Self::UnpinnedOnly => !pinned,
                Self::IdleAndUnpinned => {
                    !pinned && matches!(agent.status, Status::Idle | Status::Waiting)
                }
                Self::WorkingOnly => agent.status == Status::Working,
                Self::NonIdle => agent.status != Status::Idle,
            }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NextRequest {
    pub filter: NextFilter,
    pub session: String,
    pub pane_id: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NavigationAction {
    Next(NextFilter),
    Previous,
    Select(Identity),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NavigationRequest {
    pub action: NavigationAction,
    pub session: String,
    pub pane_id: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaneVisit {
    pub session: String,
    pub session_epoch: String,
    pub pane_id: u32,
    pub agent: Option<Identity>,
}

pub const MAX_NAVIGATION_HISTORY: usize = 10;

#[derive(Default, Serialize, Deserialize)]
pub struct NavigationHistory {
    current: Option<PaneVisit>,
    previous: VecDeque<PaneVisit>,
}

impl NavigationHistory {
    pub fn current(&self) -> Option<&PaneVisit> {
        self.current.as_ref()
    }

    pub fn advance(&mut self, from: Option<PaneVisit>, to: PaneVisit) -> bool {
        if from.as_ref() == Some(&to) {
            return false;
        }
        if from != self.current {
            self.previous.clear();
        }
        if let Some(from) = from {
            self.previous.push_back(from);
            if self.previous.len() > MAX_NAVIGATION_HISTORY {
                self.previous.pop_front();
            }
        }
        self.current = Some(to);
        true
    }

    pub fn back(
        &mut self,
        from: &PaneVisit,
        mut available: impl FnMut(&PaneVisit) -> Result<bool, String>,
    ) -> Result<Option<PaneVisit>, String> {
        if self.current.as_ref() != Some(from) {
            return Ok(None);
        }
        while let Some(candidate) = self.previous.pop_back() {
            if available(&candidate)? {
                self.current = Some(candidate.clone());
                return Ok(Some(candidate));
            }
        }
        Ok(None)
    }
}

impl View {
    /// Idle navigation cycles waiting candidates first. Without waiting candidates,
    /// walk by most recent transition to idle. Other filters keep display
    /// order, including the current anchor.
    pub fn next_agent<'a>(
        &self,
        snapshot: &'a Snapshot,
        filter: NextFilter,
        current_id: Option<&str>,
    ) -> Option<&'a Agent> {
        let mut rows = self.rows(snapshot);
        if matches!(
            filter,
            NextFilter::IdleAndPinned | NextFilter::IdleAndUnpinned
        ) {
            rows.retain(|a| filter.matches(a, super::view::hierarchy_root(snapshot, a).pinned));
            if rows.iter().any(|a| a.status == Status::Waiting) {
                rows.retain(|a| a.status == Status::Waiting);
            } else {
                // Stable sorting preserves display order for ties and unknown history.
                rows.sort_by_key(|a| std::cmp::Reverse(a.status_since_ms));
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn visit(id: u32) -> PaneVisit {
        PaneVisit {
            session: "dev".into(),
            session_epoch: "1".into(),
            pane_id: id,
            agent: None,
        }
    }

    #[test]
    fn previous_consumes_at_most_ten_moves_and_skips_closed_panes() {
        let mut history = NavigationHistory::default();
        for id in 0..15 {
            assert!(history.advance(Some(visit(id)), visit(id + 1)));
        }
        assert!(!history.advance(Some(visit(15)), visit(15)));
        assert_eq!(history.back(&visit(0), |_| Ok(true)).unwrap(), None);
        let mut current = visit(15);
        for id in (5..14).rev() {
            let previous = history.back(&current, |pane| Ok(pane.pane_id != 14));
            assert_eq!(previous.unwrap(), Some(visit(id)));
            current = visit(id);
        }
        assert_eq!(history.back(&current, |_| Ok(true)).unwrap(), None);
    }

    #[test]
    fn stalled_navigation_bounds_bursts_preserves_order_and_recovers() {
        let mut queue = NextQueue::default();
        for i in 0..MAX_NEXT_QUEUE {
            assert!(queue.push_back(if i % 2 == 0 {
                NavigationAction::Next(NextFilter::All)
            } else {
                NavigationAction::Previous
            }));
        }
        for _ in 0..10_000 {
            assert!(!queue.push_back(NavigationAction::Next(NextFilter::PinnedOnly)));
        }
        assert_eq!(queue.len(), MAX_NEXT_QUEUE);
        for i in 0..MAX_NEXT_QUEUE {
            assert_eq!(
                queue.pop_front(),
                Some(if i % 2 == 0 {
                    NavigationAction::Next(NextFilter::All)
                } else {
                    NavigationAction::Previous
                })
            );
        }
        assert!(queue.is_empty());
        assert!(queue.push_back(NavigationAction::Next(NextFilter::IdleAndPinned)));
        queue.clear();
        assert!(queue.is_empty());
        assert!(queue.push_back(NavigationAction::Next(NextFilter::IdleAndUnpinned)));
        assert_eq!(
            queue.pop_front(),
            Some(NavigationAction::Next(NextFilter::IdleAndUnpinned))
        );
    }
}
