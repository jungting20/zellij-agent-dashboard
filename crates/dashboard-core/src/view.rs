use crate::{Agent, Liveness, Snapshot};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

#[derive(Default)]
pub struct View {
    pub selected_id: Option<String>,
    pub query: String,
    pub searching: bool,
    pub pinned_only: bool,
}

impl View {
    pub fn rows<'a>(&self, snapshot: &'a Snapshot) -> Vec<&'a Agent> {
        let query = self.query.to_lowercase();
        let mut agents: Vec<_> = snapshot
            .agents
            .iter()
            .filter(|a| !self.pinned_only || a.pinned)
            .filter(|a| {
                query.is_empty()
                    || format!(
                        "{} {} {} {} {} {}",
                        a.tool,
                        a.cwd,
                        a.summary,
                        a.alias,
                        a.identity.session_name,
                        a.status.label()
                    )
                    .to_lowercase()
                    .contains(&query)
            })
            .collect();
        agents.sort_by_key(|a| {
            (
                a.liveness != Liveness::Live,
                !a.pinned,
                a.status.rank(),
                a.identity.session_name.as_str(),
                a.identity.pane_id,
                a.identity.agent_id.as_str(),
            )
        });
        agents
    }

    pub fn reconcile_selection(&mut self, snapshot: &Snapshot) {
        let rows = self.rows(snapshot);
        if !rows
            .iter()
            .any(|a| Some(&a.identity.agent_id) == self.selected_id.as_ref())
        {
            self.selected_id = rows.first().map(|a| a.identity.agent_id.clone());
        }
    }

    pub fn select(&mut self, snapshot: &Snapshot, delta: isize) {
        let rows = self.rows(snapshot);
        if rows.is_empty() {
            self.selected_id = None;
            return;
        }
        let at = rows
            .iter()
            .position(|a| Some(&a.identity.agent_id) == self.selected_id.as_ref())
            .unwrap_or(0);
        let next = at.saturating_add_signed(delta).min(rows.len() - 1);
        self.selected_id = Some(rows[next].identity.agent_id.clone());
    }

    pub fn render(
        &self,
        snapshot: &Snapshot,
        height: usize,
        width: usize,
        message: &str,
    ) -> Vec<String> {
        if height == 0 || width == 0 {
            return Vec::new();
        }
        let rows = self.rows(snapshot);
        let selected = rows
            .iter()
            .position(|a| Some(&a.identity.agent_id) == self.selected_id.as_ref())
            .unwrap_or(0);
        let mut lines = vec![
            format!(
                "Agent Dashboard  {} agents  rev {}",
                rows.len(),
                snapshot.revision
            ),
            format!(
                "{}  {}",
                if self.pinned_only {
                    "Pinned"
                } else {
                    "All agents"
                },
                message
            ),
            if self.searching || !self.query.is_empty() {
                format!("/{}", self.query)
            } else {
                "  TOOL      STATUS    PROJECT / TASK".into()
            },
        ];
        let capacity = height.saturating_sub(7).max(1);
        let start = selected.saturating_sub(capacity - 1);
        if rows.is_empty() {
            lines.push("No agents found. Start an agent in a Zellij pane.".into());
        } else {
            for (i, agent) in rows.iter().enumerate().skip(start).take(capacity) {
                let status = match agent.liveness {
                    Liveness::Gone => "gone",
                    Liveness::Unverified => "cached",
                    Liveness::Live => agent.status.label(),
                };
                let task = if agent.alias.is_empty() {
                    &agent.summary
                } else {
                    &agent.alias
                };
                lines.push(format!(
                    "{} {} {:<9} {:<9} {}{}  {}",
                    if i == selected { ">" } else { " " },
                    if agent.pinned { "*" } else { " " },
                    agent.tool,
                    status,
                    if agent.parent_id.is_some() {
                        "↳ "
                    } else {
                        ""
                    },
                    agent.project(),
                    task
                ));
            }
        }
        if let Some(agent) = rows.get(selected) {
            lines.push(format!(
                "  {} · pane {} · {}",
                agent.identity.session_name, agent.identity.pane_id, agent.cwd
            ));
            let seen = agent
                .last_report_ms
                .map(|at| format!("last seen {}", age(snapshot.now_ms.saturating_sub(at))))
                .unwrap_or_else(|| "hooks have not reported".into());
            lines.push(format!(
                "  {}{} · {} · {}",
                seen,
                if agent.stale(snapshot.now_ms) {
                    " (stale)"
                } else {
                    ""
                },
                age(snapshot.now_ms.saturating_sub(agent.status_since_ms)),
                agent.detail
            ));
        }
        if let Some(activity) = snapshot.activities.last() {
            lines.push(format!(
                "Recent: {} → {}",
                activity.project,
                activity.status.label()
            ));
        }
        lines.push(
            "j/k ↑↓ select · Enter focus · / search · Tab pinned · R refresh · q close".into(),
        );
        lines
            .into_iter()
            .take(height)
            .map(|line| truncate(&line, width.saturating_sub(1).max(1)))
            .collect()
    }
}

