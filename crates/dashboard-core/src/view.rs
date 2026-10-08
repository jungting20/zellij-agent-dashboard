use crate::{
    text::{wrap, Line},
    Agent, Liveness, PaneOutput, Snapshot, Status, StatusSource,
};
use std::collections::BTreeSet;
use unicode_width::UnicodeWidthStr;

// Match zellij-with-codeagent's terminal palette and panel proportions.
const GREEN: u8 = 42;
const RED: u8 = 196;
const BLUE: u8 = 39;
const ORANGE: u8 = 214;
const SESSION: u8 = 69;
const TAB: u8 = 75;
const MUTED: u8 = 244;
const PINNED: u8 = 220;

#[derive(Default)]
pub struct View {
    pub menu: Option<crate::menu::Menu>,
    pub selected_id: Option<String>,
    pub query: String,
    pub searching: bool,
    /// Focused area, also the visible panel in narrow terminals.
    pub pinned_only: bool,
    pub source_session: String,
    pub output: Option<PaneOutput>,
    pub instruction_open: bool,
    pub instruction_offset: usize,
    instruction_text: String,
    instruction_at: Option<u64>,
    selections: [Option<String>; 2],
}

enum Row<'a> {
    Session(&'a Agent, usize),
    Tab(&'a Agent, usize),
    Agent(&'a Agent, usize),
    Empty,
}

impl View {
    pub fn rows<'a>(&self, snapshot: &'a Snapshot) -> Vec<&'a Agent> {
        let query = self.query.to_lowercase();
        let mut rows: Vec<_> = snapshot
            .agents
            .iter()
            .filter(|a| a.visible())
            .filter(|a| {
                query.is_empty()
                    || format!(
                        "{} {} {} {} {} {} {}",
                        a.tool,
                        a.cwd,
                        a.summary,
                        a.alias,
                        a.identity.session_name,
                        a.pane.tab_name,
                        a.status.label()
                    )
                    .to_lowercase()
                    .contains(&query)
            })
            .collect();
        rows.sort_by_key(|a| {
            let root = hierarchy_root(snapshot, a);
            (
                !root.pinned,
                root.identity.session_name.as_str(),
                root.pane.tab_id,
                root.pane.tab_name.as_str(),
                a.liveness != Liveness::Live,
                a.status.rank(),
                a.identity.pane_id,
                a.identity.agent_id.as_str(),
            )
        });
        // Place descendants immediately after their parent; malformed cycles
        // still appear exactly once and never hang the renderer.
        let mut ordered = Vec::new();
        let mut visited = BTreeSet::new();
        let roots: Vec<_> = rows
            .iter()
            .copied()
            .filter(|a| {
                !rows.iter().any(|p| {
                    a.parent_id.as_deref() == Some(p.identity.agent_id.as_str())
                        && p.identity.agent_id != a.identity.agent_id
                })
            })
            .collect();
        for agent in roots.into_iter().chain(rows.iter().copied()) {
            visit(agent, &rows, &mut ordered, &mut visited);
        }
        ordered
    }

    pub fn panel_rows<'a>(&self, snapshot: &'a Snapshot, pinned: bool) -> Vec<&'a Agent> {
        self.rows(snapshot)
            .into_iter()
            .filter(|a| hierarchy_root(snapshot, a).pinned == pinned)
            .collect()
    }

    pub fn reconcile_selection(&mut self, snapshot: &Snapshot) {
        for pinned in [false, true] {
            let at = usize::from(pinned);
            let rows = self.panel_rows(snapshot, pinned);
            if pinned == self.pinned_only {
                if rows
                    .iter()
                    .any(|a| Some(&a.identity.agent_id) == self.selected_id.as_ref())
                {
                    self.selections[at] = self.selected_id.clone();
                } else {
                    self.selected_id = self.selections[at]
                        .clone()
                        .filter(|id| rows.iter().any(|a| &a.identity.agent_id == id))
                        .or_else(|| rows.first().map(|a| a.identity.agent_id.clone()));
                    self.selections[at] = self.selected_id.clone();
                }
            } else if !rows
                .iter()
                .any(|a| Some(&a.identity.agent_id) == self.selections[at].as_ref())
            {
                self.selections[at] = rows.first().map(|a| a.identity.agent_id.clone());
            }
        }
        if self
            .output
            .as_ref()
            .is_some_and(|o| Some(&o.agent_id) != self.selected_id.as_ref())
        {
            self.output = None;
        }
    }

    pub fn toggle_panel(&mut self, snapshot: &Snapshot) {
        self.focus_panel(snapshot, !self.pinned_only);
    }

    pub fn focus_panel(&mut self, snapshot: &Snapshot, pinned: bool) {
        self.selections[usize::from(self.pinned_only)] = self.selected_id.clone();
        self.pinned_only = pinned;
        self.selected_id = self.selections[usize::from(pinned)].clone();
        self.reconcile_selection(snapshot);
    }

    pub fn select(&mut self, snapshot: &Snapshot, delta: isize) {
        let rows = self.panel_rows(snapshot, self.pinned_only);
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
        self.reconcile_selection(snapshot);
    }

    pub fn select_number(&mut self, snapshot: &Snapshot, number: usize) {
        if let Some(agent) = self.rows(snapshot).get(number) {
            self.selections[usize::from(self.pinned_only)] = self.selected_id.clone();
            self.pinned_only = hierarchy_root(snapshot, agent).pinned;
            self.selected_id = Some(agent.identity.agent_id.clone());
            self.reconcile_selection(snapshot);
        }
    }

    pub fn open_instruction(&mut self, snapshot: &Snapshot) {
        if let Some(agent) = self.selected(snapshot) {
            let text = agent.summary.clone();
            let at = agent.last_instruction_ms;
            self.instruction_text = text;
            self.instruction_at = at;
            self.instruction_open = true;
            self.instruction_offset = 0;
        }
    }

    fn selected<'a>(&self, snapshot: &'a Snapshot) -> Option<&'a Agent> {
        snapshot
            .agents
            .iter()
            .find(|a| Some(&a.identity.agent_id) == self.selected_id.as_ref())
    }

    pub fn render(
        &self,
        snapshot: &Snapshot,
        height: usize,
        width: usize,
        message: &str,
    ) -> Vec<String> {
        self.frame(snapshot, height, width, message)
            .iter()
            .map(Line::render)
            .collect()
    }

    /// Same layout without terminal styles, used for accessible previews and QA.
    pub fn render_plain(
        &self,
        snapshot: &Snapshot,
        height: usize,
        width: usize,
        message: &str,
    ) -> Vec<String> {
        self.frame(snapshot, height, width, message)
            .iter()
            .map(Line::text)
            .collect()
    }

    fn frame(&self, snapshot: &Snapshot, height: usize, width: usize, message: &str) -> Vec<Line> {
        if height == 0 || width == 0 {
            return Vec::new();
        }
        let wide = width >= 100;
        let width = width.saturating_sub(1).max(1);
        let live = snapshot.now_ms.saturating_sub(snapshot.last_scan_ms) < 10_000;
        let all = self.rows(snapshot);
        let mut lines = vec![Line::color("AGENT DASHBOARD", GREEN, true)
            .append(Line::plain("  "))
            .append(Line::color(
                if live { "* LIVE" } else { "! DEGRADED" },
                if live { GREEN } else { RED },
                false,
            ))
            .append(Line::plain(format!("  {} agents", all.len())))];
        if self.searching || !self.query.is_empty() {
            lines.push(Line::color(
                format!("/{}{}", self.query, if self.searching { "▏" } else { "" }),
                TAB,
                false,
            ));
        }
        let activity_count = snapshot
            .activities
            .len()
            .min(3)
            .min(height.saturating_sub(7));
        let activity_height = if activity_count > 0 {
            activity_count + 1
        } else {
            0
        };
        let mut body_height = height
            .saturating_sub(lines.len() + 2 + activity_height)
            .max(1);
        let instruction =
            self.instruction_preview(snapshot, body_height.saturating_sub(3).min(4), width);
        body_height = body_height.saturating_sub(instruction.len()).max(1);
        let required = if wide {
            self.display_rows(snapshot, true)
                .len()
                .max(self.display_rows(snapshot, false).len())
                + 2
        } else {
            self.display_rows(snapshot, self.pinned_only).len() + 2
        };
        let output = self.output_preview(snapshot, body_height.saturating_sub(required), width);
        body_height = body_height.saturating_sub(output.len()).max(1);
        if wide {
            let left_width = width.saturating_sub(3) * 35 / 100;
            let right_width = width.saturating_sub(3 + left_width);
            let left = self.panel(snapshot, true, left_width, body_height);
            let right = self.panel(snapshot, false, right_width, body_height);
            for (left, right) in left.into_iter().zip(right) {
                lines.push(
                    left.fit(left_width, true)
                        .append(Line::plain(" │ "))
                        .append(right.fit(right_width, true)),
                );
            }
        } else {
            lines.extend(self.panel(snapshot, self.pinned_only, width, body_height));
        }
        lines.extend(instruction);
        lines.extend(output);
        if activity_count > 0 {
            lines.push(Line::color("── 최근 상태 변화 ──", MUTED, false));
            for activity in snapshot.activities.iter().rev().take(activity_count) {
                let previous = activity.previous.map(status_korean).unwrap_or("기록 없음");
                lines.push(Line::plain(format!(
                    "{}: {} → {} · {}",
                    activity.project,
                    previous,
                    status_korean(activity.status),
                    relative_age(snapshot.now_ms.saturating_sub(activity.at_ms))
                )));
            }
        }
        let text = if message == "Connected" {
            self.selected(snapshot)
                .map(|a| {
                    let report = a
                        .last_report_ms
                        .map(|at| {
                            format!(
                                "{}{}",
                                relative_age(snapshot.now_ms.saturating_sub(at)),
                                if a.stale(snapshot.now_ms) {
                                    " · stale"
                                } else {
                                    ""
                                }
                            )
                        })
                        .unwrap_or_else(|| "상태 보고 없음".into());
                    let source = match a.status_source {
                        StatusSource::Hook => "훅",
                        StatusSource::Screen => "화면",
                        StatusSource::Unknown => "미확인",
                    };
                    format!(
                        "{} · pane {} · {} · {} · {}",
                        a.identity.session_name, a.identity.pane_id, a.cwd, source, report
                    )
                })
                .unwrap_or_default()
        } else {
            message.into()
        };
        let heading = if text.is_empty() {
            String::new()
        } else {
            format!("── {} ", truncate(&text, width.saturating_sub(6)))
        };
        let rule = format!(
            "{}{}",
            heading,
            "─".repeat(width.saturating_sub(heading.width()))
        );
        let color = if !live || message.to_lowercase().contains("error") {
            RED
        } else {
            MUTED
        };
        let footer = vec![
            Line::color(rule, color, false),
            Line::plain(if width >= 75 {
                "Tab working h/l 영역 p 지시 n 새 실행 i 입력 I 에디터 g worktree m 병합 a 태그 Space pin d 종료 Enter 이동 R 갱신 q 닫기"
            } else {
                "Tab working h/l 영역 Space pin Enter focus / 검색 q quit"
            }),
        ];
        if height < 3 {
            lines.truncate(height.saturating_sub(1));
            lines.push(footer[1].clone());
        } else {
            lines.truncate(height.saturating_sub(2));
            lines.extend(footer);
        }
        if self.instruction_open {
            lines = self.instruction_popup(snapshot, height, width, lines);
        }
        if let Some(menu) = &self.menu {
            lines = menu.overlay(height, width, lines);
        }
        lines
            .into_iter()
            .take(height)
            .map(|line| line.fit(width, false))
            .collect()
    }

    fn display_rows<'a>(&self, snapshot: &'a Snapshot, pinned: bool) -> Vec<Row<'a>> {
        let all = self.rows(snapshot);
        let agents: Vec<_> = all
            .iter()
            .enumerate()
            .filter(|(_, a)| hierarchy_root(snapshot, a).pinned == pinned)
            .collect();
        if agents.is_empty() {
            return vec![Row::Empty];
        }
        let mut rows = Vec::new();
        let mut at = 0;
        while at < agents.len() {
            let root = hierarchy_root(snapshot, agents[at].1);
            let session_end = if pinned {
                agents.len()
            } else {
                let mut end = at + 1;
                while end < agents.len()
                    && hierarchy_root(snapshot, agents[end].1)
                        .identity
                        .session_name
                        == root.identity.session_name
                {
                    end += 1;
                }
                end
            };
            if !pinned && root.identity.session_name != "worktree-agent" {
                rows.push(Row::Session(root, session_end - at));
            }
            let mut tab_at = at;
            while tab_at < session_end {
                let tab_root = hierarchy_root(snapshot, agents[tab_at].1);
                let mut end = tab_at + 1;
                while end < session_end {
                    let other = hierarchy_root(snapshot, agents[end].1);
                    if !pinned
                        && (other.pane.tab_id != tab_root.pane.tab_id
                            || other.pane.tab_name != tab_root.pane.tab_name)
                    {
                        break;
                    }
                    end += 1;
                }
                if !pinned
                    && root.identity.session_name != "worktree-agent"
                    && !tab_root.pane.tab_name.is_empty()
                {
                    rows.push(Row::Tab(tab_root, end - tab_at));
                }
                for (index, agent) in &agents[tab_at..end] {
                    rows.push(Row::Agent(agent, *index));
                }
                tab_at = end;
            }
            at = session_end;
        }
        rows
    }

    fn panel(&self, snapshot: &Snapshot, pinned: bool, width: usize, height: usize) -> Vec<Line> {
        let label = format!(
            "── {} ({}) ",
            if pinned { "PINNED" } else { "UNPINNED" },
            self.panel_rows(snapshot, pinned).len()
        );
        let mut heading = Line::color(
            format!(
                "{}{}",
                label,
                "─".repeat(width.saturating_sub(label.width()))
            ),
            if pinned { PINNED } else { TAB },
            true,
        );
        heading.reverse = pinned == self.pinned_only;
        let mut lines = vec![heading.fit(width, false)];
        if height > 1 {
            lines.push(Line::plain("PIN  PROJECT  STATE  AGENT  SINCE"));
        }
        let rows = self.display_rows(snapshot, pinned);
        let selection = if pinned == self.pinned_only {
            self.selected_id.as_ref()
        } else {
            self.selections[usize::from(pinned)].as_ref()
        };
        let selected = rows
            .iter()
            .position(|r| matches!(r, Row::Agent(a, _) if Some(&a.identity.agent_id) == selection))
            .unwrap_or(0);
        let visible = height.saturating_sub(lines.len());
        let start = selected
            .saturating_sub(visible.saturating_sub(1))
            .min(rows.len().saturating_sub(visible));
        for row in rows.iter().skip(start).take(visible) {
            lines.push(match row {
                Row::Session(agent, count) => Line::color(
                    format!(
                        "{} ({}){}",
                        agent.identity.session_name,
                        count,
                        if agent.identity.session_name == self.source_session {
                            "  current"
                        } else {
                            ""
                        }
                    ),
                    SESSION,
                    true,
                ),
                Row::Tab(agent, count) => {
                    Line::color(format!("  {} ({})", agent.pane.tab_name, count), TAB, false)
                }
                Row::Empty => Line::color(
                    if self.query.is_empty() {
                        "    No agents"
                    } else {
                        "    No matching agents"
                    },
                    MUTED,
                    false,
                ),
                Row::Agent(agent, index) => self.agent_row(
                    snapshot,
                    agent,
                    width,
                    *index,
                    pinned == self.pinned_only && Some(&agent.identity.agent_id) == selection,
                ),
            });
        }
        lines.resize_with(height, Line::default);
        lines.into_iter().map(|l| l.fit(width, false)).collect()
    }

    fn agent_row(
        &self,
        snapshot: &Snapshot,
        agent: &Agent,
        width: usize,
        index: usize,
        selected: bool,
    ) -> Line {
        let number = if index < 9 {
            format!("{} ", index + 1)
        } else {
            "  ".into()
        };
        let mut project = format!(
            "{}{}{}",
            child_label(snapshot, agent),
            if agent.alias.is_empty() {
                String::new()
            } else {
                format!("[{}] ", agent.alias)
            },
            agent.project()
        );
        if project == "unknown" {
            project = "-".into();
        }
        let project_width = width.saturating_sub(39).max(8);
        let (status, color) = status_label(agent);
        let mut line = Line::plain(format!(
            "{} {}{} ",
            if selected { ">" } else { " " },
            number,
            if agent.pinned { "*" } else { " " }
        ))
        .append(Line::plain(project).fit(project_width, true))
        .append(Line::plain("  "))
        .append(Line::color(status, color, false).fit(10, true))
        .append(Line::plain("  "))
        .append(Line::plain(agent_name(&agent.tool)).fit(12, true))
        .append(Line::plain(format!(
            "  {}",
            elapsed(snapshot.now_ms.saturating_sub(agent.status_since_ms))
        )));
        line.reverse = selected;
        line.fit(width, true)
    }

    fn instruction_preview(
        &self,
        snapshot: &Snapshot,
        available: usize,
        width: usize,
    ) -> Vec<Line> {
        if available < 2 {
            return Vec::new();
        }
        let Some(agent) = self.selected(snapshot).filter(|a| !a.summary.is_empty()) else {
            return Vec::new();
        };
        let at = agent
            .last_instruction_ms
            .or(agent.last_report_ms)
            .unwrap_or(snapshot.now_ms);
        let mut lines = vec![Line::color(
            format!(
                "── 마지막 지시 · {} · p 전체 보기 ──",
                relative_age(snapshot.now_ms.saturating_sub(at))
            ),
            MUTED,
            false,
        )];
        let wrapped = wrap(&agent.summary, width);
        let count = wrapped.len().min(available - 1);
        for (i, text) in wrapped.iter().take(count).enumerate() {
            let text = if i + 1 == count && count < wrapped.len() {
                format!("{}…", truncate(text, width.saturating_sub(1)))
            } else {
                text.clone()
            };
            lines.push(Line::plain(text));
        }
        lines
    }

    fn output_preview(&self, snapshot: &Snapshot, available: usize, width: usize) -> Vec<Line> {
        if available < 2 {
            return Vec::new();
        }
        let Some(agent) = self.selected(snapshot) else {
            return Vec::new();
        };
        let text = self
            .output
            .as_ref()
            .filter(|o| o.agent_id == agent.identity.agent_id)
            .map(|o| o.text.trim_end())
            .unwrap_or("");
        if text.is_empty() {
            return vec![
                Line::color("── Pane 출력 ──", MUTED, false),
                Line::color("아직 수집된 출력이 없습니다", MUTED, false),
            ];
        }
        let wrapped = wrap(text, width);
        let count = wrapped.len().min(available - 1).min(20);
        let mut lines = vec![Line::color(
            format!(
                "── Pane 출력 · {} · 마지막 {}줄 ──",
                agent_name(&agent.tool),
                count
            ),
            MUTED,
            false,
        )];
        lines.extend(wrapped.iter().skip(wrapped.len() - count).map(Line::plain));
        lines
    }

    fn instruction_popup(
        &self,
        snapshot: &Snapshot,
        height: usize,
        width: usize,
        mut base: Vec<Line>,
    ) -> Vec<Line> {
        if width < 8 || height < 5 {
            return base;
        }
        let inner = width.saturating_sub(6).max(1);
        let visible = height.saturating_sub(7).max(1);
        let text = if self.instruction_text.is_empty() {
            "아직 저장된 지시가 없습니다"
        } else {
            &self.instruction_text
        };
        let wrapped = wrap(text, inner);
        let offset = self
            .instruction_offset
            .min(wrapped.len().saturating_sub(visible));
        let age = self
            .instruction_at
            .map(|at| relative_age(snapshot.now_ms.saturating_sub(at)))
            .unwrap_or_else(|| "시간 정보 없음".into());
        let mut popup = vec![Line::color(
            format!("╭{}╮", "─".repeat(inner + 2)),
            MUTED,
            false,
        )];
        let border = |line: Line| {
            Line::color("│ ", MUTED, false)
                .append(line.fit(inner, true))
                .append(Line::color(" │", MUTED, false))
        };
        popup.push(border(Line::plain(format!("마지막 지시 · {age}"))));
        for i in 0..visible {
            popup.push(border(Line::plain(
                wrapped.get(offset + i).map(String::as_str).unwrap_or(""),
            )));
        }
        popup.push(border(Line::plain("↑/↓ j/k 스크롤 · p/Esc 닫기")));
        popup.push(Line::color(
            format!("╰{}╯", "─".repeat(inner + 2)),
            MUTED,
            false,
        ));
        for (i, line) in popup.into_iter().take(height).enumerate() {
            if let Some(target) = base.get_mut(i) {
                *target = line;
            } else {
                base.push(line);
            }
        }
        base
    }
}

