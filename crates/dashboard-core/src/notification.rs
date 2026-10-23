//! Read-only waiting notification shared by the host and the small plugin pane.
use crate::{text::Line, Agent, Liveness, PanePresence, Snapshot, Status};

pub fn waiting(snapshot: &Snapshot) -> Vec<&Agent> {
    let mut agents: Vec<_> = snapshot
        .agents
        .iter()
        .filter(|a| {
            a.status == Status::Waiting
                && a.liveness == Liveness::Live
                && !a.ended
                && a.pane.presence == PanePresence::Present
        })
        .collect();
    agents.sort_by_key(|a| (a.status_since_ms, &a.identity.agent_id));
    agents
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Notification {
    pub count: usize,
    pub entries: Vec<String>,
}

impl Notification {
    pub fn from_snapshot(snapshot: &Snapshot) -> Self {
        let agents = waiting(snapshot);
        Self {
            count: agents.len(),
            entries: agents
                .iter()
                .take(3)
                .map(|a| {
                    format!(
                        " {} · {} / {}",
                        a.tool, a.identity.session_name, a.pane.tab_name
                    )
                })
                .collect(),
        }
    }

    pub fn render(&self, rows: usize, cols: usize) -> Vec<String> {
        let mut lines = vec![Line::color(
            format!(" 응답 대기 {}개", self.count),
            220,
            true,
        )];
        let capacity = rows.saturating_sub(1);
        for (index, entry) in self.entries.iter().take(capacity).enumerate() {
            if index + 1 == capacity && self.count > capacity {
                lines.push(Line::plain(format!(
                    " 외 {}개 · 대시보드에서 확인",
                    self.count - index
                )));
            } else {
                lines.push(Line::plain(entry));
            }
        }
        lines
            .into_iter()
            .take(rows)
            .map(|l| l.fit(cols, true).render())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FoundProcess, Identity, Store};

    #[test]
    fn notifications_exclude_unreachable_and_resolved_runs() {
        let mut store = Store::default();
        store.reconcile(
            &(0..6)
                .map(|id| FoundProcess {
                    identity: Identity {
                        agent_id: id.to_string(),
                        session_name: "운영".into(),
                        session_epoch: "epoch".into(),
                        pane_id: id,
                        incarnation_id: id.to_string(),
                        pid: id + 100,
                        process_started: "start".into(),
                    },
                    tool: "codex".into(),
                    cwd: "/tmp".into(),
                })
                .collect::<Vec<_>>(),
            1000,
        );
        let mut snapshot = store.snapshot(1000);
        for agent in &mut snapshot.agents {
            agent.status = Status::Waiting;
            agent.pane.presence = PanePresence::Present;
        }
        snapshot.agents[1].status = Status::Idle;
        snapshot.agents[2].ended = true;
        snapshot.agents[3].liveness = Liveness::Gone;
        snapshot.agents[4].pane.presence = PanePresence::Missing;
        snapshot.agents[5].liveness = Liveness::Unverified;
        assert_eq!(waiting(&snapshot).len(), 1);
        assert!(Notification::from_snapshot(&snapshot).render(4, 40)[0].contains("응답 대기 1개"));
        snapshot.agents[0].status = Status::Working;
        assert!(waiting(&snapshot).is_empty());
    }
}