pub fn age(ms: u64) -> String {
    let secs = ms / 1000;
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else {
        format!("{}h", secs / 3600)
    }
}

/// Remove terminal controls from user-controlled data before rendering it.
pub fn truncate(text: &str, width: usize) -> String {
    let clean: String = text.chars().filter(|c| !c.is_control()).collect();
    if clean.width() <= width {
        return clean;
    }
    let mut result = String::new();
    let mut used = 0;
    for c in clean.chars() {
        let columns = c.width().unwrap_or(0);
        if used + columns > width.saturating_sub(1) {
            break;
        }
        result.push(c);
        used += columns;
    }
    if width > 0 {
        result.push('…');
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn populated() -> Snapshot {
        let mut store = crate::Store::default();
        let found: Vec<_> = (0..40)
            .map(|pane| crate::FoundProcess {
                identity: crate::Identity {
                    agent_id: format!("agent-{pane}"),
                    session_name: "한글 세션".into(),
                    session_epoch: "server-run".into(),
                    pane_id: pane,
                    incarnation_id: format!("run-{pane}"),
                    pid: pane + 100,
                    process_started: "start".into(),
                },
                tool: "claude".into(),
                cwd: "/프로젝트/긴 경로".into(),
            })
            .collect();
        store.reconcile(&found, 1000);
        store.snapshot(1000)
    }

    #[test]
    fn selection_survives_urgency_reordering_and_search_uses_identity() {
        let mut snapshot = populated();
        let mut view = View::default();
        view.reconcile_selection(&snapshot);
        view.select(&snapshot, 3);
        let selected = view.selected_id.clone();
        snapshot.agents.last_mut().unwrap().status = crate::Status::Waiting;
        snapshot.agents.reverse();
        view.reconcile_selection(&snapshot);
        assert_eq!(view.selected_id, selected);
        snapshot.agents[0].summary = "검색 대상".into();
        let expected = snapshot.agents[0].identity.agent_id.clone();
        view.query = "검색 대상".into();
        view.reconcile_selection(&snapshot);
        assert_eq!(view.selected_id.as_deref(), Some(expected.as_str()));
        view.query = "no matches".into();
        view.reconcile_selection(&snapshot);
        assert_eq!(view.selected_id, None);
    }

    #[test]
    fn scrolled_selection_is_visible_and_populated_rows_fit_terminal() {
        let snapshot = populated();
        let view = View {
            selected_id: Some("agent-39".into()),
            ..View::default()
        };
        for width in [40, 80, 120] {
            let lines = view.render(&snapshot, 24, width, "connected");
            assert!(lines.iter().any(|l| l.starts_with('>')));
            assert!(lines.len() <= 24);
            assert!(lines.iter().all(|l| l.width() < width));
        }
    }

    #[test]
    fn truncation_respects_korean_columns_and_removes_terminal_controls() {
        let value = truncate("한국어 프로젝트\x1b\n\r", 10);
        assert!(value.width() <= 10);
        assert!(!value.contains('\x1b'));
        assert!(value.ends_with('…'));
    }

    #[test]
    fn small_viewport_never_exceeds_requested_dimensions() {
        let snapshot = crate::Store::default().snapshot(0);
        for height in [1, 3, 10, 24] {
            for width in [1, 40, 80, 120] {
                let lines = View::default().render(&snapshot, height, width, "connecting");
                assert!(lines.len() <= height);
                assert!(lines.iter().all(|l| l.width() <= width));
            }
        }
    }
}
