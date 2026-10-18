#[derive(Default)]
enum ParserState {
    #[default]
    Normal,
    Escape,
    Csi,
    Osc,
}

pub(super) struct TerminalLogSanitizer {
    display: bool,
    state: ParserState,
    csi: String,
    line: Vec<char>,
    cursor: usize,
    insert_mode: bool,
}

impl TerminalLogSanitizer {
    pub(super) fn new(format: &str) -> Self {
        Self {
            display: format != "strip_controls",
            state: ParserState::Normal,
            csi: String::new(),
            line: Vec::new(),
            cursor: 0,
            insert_mode: false,
        }
    }

    pub(super) fn push(&mut self, data: &str) -> String {
        let mut output = String::new();
        for ch in data.chars() {
            match self.state {
                ParserState::Escape => {
                    self.state = match ch {
                        '[' => {
                            self.csi.clear();
                            ParserState::Csi
                        }
                        ']' => ParserState::Osc,
                        _ => ParserState::Normal,
                    };
                }
                ParserState::Csi => {
                    self.csi.push(ch);
                    if ('\u{40}'..='\u{7e}').contains(&ch) {
                        if self.display {
                            self.apply_csi();
                        }
                        self.csi.clear();
                        self.state = ParserState::Normal;
                    }
                }
                ParserState::Osc => {
                    if ch == '\u{7}' {
                        self.state = ParserState::Normal;
                    } else if ch == '\u{1b}' {
                        self.state = ParserState::Escape;
                    }
                }
                ParserState::Normal => {
                    if ch == '\u{1b}' {
                        self.state = ParserState::Escape;
                    } else if !self.display {
                        if matches!(ch, '\n' | '\r' | '\t') || ch >= ' ' {
                            output.push(ch);
                        }
                    } else {
                        match ch {
                            '\u{8}' => self.cursor = self.cursor.saturating_sub(1),
                            '\u{7f}' => (),
                            '\r' => self.cursor = 0,
                            '\n' => {
                                output.extend(self.line.drain(..));
                                output.push('\n');
                                self.cursor = 0;
                            }
                            '\t' => self.write_char(ch),
                            _ if ch >= ' ' => self.write_char(ch),
                            _ => (),
                        }
                    }
                }
            }
        }
        output
    }

    pub(super) fn flush(&mut self) -> String {
        let output = self.line.drain(..).collect();
        self.state = ParserState::Normal;
        self.csi.clear();
        self.cursor = 0;
        self.insert_mode = false;
        output
    }

    fn write_char(&mut self, ch: char) {
        if self.cursor > self.line.len() {
            self.line.resize(self.cursor, ' ');
        }
        if self.insert_mode || self.cursor == self.line.len() {
            self.line.insert(self.cursor, ch);
        } else {
            self.line[self.cursor] = ch;
        }
        self.cursor += 1;
    }

