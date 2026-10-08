use crate::{
    text::{wrap, Line},
    Catalog, Identity,
};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuKind {
    Input,
    Alias,
    AliasCustom,
    Recent,
    Tool,
    Worktree,
    WorktreeName,
    WorktreeTool,
    Children,
    Merge,
    Shell,
    Result,
    Requests,
}

#[derive(Default)]
pub struct TextInput {
    pub text: String,
    pub cursor: usize,
}

impl TextInput {
    pub fn new(text: String) -> Self {
        let cursor = text.len();
        Self { text, cursor }
    }
    pub fn insert(&mut self, value: &str, multiline: bool) {
        let value: String = value
            .replace("\r\n", "\n")
            .chars()
            .filter(|c| !c.is_control() || (multiline && matches!(c, '\n' | '\t')))
            .collect();
        if self.text.len() + value.len() > 64 * 1024 {
            return;
        }
        self.text.insert_str(self.cursor, &value);
        self.cursor += value.len();
    }
    pub fn left(&mut self) {
        self.cursor = self.text[..self.cursor]
            .grapheme_indices(true)
            .next_back()
            .map_or(0, |(i, _)| i);
    }
    pub fn right(&mut self) {
        self.cursor += self.text[self.cursor..]
            .graphemes(true)
            .next()
            .map_or(0, str::len);
    }
    pub fn backspace(&mut self) {
        let end = self.cursor;
        self.left();
        self.text.replace_range(self.cursor..end, "");
    }
    pub fn delete(&mut self) {
        let start = self.cursor;
        self.right();
        self.text.replace_range(start..self.cursor, "");
        self.cursor = start;
    }
    pub fn display(&self) -> String {
        format!(
            "{}▏{}",
            &self.text[..self.cursor],
            &self.text[self.cursor..]
        )
    }
}

pub struct Menu {
    pub kind: MenuKind,
    pub target: Option<Identity>,
    pub title: String,
    pub input: TextInput,
    pub options: Vec<(String, String)>,
    pub selected: usize,
    pub catalog: Catalog,
    pub directory: String,
    pub branch: String,
    pub error: String,
    pub busy: bool,
    pub request_id: Option<String>,
    pub result: String,
    pub offset: usize,
}

impl Menu {
    pub fn new(kind: MenuKind, title: impl Into<String>, target: Option<Identity>) -> Self {
        Self {
            kind,
            title: title.into(),
            target,
            input: TextInput::default(),
            options: Vec::new(),
            selected: 0,
            catalog: Catalog::default(),
            directory: String::new(),
            branch: String::new(),
            error: String::new(),
            busy: false,
            request_id: None,
            result: String::new(),
            offset: 0,
        }
    }
    pub fn editing(&self) -> bool {
        matches!(
            self.kind,
            MenuKind::Input
                | MenuKind::AliasCustom
                | MenuKind::Recent
                | MenuKind::WorktreeName
                | MenuKind::Shell
        )
    }
    pub fn matches(&self) -> Vec<&(String, String)> {
        if self.kind != MenuKind::Recent {
            return self.options.iter().collect();
        }
        let query = self.input.text.to_lowercase();
        self.options
            .iter()
            .filter(|(label, _)| label.to_lowercase().contains(&query))
            .collect()
    }
    pub fn move_selection(&mut self, delta: isize) {
        self.selected = self
            .selected
            .saturating_add_signed(delta)
            .min(self.matches().len().saturating_sub(1));
    }
    pub fn chosen(&self) -> Option<String> {
        self.matches()
            .get(self.selected)
            .map(|(_, value)| value.clone())
    }