fn visit<'a>(
    agent: &'a Agent,
    rows: &[&'a Agent],
    ordered: &mut Vec<&'a Agent>,
    visited: &mut BTreeSet<String>,
) {
    if !visited.insert(agent.identity.agent_id.clone()) {
        return;
    }
    ordered.push(agent);
    for child in rows
        .iter()
        .copied()
        .filter(|a| a.parent_id.as_deref() == Some(agent.identity.agent_id.as_str()))
    {
        visit(child, rows, ordered, visited);
    }
}

pub(crate) fn hierarchy_root<'a>(snapshot: &'a Snapshot, mut agent: &'a Agent) -> &'a Agent {
    let mut seen = BTreeSet::new();
    while let Some(parent) = agent.parent_id.as_ref() {
        if !seen.insert(agent.identity.agent_id.as_str()) {
            return snapshot
                .agents
                .iter()
                .filter(|a| seen.contains(a.identity.agent_id.as_str()))
                .min_by_key(|a| &a.identity.agent_id)
                .unwrap_or(agent);
        }
        let Some(next) = snapshot
            .agents
            .iter()
            .find(|a| &a.identity.agent_id == parent && a.visible())
        else {
            break;
        };
        agent = next;
    }
    agent
}

fn child_label<'a>(snapshot: &'a Snapshot, mut agent: &'a Agent) -> String {
    let mut depth = 0;
    let mut seen = BTreeSet::new();
    while let Some(parent) = agent.parent_id.as_ref() {
        if !seen.insert(agent.identity.agent_id.as_str()) {
            break;
        }
        depth += 1;
        let Some(next) = snapshot
            .agents
            .iter()
            .find(|a| &a.identity.agent_id == parent && a.visible())
        else {
            break;
        };
        agent = next;
    }
    if depth == 0 {
        String::new()
    } else {
        format!("{}↳ ", "  ".repeat(depth.min(4)))
    }
}

