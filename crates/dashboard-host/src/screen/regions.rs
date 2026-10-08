//! Region semantics ported from zellij-with-codeagent/internal/codingagent/regions.go.
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Region {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub lines: usize,
}

fn prompt(line: &str) -> bool {
    let mut chars = line.trim().chars();
    matches!(chars.next(), Some('›' | '»'))
        && chars
            .next()
            .is_none_or(|c| c.is_whitespace() || ('\u{2800}'..='\u{28ff}').contains(&c))
}

fn horizontal(line: &str) -> bool {
    let line = line.trim();
    line.chars().all(|c| "─━-=╭╮╰╯├┤┬┴┼┌┐└┘".contains(c))
        && line.chars().filter(|c| "─━-=".contains(*c)).count() >= 3
}

impl Region {
    pub fn validate(&self) -> Result<(), String> {
        match self.kind.as_str() {
            "whole_recent"
            | "after_last_prompt_marker"
            | "before_current_prompt_marker"
            | "current_prompt_marker"
            | "prompt_box_body"
            | "after_last_horizontal_rule"
            | "osc_title"
            | "osc_progress" => Ok(()),
            "bottom_non_empty_lines" if self.lines > 0 => Ok(()),
            _ => Err(format!("invalid screen region {}", self.kind)),
        }
    }

    pub fn select(&self, screen: &str, title: &str) -> String {
        let lines: Vec<_> = screen.split('\n').collect();
        let last_prompt = lines.iter().rposition(|line| prompt(line));
        let current_prompt = last_prompt.filter(|i| {
            !lines[i + 1..].iter().any(|line| {
                line.trim()
                    .chars()
                    .next()
                    .is_some_and(|c| "•◦■✗✓─".contains(c))
            })
        });
        match self.kind.as_str() {
            "whole_recent" => screen.into(),
            "osc_title" => title.into(),
            // dump-screen/list-panes do not expose OSC progress.
            "osc_progress" => String::new(),
            "after_last_prompt_marker" => {
                last_prompt.map_or_else(|| screen.into(), |i| lines[i + 1..].join("\n"))
            }
            "before_current_prompt_marker" => {
                current_prompt.map_or_else(|| screen.into(), |i| lines[..i].join("\n"))
            }
            "current_prompt_marker" => current_prompt.map_or_else(String::new, |i| lines[i].into()),
            "bottom_non_empty_lines" => {
                let start = lines
                    .iter()
                    .enumerate()
                    .rev()
                    .filter(|(_, line)| !line.trim().is_empty())
                    .nth(self.lines.saturating_sub(1))
                    .map_or(0, |(i, _)| i);
                lines[start..].join("\n")
            }
            "after_last_horizontal_rule" => lines
                .iter()
                .rposition(|line| horizontal(line))
                .map_or_else(|| screen.into(), |i| lines[i + 1..].join("\n")),
            "prompt_box_body" => {
                if let Some(upper) = lines
                    .iter()
                    .rposition(|line| line.contains('╭') && horizontal(line))
                {
                    let lower = (upper + 1..lines.len())
                        .find(|i| horizontal(lines[*i]))
                        .unwrap_or(lines.len());
                    lines[upper + 1..lower].join("\n")
                } else {
                    screen.into()
                }
            }
            _ => String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_regions_handle_braille_and_reject_quoted_markers_and_submitted_prompts() {
        let current = Region {
            kind: "current_prompt_marker".into(),
            lines: 0,
        };
        assert_eq!(
            current.select("output mentions › symbol\n»⠋ next", ""),
            "»⠋ next"
        );
        assert_eq!(current.select("› submitted\n• response", ""), "");
        let after = Region {
            kind: "after_last_prompt_marker".into(),
            lines: 0,
        };
        assert_eq!(after.select("old blocker\n› new\nlatest", ""), "latest");
    }

    #[test]
    fn prompt_boxes_bottom_lines_and_horizontal_boundaries_keep_only_live_regions() {
        let boxed = Region {
            kind: "prompt_box_body".into(),
            lines: 0,
        };
        assert_eq!(
            boxed.select("old\n╭───╮\n│ ❯ now\n╰───╯\nfooter", ""),
            "│ ❯ now"
        );
        let bottom = Region {
            kind: "bottom_non_empty_lines".into(),
            lines: 2,
        };
        assert_eq!(
            bottom.select("old\n\nfirst\n\nlast\n", ""),
            "first\n\nlast\n"
        );
        let after = Region {
            kind: "after_last_horizontal_rule".into(),
            lines: 0,
        };
        assert_eq!(after.select("old\n───\ncurrent", ""), "current");
    }
}
