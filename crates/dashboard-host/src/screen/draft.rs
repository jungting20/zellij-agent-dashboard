//! Composer observations are independent of task status. Never retain draft text.
use regex::Regex;
use std::sync::LazyLock;

static ANSI: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\x1b\[[0-?]*[ -/]*[@-~]").unwrap());

pub fn plain(screen: &str) -> String {
    ANSI.replace_all(screen, "").into_owned()
}

// Placeholder text is dim in Codex/Claude. Remove dim glyphs while preserving
// columns so a placeholder cannot look like an unsent message (even translated).
fn without_dim(screen: &str) -> String {
    let mut result = String::new();
    let mut dim = false;
    let mut end = 0;
    for escape in ANSI.find_iter(screen) {
        for c in screen[end..escape.start()].chars() {
            result.push(if dim && c != '\n' { ' ' } else { c });
        }
        let code = escape.as_str();
        if code.ends_with('m') {
            let parameters = &code[2..code.len() - 1];
            // Zellij dumps use separate SGR attributes. Consume extended color
            // arguments together: RGB values of 0/2/22 are not intensity codes.
            let values: Vec<_> = parameters.split(';').collect();
            let mut i = 0;
            while i < values.len() {
                match values[i] {
                    "" | "0" | "22" => dim = false,
                    "2" => dim = true,
                    "38" | "48" | "58" => {
                        i += match values.get(i + 1) {
                            Some(&"2") => 4,
                            Some(&"5") => 2,
                            _ => 0,
                        };
                    }
                    _ => {}
                }
                i += 1;
            }
        }
        end = escape.end();
    }
    for c in screen[end..].chars() {
        result.push(if dim && c != '\n' { ' ' } else { c });
    }
    result
}

fn horizontal(line: &str) -> bool {
    let line = line.trim();
    line.chars().filter(|c| "─━-=".contains(*c)).count() >= 3
        && line.chars().all(|c| "─━-=╭╮╰╯├┤┬┴┼┌┐└┘│ ".contains(c))
}

/// None means no trustworthy composer is visible (overlay, unsupported tool).
pub fn detect(tool: &str, screen: &str) -> Option<bool> {
    if !matches!(tool, "codex" | "claude") {
        return None;
    }
    let plain = plain(screen);
    let lower = plain.to_lowercase();
    if lower.contains("to scroll")
        || lower.contains("esc to cancel")
        || lower.contains("enter to select")
        || lower.contains("esc to close")
    {
        return None;
    }
    let visible = without_dim(screen);
    let lines: Vec<_> = plain.lines().collect();
    let ink: Vec<_> = visible.lines().collect();
    let marker = if tool == "codex" { '›' } else { '❯' };
    let start = lines
        .iter()
        .rposition(|l| l.trim_start_matches([' ', '│']).starts_with(marker))?;
    let mut end = start + 1;
    if tool == "claude" {
        // A live Claude composer is bounded by horizontal rules, unlike ❯
        // entries in permission menus or previously submitted conversation.
        if !lines[..start].iter().rev().take(2).any(|l| horizontal(l)) {
            return None;
        }
        end = (start + 1..lines.len()).find(|i| horizontal(lines[*i]))?;
    } else {
        // Reject submitted prompts followed by agent output. Continuation lines
        // in the live composer are indented; footer follows a blank separator.
        for line in &lines[start + 1..] {
            if line.trim().is_empty() {
                break;
            }
            if !line.starts_with("  ") || line.trim_start().starts_with(['•', '◦', '■', '─'])
            {
                return None;
            }
            end += 1;
        }
    }
    let first = ink.get(start)?.trim_start_matches([' ', '│']);
    let first = first.strip_prefix(marker).unwrap_or(first).trim();
    Some(
        !first.is_empty()
            || ink[start + 1..end]
                .iter()
                .any(|line| !line.trim_matches([' ', '│', '\r']).is_empty()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn codex_drafts_placeholders_multiline_and_submitted_history() {
        assert_eq!(detect("codex", "old\n› \n\n  GPT-6"), Some(false));
        assert_eq!(
            detect("codex", "› \x1b[2mAsk anything\x1b[22m\n\nfooter"),
            Some(false)
        );
        assert_eq!(detect("codex", "› Ask anything\n\nfooter"), Some(true));
        assert_eq!(
            detect("codex", "› 한글\n  이어지는 입력\n\nfooter"),
            Some(true)
        );
        assert_eq!(detect("codex", "› \n  두 번째 줄\n\nfooter"), Some(true));
        assert_eq!(detect("codex", "› sent\n• response"), None);
        assert_eq!(detect("codex", "› old\n• response\n› \n"), Some(false));
        assert_eq!(detect("codex", "› x\n↑/↓ to scroll"), None);
    }
    #[test]
    fn claude_requires_live_box_and_preserves_unknown_screens() {
        assert_eq!(detect("claude", "───\n❯ \n───\nfooter"), Some(false));
        assert_eq!(
            detect("claude", "╭───╮\n│ ❯ 한글\n│ next\n╰───╯"),
            Some(true)
        );
        assert_eq!(
            detect("claude", "───\n❯ \x1b[2mTry a prompt\x1b[0m\n───"),
            Some(false)
        );
        assert_eq!(detect("claude", "❯ Yes\n  No\nEnter to select"), None);
        assert_eq!(detect("claude", "❯ historical\nresponse"), None);
        assert_eq!(detect("codex", ""), None);
        assert_eq!(detect("pi", "› text"), None);
    }
    #[test]
    fn sgr_color_components_do_not_change_dim_state() {
        assert_eq!(without_dim("\x1b[38;2;2;22;0mtext"), "text");
        assert_eq!(without_dim("\x1b[2;38;2;22;0;2mtext\x1b[22m!"), "    !");
    }
}
