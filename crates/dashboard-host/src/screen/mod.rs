//! Screen adapter ported from zellij-with-codeagent's codingagent detector.
//! See docs/screen-adapter-provenance.md for source and licensing information.
mod regions;

use dashboard_core::{Agent, StateSignal, Status, StatusObservation};
use regex::Regex;
use regions::Region;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    version: u32,
    agent: String,
    #[serde(default)]
    preserve_state_on_no_match: bool,
    rules: Vec<Rule>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Rule {
    id: String,
    priority: i32,
    #[serde(default)]
    state: String,
    region: Region,
    #[serde(rename = "match")]
    matcher: RawMatcher,
    #[serde(default)]
    visible_idle: bool,
    #[serde(default)]
    visible_working: bool,
    #[serde(default)]
    visible_blocker: bool,
    #[serde(default)]
    skip_state_update: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum RawMatcher {
    Contains(Vec<String>),
    Regex(Vec<String>),
    LineRegex(Vec<String>),
    All(Vec<RawMatcher>),
    Any(Vec<RawMatcher>),
    Not(Vec<RawMatcher>),
}

enum Matcher {
    Contains(Vec<String>),
    Regex(Vec<Regex>, bool),
    Children(Vec<Matcher>, &'static str),
}

impl RawMatcher {
    fn compile(self) -> Result<Matcher, String> {
        match self {
            Self::Contains(values) => {
                if values.is_empty() || values.iter().any(String::is_empty) {
                    return Err("empty contains matcher".into());
                }
                Ok(Matcher::Contains(
                    values.into_iter().map(|v| v.to_lowercase()).collect(),
                ))
            }
            Self::Regex(values) => compile_regex(values, false),
            Self::LineRegex(values) => compile_regex(values, true),
            Self::All(children) => compile_children(children, "all"),
            Self::Any(children) => compile_children(children, "any"),
            Self::Not(children) => compile_children(children, "not"),
        }
    }
}

fn compile_regex(values: Vec<String>, lines: bool) -> Result<Matcher, String> {
    if values.is_empty() || values.iter().any(String::is_empty) {
        return Err("empty regex matcher".into());
    }
    Ok(Matcher::Regex(
        values
            .iter()
            .map(|v| Regex::new(v).map_err(|e| e.to_string()))
            .collect::<Result<_, _>>()?,
        lines,
    ))
}

fn compile_children(children: Vec<RawMatcher>, op: &'static str) -> Result<Matcher, String> {
    if children.is_empty() {
        return Err("empty compound matcher".into());
    }
    Ok(Matcher::Children(
        children
            .into_iter()
            .map(RawMatcher::compile)
            .collect::<Result<_, _>>()?,
        op,
    ))
}

impl Matcher {
    fn matches(&self, text: &str) -> bool {
        match self {
            Self::Contains(values) => {
                let lower = text.to_lowercase();
                values.iter().all(|v| lower.contains(v))
            }
            Self::Regex(expressions, lines) => expressions.iter().all(|re| {
                if *lines {
                    text.split('\n').any(|line| re.is_match(line))
                } else {
                    re.is_match(text)
                }
            }),
            Self::Children(children, "all") => children.iter().all(|c| c.matches(text)),
            Self::Children(children, "any") => children.iter().any(|c| c.matches(text)),
            Self::Children(children, "not") => children.iter().all(|c| !c.matches(text)),
            _ => false,
        }
    }
}

struct CompiledRule {
    id: String,
    status: Option<Status>,
    region: Region,
    matcher: Matcher,
    visible_idle: bool,
}

pub struct Detector {
    profiles: BTreeMap<String, (Vec<CompiledRule>, bool)>,
}

fn status(state: &str) -> Result<Status, String> {
    match state {
        "working" => Ok(Status::Working),
        "blocked" => Ok(Status::Waiting),
        "idle" => Ok(Status::Idle),
        "unknown" => Ok(Status::Found),
        _ => Err(format!("invalid screen state {state}")),
    }
}

impl Detector {
    pub fn embedded() -> Result<Self, String> {
        let mut profiles = BTreeMap::new();
        for (tool, source) in [
            ("claude", include_str!("manifests/claude.json")),
            ("codex", include_str!("manifests/codex.json")),
            ("gemini", include_str!("manifests/gemini.json")),
            ("cursor", include_str!("manifests/cursor.json")),
        ] {
            let mut manifest: Manifest =
                serde_json::from_str(source).map_err(|e| format!("{tool}: {e}"))?;
            if manifest.version != 1 || manifest.agent != tool || manifest.rules.is_empty() {
                return Err(format!("invalid {tool} manifest"));
            }
            manifest.rules.sort_by(|a, b| b.priority.cmp(&a.priority));
            let mut ids = BTreeSet::new();
            let mut rules = Vec::new();
            for rule in manifest.rules {
                if rule.id.is_empty() || !ids.insert(rule.id.clone()) {
                    return Err("empty/duplicate rule ID".into());
                }
                rule.region.validate()?;
                let state = if rule.skip_state_update {
                    None
                } else {
                    Some(status(&rule.state)?)
                };
                // Parsed for manifest compatibility; status and visible_idle drive
                // this collector's policy rather than the old runtime's timers.
                let _ = (rule.visible_working, rule.visible_blocker);
                rules.push(CompiledRule {
                    id: rule.id,
                    status: state,
                    region: rule.region,
                    matcher: rule.matcher.compile()?,
                    visible_idle: rule.visible_idle,
                });
            }
            profiles.insert(tool.into(), (rules, manifest.preserve_state_on_no_match));
        }
        Ok(Self { profiles })
    }

    pub fn supports(&self, tool: &str) -> bool {
        self.profiles.contains_key(tool)
    }

    pub fn signal(
        &self,
        agent: &Agent,
        screen: &str,
        pane_title: &str,
        at: u64,
    ) -> Option<StateSignal> {
        let (rules, preserve) = self.profiles.get(&agent.tool)?;
        // A manually assigned pane title cannot establish Idle. Only active
        // Working/Waiting title rules participate, as in the source monitor.
        let title = if rules.iter().any(|r| {
            r.region.kind == "osc_title"
                && matches!(r.status, Some(Status::Working | Status::Waiting))
                && r.matcher.matches(pane_title)
        }) {
            pane_title
        } else {
            ""
        };
        let matched = rules
            .iter()
            .find(|r| r.matcher.matches(&r.region.select(screen, title)));
        let (state, visible_idle, rule_id) = matched.map_or_else(
            || {
                (
                    if *preserve { None } else { Some(Status::Idle) },
                    false,
                    "default_known_agent_idle_fallback".to_owned(),
                )
            },
            |r| (r.status, r.visible_idle, r.id.clone()),
        );
        Some(
            StatusObservation {
                identity: agent.identity.clone(),
                tool: agent.tool.clone(),
                observation_id: uuid::Uuid::new_v4().to_string(),
                observed_at_ms: at,
                status: state,
                visible_idle,
                rule_id,
            }
            .into(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dashboard_core::{FoundProcess, Identity, Store};

    fn detect(tool: &str, screen: &str, title: &str) -> StatusObservation {
        let mut store = Store::default();
        store.reconcile(
            &[FoundProcess {
                identity: Identity {
                    agent_id: "run".into(),
                    session_name: "dev".into(),
                    session_epoch: "s".into(),
                    pane_id: 1,
                    incarnation_id: "i".into(),
                    pid: 1,
                    process_started: "p".into(),
                },
                tool: tool.into(),
                cwd: "/project".into(),
            }],
            1000,
        );
        let StateSignal::Screen(observation) = Detector::embedded()
            .unwrap()
            .signal(&store.agents["run"], screen, title, 5000)
            .unwrap()
        else {
            panic!("expected screen signal")
        };
        observation
    }

    #[test]
    fn embedded_rules_compile_and_all_supported_tools_detect_active_screens() {
        for (tool, screen, title, expected, rule) in [
            (
                "codex",
                "• Thinking (3s • esc to interrupt)\n› ",
                "",
                Status::Working,
                "screen_working_fallback",
            ),
            (
                "codex",
                "› task\nAllow command?",
                "",
                Status::Waiting,
                "live_strong_blocker",
            ),
            (
                "codex",
                "› ",
                "manually named pane",
                Status::Idle,
                "screen_prompt_idle",
            ),
            (
                "codex",
                "output",
                "Action Required",
                Status::Waiting,
                "osc_title_blocked",
            ),
            (
                "claude",
                "output",
                "⠋ Thinking",
                Status::Working,
                "osc_title_working",
            ),
            (
                "claude",
                "╭────╮\n│ ❯ \n╰────╯",
                "",
                Status::Idle,
                "live_prompt_box",
            ),
            (
                "claude",
                "────\nEnter to select · Esc to cancel\nArrow keys to navigate",
                "",
                Status::Waiting,
                "live_blocked_form",
            ),
            (
                "gemini",
                "│ Allow execution",
                "",
                Status::Waiting,
                "apply_or_allow_change",
            ),
            (
                "gemini",
                "esc to cancel",
                "",
                Status::Working,
                "esc_cancel_working",
            ),
            (
                "cursor",
                "Write to this file?\nProceed (y)\nReject & propose changes",
                "",
                Status::Waiting,
                "write_file_approval",
            ),
            (
                "cursor",
                "ctrl+c to stop",
                "",
                Status::Working,
                "stop_hint_working",
            ),
            (
                "cursor",
                "2 background tasks",
                "",
                Status::Working,
                "background_task_status_working",
            ),
        ] {
            let observation = detect(tool, screen, title);
            assert_eq!(observation.status, Some(expected), "{tool}: {screen}");
            assert_eq!(observation.rule_id, rule);
        }
    }

    #[test]
    fn transcript_overlays_preserve_state_and_historical_blockers_are_excluded() {
        let overlay = detect("codex", "› \n↑/↓ to scroll · PgUp/PgDn to scroll · Home/End to jump · q to quit · esc to edit prev", "");
        assert_eq!(overlay.status, None);
        assert_eq!(overlay.rule_id, "transcript_viewer");
        let overlay = detect(
            "claude",
            "Showing detailed transcript\nctrl+o to toggle",
            "",
        );
        assert_eq!(overlay.status, None);
        let current = detect("codex", "Allow command?\n› new task", "");
        assert_eq!(current.status, Some(Status::Idle));
        assert!(current.visible_idle);
    }

    #[test]
    fn plain_titles_cannot_prove_idle_and_historical_timers_cannot_prove_working() {
        let plain = detect("codex", "ordinary output", "task name");
        assert_eq!(plain.rule_id, "default_known_agent_idle_fallback");
        assert!(!plain.visible_idle);
        let historical = detect(
            "codex",
            "• Thinking (3s • esc to interrupt)\n• Finished response\n› ",
            "",
        );
        assert_eq!(historical.status, Some(Status::Idle));
        let unsupported = Detector::embedded().unwrap();
        assert!(!unsupported.supports("hermes"));
    }
}