    fn apply_csi(&mut self) {
        let Some(command) = self.csi.chars().last() else {
            return;
        };
        let params = &self.csi[..self.csi.len() - command.len_utf8()];
        if !params.chars().all(|ch| ch.is_ascii_digit() || ch == ';') {
            return;
        }
        let first = params.split(';').next().unwrap_or("");
        let amount = if first.is_empty() {
            1
        } else {
            first.parse::<usize>().unwrap_or(usize::MAX).max(1)
        };
        // There is no viewport width in a text log; bound cursor-created padding.
        let cursor_limit = self.line.len().max(65_536);
        match command {
            'D' => self.cursor = self.cursor.saturating_sub(amount),
            'C' => self.cursor = self.cursor.saturating_add(amount).min(cursor_limit),
            'G' => self.cursor = amount.saturating_sub(1).min(cursor_limit),
            'h' | 'l' if params.split(';').any(|param| param == "4") => {
                self.insert_mode = command == 'h';
            }
            'P' => {
                let start = self.cursor.min(self.line.len());
                let end = start.saturating_add(amount).min(self.line.len());
                self.line.drain(start..end);
            }
            '@' if self.cursor < self.line.len() => {
                let amount = amount.min(cursor_limit);
                self.line
                    .splice(self.cursor..self.cursor, std::iter::repeat_n(' ', amount));
            }
            'K' => match params {
                "" | "0" => self.line.truncate(self.cursor),
                "1" => {
                    let end = self.cursor.saturating_add(1).min(self.line.len());
                    self.line[..end].fill(' ');
                }
                "2" => self.line.clear(),
                _ => (),
            },
            _ => (),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TerminalLogSanitizer;

    #[test]
    fn display_edits_follow_terminal_overwrite_and_cursor_semantics() {
        let cases = [
            ("abc\rX\n", "Xbc\n"),
            ("abc\x1b[2DX\n", "aXc\n"),
            ("abc\r\x1b[CX\n", "aXc\n"),
            ("abc\x1b[2GX\n", "aXc\n"),
            ("abc\r\x1b[0CX\n", "aXc\n"),
            ("abc\x1b[GX\n", "Xbc\n"),
            ("abc\x1b[999DX\n", "Xbc\n"),
            ("a\x1b[2CX\n", "a  X\n"),
            ("a\x1b[4GX\n", "a  X\n"),
            ("abc\x08\n", "abc\n"),
            ("abc\x08X\n", "abX\n"),
            ("abc\r\x08X\n", "Xbc\n"),
            ("abc\x08 \x08\n", "ab \n"),
            ("abc\x7f\n", "abc\n"),
            ("abcd\x1b[3D\x1b[PX\n", "aXd\n"),
            ("abcde\x1b[4D\x1b[2PX\n", "aXe\n"),
            ("abcd\x1b[2D\x1b[KX\n", "abX\n"),
            ("abcd\x1b[2D\x1b[1KX\n", "  Xd\n"),
            ("abcd\x1b[2D\x1b[2KX\n", "  X\n"),
            ("abc\x1b[2D\x1b[4hX\n", "aXbc\n"),
            ("abc\x1b[2D\x1b[@X\n", "aXbc\n"),
            ("abc\r\x1b[4hX\x1b[4lY\n", "XYbc\n"),
            ("abc\r\x1b[?4hX\n", "Xbc\n"),
            ("é😀界\rX\n", "X😀界\n"),
            ("e\u{301}😀界\n", "e\u{301}😀界\n"),
            ("a😀b\x1b[2D\x1b[P\n", "ab\n"),
            ("é😀界\r\x1b[4hX\n", "Xé😀界\n"),
            ("abc\r\x1b[31mX\x1b[0m\n", "Xbc\n"),
            ("show ver\x1b[3Dversion\x1b[K\r\n", "show version\n"),
        ];
        for (input, expected) in cases {
            let mut sanitizer = TerminalLogSanitizer::new("display");
            assert_eq!(
                sanitizer.push(input) + &sanitizer.flush(),
                expected,
                "{input:?}"
            );
        }
    }

    #[test]
    fn parser_and_insert_mode_survive_chunks_and_lines() {
        let mut sanitizer = TerminalLogSanitizer::new("display");
        assert_eq!(sanitizer.push("abc\r\x1b["), "");
        assert_eq!(sanitizer.push("4hX\nabc\rY\n"), "Xabc\nYabc\n");
        sanitizer.push("abc\rX");
        assert_eq!(sanitizer.flush(), "Xabc");
        assert_eq!(sanitizer.push("abc\rY\n"), "Ybc\n");
    }

    #[test]
    fn strip_controls_preserves_raw_carriage_returns() {
        let mut sanitizer = TerminalLogSanitizer::new("strip_controls");
        assert_eq!(sanitizer.push("abc\r\x1b[31mX\x08\x1b[0m\n"), "abc\rX\n");
        assert_eq!(sanitizer.flush(), "");
    }

    #[test]
    fn cursor_padding_is_bounded_for_extreme_csi_parameters() {
        for command in ['C', 'G'] {
            let mut sanitizer = TerminalLogSanitizer::new("display");
            let input = format!("\x1b[999999999999999999999999999999{command}X\n");
            let output = sanitizer.push(&input);
            assert_eq!(output, " ".repeat(65_536) + "X\n");
        }
    }
}
