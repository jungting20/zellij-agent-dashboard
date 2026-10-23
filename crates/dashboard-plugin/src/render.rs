/// Replace the plugin viewport without advancing past its bottom row.
/// A final newline scrolls Zellij's persistent plugin grid on every repaint.
pub fn frame(lines: &[String], rows: usize) -> String {
    let mut output = String::from("\x1b[0m\x1b[2J\x1b[H");
    for (index, line) in lines.iter().take(rows).enumerate() {
        if index > 0 {
            output.push_str("\r\n");
        }
        output.push_str(line);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_height_frame_never_advances_beyond_viewport() {
        for rows in [0, 1, 2, 30] {
            let lines = vec!["content".into(); rows + 1];
            let output = frame(&lines, rows);
            assert_eq!(output.matches('\n').count(), rows.saturating_sub(1));
            assert!(!output.ends_with('\n'));
            assert!(output.starts_with("\x1b[0m\x1b[2J\x1b[H"));
        }
    }

    #[test]
    fn shorter_repaint_clears_old_content_and_preserves_styles() {
        let line = "\x1b[7m선택\x1b[0m".to_string();
        assert_eq!(
            frame(&[line.clone()], 30),
            format!("\x1b[0m\x1b[2J\x1b[H{line}")
        );
    }
}