    pub(crate) fn overlay(&self, height: usize, width: usize, mut base: Vec<Line>) -> Vec<Line> {
        if height < 4 || width < 8 {
            return base;
        }
        let box_width = width.saturating_sub(4).min(
            if matches!(
                self.kind,
                MenuKind::Input | MenuKind::Alias | MenuKind::AliasCustom
            ) {
                60
            } else {
                76
            },
        );
        let content_width = box_width.saturating_sub(4).max(1);
        let visible = height.saturating_sub(8).clamp(1, 16);
        let mut content = vec![Line::color(&self.title, 42, true)];
        if !self.directory.is_empty() {
            content.push(Line::plain(&self.directory));
        }
        if self.editing() {
            let text = wrap(&format!("> {}", self.input.display()), content_width);
            let cursor_lines = wrap(
                &format!("> {}▏", &self.input.text[..self.input.cursor]),
                content_width,
            )
            .len();
            let start = cursor_lines.saturating_sub(visible.min(5));
            content.extend(
                text.iter()
                    .skip(start)
                    .take(visible.min(5))
                    .map(Line::plain),
            );
        }
        if self.kind == MenuKind::Result {
            let rows = wrap(&self.result, content_width);
            let start = self.offset.min(rows.len().saturating_sub(visible));
            content.extend(rows.iter().skip(start).take(visible).map(Line::plain));
        } else {
            let options = self.matches();
            let start = self.selected.saturating_sub(visible.saturating_sub(1));
            for (i, (label, _)) in options.iter().enumerate().skip(start).take(visible) {
                let mut row = Line::plain(format!(
                    "{} {label}",
                    if i == self.selected { ">" } else { " " }
                ));
                row.reverse = i == self.selected;
                content.push(row);
            }
        }
        if !self.error.is_empty() {
            content.push(Line::color(&self.error, 196, false));
        }
        if self.busy {
            content.push(Line::color("처리 중…", 244, false));
        }
        let help = match self.kind {
            MenuKind::Input => "Enter 전송 · Alt+Enter 줄바꿈 · Ctrl+e 에디터 · Esc 취소",
            MenuKind::Recent => "검색/경로 입력 · ↑↓ 선택 · Enter 다음 · Esc 취소",
            MenuKind::Result => "↑↓/PgUp/PgDn 스크롤 · Esc 닫기",
            MenuKind::Worktree => "a 추가 · s 셸 · m 병합 · t 자식 이동 · g lazygit · Esc 닫기",
            _ => "↑↓ 선택 · Enter 적용 · Esc 취소",
        };
        content.push(Line::color(help, 244, false));
        content.truncate(height.saturating_sub(2));
        let mut popup = vec![Line::color(
            format!("╭{}╮", "─".repeat(box_width.saturating_sub(2))),
            42,
            false,
        )];
        popup.extend(content.into_iter().map(|line| {
            Line::color("│ ", 42, false)
                .append(line.fit(content_width, true))
                .append(Line::color(" │", 42, false))
        }));
        popup.push(Line::color(
            format!("╰{}╯", "─".repeat(box_width.saturating_sub(2))),
            42,
            false,
        ));
        base.resize(height, Line::default());
        let x = (width - box_width) / 2;
        let y = (height - popup.len()) / 2;
        for (i, line) in popup.into_iter().enumerate() {
            let background = base[y + i].clone().fit(width, true);
            base[y + i] = background
                .cut(0, x)
                .append(line)
                .append(background.cut(x + box_width, width));
        }
        base
    }
}

pub const ALIASES: &[(&str, &str)] = &[
    ("미지정", ""),
    ("구현", "implement"),
    ("버그 수정", "bugfix"),
    ("리팩터링", "refactor"),
    ("테스트", "test"),
    ("리뷰", "review"),
    ("문서", "docs"),
    ("조사", "research"),
    ("직접입력", "custom"),
];
pub fn alias_label(value: &str) -> &str {
    ALIASES
        .iter()
        .find(|(_, v)| *v == value)
        .map_or(value, |(label, _)| *label)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn edits_graphemes_and_preserves_literal_multiline_input() {
        let mut input = TextInput::new("한글 👩‍💻".into());
        input.backspace();
        assert_eq!(input.text, "한글 ");
        input.left();
        input.insert("$(touch bad)\n둘째 줄", true);
        assert_eq!(input.text, "한글$(touch bad)\n둘째 줄 ");
        input.delete();
        input.insert("\x1b", true);
        assert!(!input.text.contains('\x1b'));
    }
}