fn status_label(agent: &Agent) -> (&'static str, u8) {
    match agent.liveness {
        Liveness::Gone => ("× gone", MUTED),
        Liveness::Unverified => ("? cached", ORANGE),
        Liveness::Live => match agent.status {
            Status::Working => ("● working", GREEN),
            Status::Waiting => ("! waiting", RED),
            Status::Failed => ("! failed", RED),
            Status::Idle => ("○ idle", BLUE),
            Status::Done => ("✓ done", BLUE),
            Status::Compact => ("● compact", GREEN),
            Status::Found => ("? found", ORANGE),
        },
    }
}

fn agent_name(tool: &str) -> &str {
    match tool {
        "claude" => "Claude",
        "codex" => "Codex",
        "cursor" => "Cursor",
        "gemini" => "Gemini",
        "hermes" => "Hermes",
        "pi" => "Pi",
        _ => tool,
    }
}
fn status_korean(status: Status) -> &'static str {
    match status {
        Status::Working => "작업 중",
        Status::Waiting => "확인 필요",
        Status::Failed => "실패",
        Status::Idle => "입력 대기",
        Status::Done => "완료",
        Status::Compact => "압축 중",
        Status::Found => "발견",
    }
}
fn relative_age(ms: u64) -> String {
    if ms < 60_000 {
        "방금 전".into()
    } else if ms < 3_600_000 {
        format!("{}분 전", ms / 60_000)
    } else if ms < 86_400_000 {
        format!("{}시간 전", ms / 3_600_000)
    } else {
        format!("{}일 전", ms / 86_400_000)
    }
}
fn elapsed(ms: u64) -> String {
    let s = ms / 1000;
    if s < 3600 {
        format!("{:02}:{:02}", s / 60, s % 60)
    } else {
        format!("{:02}:{:02}", s / 3600, s / 60 % 60)
    }
}
pub fn age(ms: u64) -> String {
    let s = ms / 1000;
    if s < 60 {
        format!("{s}s")
    } else if s < 3600 {
        format!("{}m", s / 60)
    } else {
        format!("{}h", s / 3600)
    }
}
pub fn truncate(text: &str, width: usize) -> String {
    Line::plain(text).fit(width, false).text()
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
        for agent in store.data.agents.values_mut() {
            agent.pane.presence = crate::PanePresence::Present;
            agent.pane.observed_at_ms = 1000;
        }
        store.snapshot(1000)
    }

    #[test]
    fn working_cycle_crosses_panels_wraps_and_preserves_empty_selection() {
        let mut snapshot = populated();
        for a in &mut snapshot.agents {
            a.status = Status::Idle;
        }
        snapshot.agents[0].pinned = true;
        snapshot.agents[0].status = Status::Working;
        snapshot.agents[2].status = Status::Working;
        snapshot.agents[3].status = Status::Working;
        snapshot.agents[3].liveness = Liveness::Unverified;
        let pinned = snapshot.agents[0].identity.agent_id.clone();
        let unpinned = snapshot.agents[2].identity.agent_id.clone();
        let mut view = View::default();
        view.select_next_working(&snapshot);
        assert_eq!(view.selected_id.as_ref(), Some(&pinned));
        assert!(view.pinned_only);
        view.select_next_working(&snapshot);
        assert_eq!(view.selected_id.as_ref(), Some(&unpinned));
        assert!(!view.pinned_only);
        view.select_next_working(&snapshot);
        assert_eq!(view.selected_id.as_ref(), Some(&pinned));
        view.query = "no match".into();
        view.select_next_working(&snapshot);
        assert_eq!(view.selected_id.as_ref(), Some(&pinned));
        view.query.clear();
        snapshot.agents[0].status = Status::Idle;
        snapshot.agents[2].status = Status::Idle;
        view.select_next_working(&snapshot);
        assert_eq!(view.selected_id.as_ref(), Some(&pinned));
    }

    #[test]
    fn next_filters_use_live_panes_exact_idle_and_inherited_pin() {
        let mut snapshot = populated();
        snapshot.agents.truncate(5);
        for a in &mut snapshot.agents {
            a.status = Status::Idle;
        }
        let parent = snapshot.agents[0].identity.agent_id.clone();
        let child = snapshot.agents[1].identity.agent_id.clone();
        snapshot.agents[0].pinned = true;
        snapshot.agents[1].parent_id = Some(parent.clone());
        snapshot.agents[2].status = Status::Done;
        snapshot.agents[3].pane.presence = crate::PanePresence::Missing;
        snapshot.agents[4].liveness = Liveness::Gone;
        let view = View::default();
        use crate::NextFilter as F;
        assert_eq!(
            view.next_agent(&snapshot, F::IdleAndPinned, Some(&parent))
                .unwrap()
                .identity
                .agent_id,
            child
        );
        assert_eq!(
            view.next_agent(&snapshot, F::PinnedOnly, Some(&child))
                .unwrap()
                .identity
                .agent_id,
            parent
        );
        assert!(view
            .next_agent(&snapshot, F::IdleAndUnpinned, None)
            .is_none());
        assert_eq!(
            view.next_agent(&snapshot, F::UnpinnedOnly, None)
                .unwrap()
                .status,
            Status::Done
        );
        snapshot.agents[0].ended = true;
        assert_eq!(
            view.next_agent(&snapshot, F::IdleAndUnpinned, Some("deleted-cursor"))
                .unwrap()
                .identity
                .agent_id,
            child
        );
        assert!(F::parse("unexpected").is_err());
        assert_eq!(F::parse("working-only").unwrap(), F::WorkingOnly);
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
            let lines = view.render_plain(&snapshot, 24, width, "Connected");
            assert!(lines.iter().any(|l| l.contains('>')));
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
                let lines = View::default().render_plain(&snapshot, height, width, "connecting");
                assert!(lines.len() <= height);
                assert!(lines.iter().all(|l| l.width() <= width));
            }
        }
    }

    #[test]
    fn both_panels_group_sessions_and_tabs_and_remember_independent_selection() {
        let mut snapshot = populated();
        snapshot.agents[0].pinned = true;
        snapshot.agents[1].pinned = true;
        for a in &mut snapshot.agents {
            a.pane.tab_id = Some(1);
            a.pane.tab_name = "개발 탭".into();
        }
        let mut view = View::default();
        view.reconcile_selection(&snapshot);
        view.select(&snapshot, 3);
        let unpinned = view.selected_id.clone();
        view.toggle_panel(&snapshot);
        view.select(&snapshot, 1);
        let pinned = view.selected_id.clone();
        view.focus_panel(&snapshot, false);
        assert_eq!(view.selected_id, unpinned);
        view.focus_panel(&snapshot, false);
        assert_eq!(view.selected_id, unpinned);
        view.focus_panel(&snapshot, true);
        assert_eq!(view.selected_id, pinned);
        view.focus_panel(&snapshot, true);
        assert_eq!(view.selected_id, pinned);
        let wide = view
            .render_plain(&snapshot, 24, 120, "Connected")
            .join("\n");
        assert!(wide.contains("PINNED (2)"));
        assert!(wide.contains("UNPINNED (38)"));
        assert!(wide.contains(" │ "));
        assert!(wide.contains("개발 탭"));
        let narrow = view.render_plain(&snapshot, 24, 80, "Connected").join("\n");
        assert!(narrow.contains("PINNED (2)"));
        assert!(!narrow.contains("UNPINNED"));
    }

    #[test]
    fn cycles_render_once_and_multiline_instruction_and_output_fit() {
        let mut snapshot = populated();
        let a = snapshot.agents[0].identity.agent_id.clone();
        let b = snapshot.agents[1].identity.agent_id.clone();
        snapshot.agents[0].parent_id = Some(b);
        snapshot.agents[1].parent_id = Some(a.clone());
        snapshot.agents[0].summary = "한글과 👩‍💻\n두 번째 줄\x1b[31m".into();
        let mut view = View {
            selected_id: Some(a),
            ..View::default()
        };
        view.reconcile_selection(&snapshot);
        assert_eq!(view.rows(&snapshot).len(), snapshot.agents.len());
        for height in [0, 1, 3, 10, 24, 40] {
            for width in [0, 1, 40, 80, 100, 120] {
                for line in view.render_plain(&snapshot, height, width, "Connected") {
                    assert!(!line.contains('\x1b'));
                    assert!(line.width() <= width);
                }
            }
        }
        view.open_instruction(&snapshot);
        let popup = view.render_plain(&snapshot, 24, 80, "Connected").join("\n");
        assert!(popup.contains("마지막 지시"));
        assert!(popup.contains("두 번째 줄"));
    }

    #[test]
    fn only_reachable_current_agents_appear_and_hidden_parents_do_not_pin_children() {
        let mut snapshot = populated();
        let parent = snapshot.agents[0].identity.agent_id.clone();
        snapshot.agents[0].pinned = true;
        snapshot.agents[0].liveness = Liveness::Gone;
        snapshot.agents[1].parent_id = Some(parent.clone());
        snapshot.agents[2].pane.presence = crate::PanePresence::Missing;
        snapshot.agents[3].pane.presence = crate::PanePresence::Unknown;
        snapshot.agents[4].ended = true;
        snapshot.agents[5].liveness = Liveness::Unverified;
        snapshot.agents[6].status = Status::Done;
        let mut view = View {
            selected_id: Some(parent),
            ..View::default()
        };
        view.reconcile_selection(&snapshot);
        assert_eq!(view.rows(&snapshot).len(), snapshot.agents.len() - 4);
        assert!(view.panel_rows(&snapshot, true).is_empty());
        assert_eq!(
            view.selected_id.as_ref(),
            Some(&view.rows(&snapshot)[0].identity.agent_id)
        );
        assert!(view
            .panel_rows(&snapshot, false)
            .iter()
            .any(|a| a.identity == snapshot.agents[1].identity));
        assert!(view
            .rows(&snapshot)
            .iter()
            .any(|a| a.liveness == Liveness::Unverified));
        assert!(view
            .rows(&snapshot)
            .iter()
            .any(|a| a.status == Status::Done));
        view.query = "old deleted pane".into();
        snapshot.agents[0].alias = view.query.clone();
        assert!(view.rows(&snapshot).is_empty());
    }
}
