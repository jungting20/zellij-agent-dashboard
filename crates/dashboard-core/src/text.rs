use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

#[derive(Clone, Default)]
pub(crate) struct Line {
    spans: Vec<Span>,
    pub reverse: bool,
}

#[derive(Clone)]
struct Span {
    text: String,
    color: Option<u8>,
    bold: bool,
    reverse: bool,
}

impl Line {
    pub fn plain(text: impl AsRef<str>) -> Self {
        Self::styled(text, None, false)
    }

    pub fn color(text: impl AsRef<str>, color: u8, bold: bool) -> Self {
        Self::styled(text, Some(color), bold)
    }

    fn styled(text: impl AsRef<str>, color: Option<u8>, bold: bool) -> Self {
        Self {
            spans: vec![Span {
                text: clean(text.as_ref()),
                color,
                bold,
                reverse: false,
            }],
            reverse: false,
        }
    }

    pub fn append(mut self, mut other: Self) -> Self {
        if self.reverse {
            for span in &mut self.spans {
                span.reverse = true;
            }
            self.reverse = false;
        }
        if other.reverse {
            for span in &mut other.spans {
                span.reverse = true;
            }
        }
        self.spans.extend(other.spans);
        self
    }

    pub fn text(&self) -> String {
        self.spans.iter().map(|s| s.text.as_str()).collect()
    }

    pub fn cut(&self, start: usize, end: usize) -> Self {
        let mut at = 0;
        let mut spans = Vec::new();
        for span in &self.spans {
            let mut clipped = span.clone();
            clipped.text.clear();
            for glyph in span.text.graphemes(true) {
                let next = at + glyph.width();
                if at >= start && next <= end {
                    clipped.text.push_str(glyph);
                } else if next > start && at < end {
                    clipped
                        .text
                        .push_str(&" ".repeat(next.min(end) - at.max(start)));
                }
                at = next;
            }
            spans.push(clipped);
        }
        Self {
            spans,
            reverse: self.reverse,
        }
    }

    pub fn fit(mut self, width: usize, pad: bool) -> Self {
        let overflows = self.spans.iter().map(|s| s.text.width()).sum::<usize>() > width;
        let budget = width.saturating_sub(usize::from(overflows));
        let mut used = 0;
        let mut spans = Vec::new();
        for mut span in self.spans {
            let mut clipped = String::new();
            for glyph in span.text.graphemes(true) {
                let columns = glyph.width();
                if used + columns > budget {
                    break;
                }
                clipped.push_str(glyph);
                used += columns;
            }
            let complete = clipped.len() == span.text.len();
            span.text = clipped;
            spans.push(span);
            if !complete {
                break;
            }
        }
        if overflows && width > 0 {
            spans.push(Span {
                text: "…".into(),
                color: None,
                bold: false,
                reverse: false,
            });
            used += 1;
        }
        if pad && used < width {
            spans.push(Span {
                text: " ".repeat(width - used),
                color: None,
                bold: false,
                reverse: false,
            });
        }
        self.spans = spans;
        self
    }

    pub fn render(&self) -> String {
        let mut result = String::new();
        for span in &self.spans {
            if span.text.is_empty() {
                continue;
            }
            result.push_str("\x1b[0m");
            if self.reverse || span.reverse {
                result.push_str("\x1b[7m");
            }
            if span.bold {
                result.push_str("\x1b[1m");
            }
            if let Some(color) = span.color {
                result.push_str(&format!("\x1b[38;5;{color}m"));
            }
            result.push_str(&span.text);
        }
        result.push_str("\x1b[0m");
        result
    }
}

pub(crate) fn clean(text: &str) -> String {
    text.chars().filter(|c| !c.is_control()).collect()
}

pub(crate) fn wrap(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut lines = Vec::new();
    for paragraph in text.replace("\r\n", "\n").split('\n') {
        let paragraph = clean(&paragraph.replace('\t', " "));
        let mut line = String::new();
        let mut used = 0;
        for glyph in paragraph.graphemes(true) {
            let columns = glyph.width();
            if used + columns > width && !line.is_empty() {
                lines.push(std::mem::take(&mut line));
                used = 0;
            }
            if columns <= width {
                line.push_str(glyph);
                used += columns;
            }
        }
        lines.push(line);
    }
    lines
}
