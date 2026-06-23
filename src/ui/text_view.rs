use unicode_width::UnicodeWidthChar;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WrappedRow {
    pub text: String,
    pub source_line: usize,
    pub continuation: bool,
}

pub fn sanitize_terminal_text(input: &str) -> String {
    let mut output = String::new();
    let mut line = Vec::<char>::new();
    let mut cursor = 0_usize;
    let mut chars = input.chars().peekable();

    while let Some(character) = chars.next() {
        match character {
            '\u{1b}' => skip_escape_sequence(&mut chars),
            '\n' => {
                output.extend(line.iter());
                output.push('\n');
                line.clear();
                cursor = 0;
            }
            '\r' => cursor = 0,
            '\u{8}' => cursor = cursor.saturating_sub(1),
            '\t' => {
                let spaces = 4 - (cursor % 4);
                for _ in 0..spaces {
                    put_character(&mut line, &mut cursor, ' ');
                }
            }
            character if !character.is_control() => {
                put_character(&mut line, &mut cursor, character);
            }
            _ => {}
        }
    }

    output.extend(line.iter());
    output
}

pub fn wrap_text(input: &str, width: usize) -> Vec<WrappedRow> {
    let width = width.max(1);
    let continuation_width = width.saturating_sub(2).max(1);
    let mut rows = Vec::new();

    for (line_index, source) in input.split('\n').enumerate() {
        if source.is_empty() {
            rows.push(WrappedRow {
                text: String::new(),
                source_line: line_index + 1,
                continuation: false,
            });
            continue;
        }

        let mut chunk = String::new();
        let mut chunk_width = 0;
        let mut first = true;
        for character in source.chars() {
            let capacity = if first { width } else { continuation_width };
            let character_width = UnicodeWidthChar::width(character).unwrap_or(0);
            if !chunk.is_empty() && chunk_width + character_width > capacity {
                rows.push(WrappedRow {
                    text: if first {
                        std::mem::take(&mut chunk)
                    } else {
                        format!("↪ {}", std::mem::take(&mut chunk))
                    },
                    source_line: line_index + 1,
                    continuation: !first,
                });
                first = false;
                chunk_width = 0;
            }
            chunk.push(character);
            chunk_width += character_width;
        }
        if !chunk.is_empty() {
            rows.push(WrappedRow {
                text: if first { chunk } else { format!("↪ {chunk}") },
                source_line: line_index + 1,
                continuation: !first,
            });
        }
    }

    rows
}

pub fn page_metrics(offset: usize, viewport: usize, total: usize) -> (usize, usize, usize, usize) {
    if total == 0 {
        return (0, 0, 0, 0);
    }
    let viewport = viewport.max(1);
    let offset = offset.min(total.saturating_sub(1));
    let end = (offset + viewport).min(total);
    let page = offset / viewport + 1;
    let pages = (total + viewport - 1) / viewport;
    (page, pages, offset + 1, end)
}

fn put_character(line: &mut Vec<char>, cursor: &mut usize, character: char) {
    if *cursor < line.len() {
        line[*cursor] = character;
    } else {
        while line.len() < *cursor {
            line.push(' ');
        }
        line.push(character);
    }
    *cursor += 1;
}

fn skip_escape_sequence<I>(chars: &mut std::iter::Peekable<I>)
where
    I: Iterator<Item = char>,
{
    match chars.next() {
        Some('[') => {
            for character in chars.by_ref() {
                if ('@'..='~').contains(&character) {
                    break;
                }
            }
        }
        Some(']') => {
            let mut previous_escape = false;
            for character in chars.by_ref() {
                if character == '\u{7}' || (previous_escape && character == '\\') {
                    break;
                }
                previous_escape = character == '\u{1b}';
            }
        }
        Some(_) | None => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_terminal_sequences_and_controls() {
        let input = "\u{1b}[31mred\u{1b}[0m \u{1b}]0;title\u{7}ok\u{0}";
        assert_eq!(sanitize_terminal_text(input), "red ok");
    }

    #[test]
    fn resolves_carriage_returns_backspaces_and_tabs() {
        assert_eq!(sanitize_terminal_text("abc\rXY\nab\u{8}c\tz"), "XYc\nac  z");
    }

    #[test]
    fn wraps_unicode_with_continuation_markers() {
        let rows = wrap_text("樱花abcdef", 5);
        assert_eq!(rows[0].text, "樱花a");
        assert_eq!(rows[1].text, "↪ bcd");
        assert_eq!(rows[1].source_line, 1);
    }

    #[test]
    fn computes_page_ranges() {
        assert_eq!(page_metrics(10, 10, 25), (2, 3, 11, 20));
        assert_eq!(page_metrics(0, 10, 0), (0, 0, 0, 0));
    }
}
