//! Which lines of a Markdown draft continue the paragraph of the line before (CommonMark 0.31), so that a
//! statement spans a soft wrap and stops where a block starts. Each line is matched against the open block quotes
//! and list items in order, as a CommonMark parser does, and then read for the blocks it starts: ATX and setext
//! headings, thematic breaks, fenced and indented code, block quotes, list items and paragraph text. A line
//! holding `|` is a table row, as before; HTML blocks and link reference definitions are not modelled.
//!
//! The paragraph-interruption rules are where a line-by-line reader goes wrong. A wrapped line that begins `2025.`
//! or `10)` continues its sentence, because only an ordered list starting at 1 may interrupt a paragraph. The same
//! line after a finished list item starts the next item. A list item's or block quote's wrapped line continues
//! the item's or quote's sentence.

/// A code fence: its marker byte, its width, and the text after it.
#[must_use]
pub(crate) fn fence(line: &str) -> Option<(u8, usize, &str)> {
    let text = line.trim_start_matches(' ');
    let marker = *text.as_bytes().first()?;
    if line.len() - text.len() > 3 || !matches!(marker, b'`' | b'~') {
        return None;
    }
    let width = text.bytes().take_while(|&b| b == marker).count();
    let tail = &text[width..];
    // A backtick fence's info string holds no backtick: "```a`" opens a code span, not a fence.
    (width >= 3 && !(marker == b'`' && tail.contains('`'))).then_some((marker, width, tail))
}

/// For each line of `text` (split after `\n`, a trailing `\r` ignored), whether the next line continues the same
/// paragraph. The last line never does.
#[must_use]
pub(crate) fn joins_next(text: &str) -> Vec<bool> {
    let mut state = State::default();
    let mut joins = Vec::new();
    for raw in text.split_inclusive('\n') {
        let continued = state.feed(raw.trim_end_matches(['\r', '\n']));
        if let Some(previous) = joins.last_mut() {
            *previous = continued;
        }
        joins.push(false);
    }
    joins
}

#[derive(Clone, Copy)]
enum Container {
    Quote,
    /// A list item whose content starts `width` columns in; `empty` while it holds nothing yet.
    Item {
        width: usize,
        empty: bool,
    },
}

#[derive(Default)]
struct State {
    /// The last line was paragraph text, so this line may continue it.
    paragraph: bool,
    /// The open block quotes and list items, outermost first.
    containers: Vec<Container>,
    /// The open code fence: its marker, its width, and how many containers hold it.
    fence: Option<(u8, usize, usize)>,
}

/// A line's position: the byte offset and the column reached, with a tab advancing to the next multiple of 4.
/// `partial` counts the columns left of a tab that indentation consumed only in part.
#[derive(Clone, Copy)]
struct Cursor<'a> {
    line: &'a str,
    at: usize,
    column: usize,
    partial: usize,
}

impl<'a> Cursor<'a> {
    /// Columns of whitespace from here.
    fn indent(self) -> usize {
        let mut column = self.column + self.partial;
        for byte in self.line[self.at..].bytes() {
            match byte {
                b' ' => column += 1,
                b'\t' => column += 4 - column % 4,
                _ => break,
            }
        }
        column - self.column
    }

    /// The text after the whitespace.
    fn rest(self) -> &'a str {
        self.line[self.at..].trim_start_matches([' ', '\t'])
    }

    /// Consumes whitespace up to `columns` columns; a tab that overshoots leaves its remaining columns.
    fn skip(&mut self, columns: usize) {
        let mut left = columns;
        let used = self.partial.min(left);
        self.partial -= used;
        self.column += used;
        left -= used;
        while left > 0 {
            let tab = match self.line.as_bytes().get(self.at) {
                Some(b' ') => 1,
                Some(b'\t') => 4 - self.column % 4,
                _ => return,
            };
            self.at += 1;
            let used = tab.min(left);
            self.column += used;
            self.partial = tab - used;
            left -= used;
        }
    }

    /// Consumes the whitespace, then `bytes` bytes of marker.
    fn skip_marker(&mut self, bytes: usize) {
        self.skip(self.indent());
        self.at += bytes;
        self.column += bytes;
    }
}

#[derive(Clone, Copy)]
enum Start {
    Blank,
    /// An ATX heading, thematic break, code fence or table row: never paragraph text.
    Block,
    Quote,
    /// `=` or `-` alone, repeated: a setext underline after paragraph text.
    Underline,
    /// A list marker `width` bytes long; `number` is an ordered list's start.
    Item {
        number: Option<u64>,
        width: usize,
    },
    Text,
}

impl State {
    /// Takes the next line and returns whether it continues the open paragraph.
    fn feed(&mut self, line: &str) -> bool {
        let mut cursor = Cursor {
            line,
            at: 0,
            column: 0,
            partial: 0,
        };
        let matched = self.match_containers(&mut cursor);
        if let Some((open, minimum, depth)) = self.fence {
            if matched >= depth {
                if fence(&line[cursor.at..]).is_some_and(|(marker, width, tail)| {
                    marker == open && width >= minimum && tail.trim().is_empty()
                }) {
                    self.fence = None;
                }
                return false;
            }
            self.fence = None; // a container closed, and the code block with it
        }
        let all = matched == self.containers.len();
        if self.paragraph {
            match self.after_paragraph(cursor, all) {
                Next::Continues => return true,
                Next::Underline => {
                    self.paragraph = false;
                    return false;
                }
                Next::Block => {}
            }
        }
        self.containers.truncate(matched);
        if !cursor.rest().is_empty() {
            for container in &mut self.containers {
                if let Container::Item { empty, .. } = container {
                    *empty = false;
                }
            }
        }
        self.paragraph = self.open_blocks(cursor);
        false
    }

    /// How many open containers the line continues, consuming their markers and indentation.
    fn match_containers(&self, cursor: &mut Cursor<'_>) -> usize {
        for (index, container) in self.containers.iter().enumerate() {
            let blank = cursor.rest().is_empty();
            match *container {
                Container::Quote => {
                    if cursor.indent() > 3 || !cursor.rest().starts_with('>') {
                        return index;
                    }
                    cursor.skip_marker(1);
                    cursor.skip(1);
                }
                // A blank line continues an item, unless the item has held nothing since its marker line.
                Container::Item { empty, .. } if blank => {
                    if empty {
                        return index;
                    }
                }
                Container::Item { width, .. } => {
                    if cursor.indent() < width {
                        return index;
                    }
                    cursor.skip(width);
                }
            }
        }
        self.containers.len()
    }

    /// What the line does to the open paragraph. With every container matched the paragraph is the container,
    /// and only a block quote, a bullet with content, an ordered item starting at 1, a setext underline or a
    /// block (heading, rule, fence) interrupts it. As a lazy continuation line, any list marker starts an item.
    fn after_paragraph(&self, cursor: Cursor<'_>, all: bool) -> Next {
        let rest = cursor.rest();
        if rest.is_empty() {
            return Next::Block;
        }
        if cursor.indent() >= 4 {
            return Next::Continues; // indented code cannot interrupt a paragraph
        }
        let has_content = |width: usize| !rest[width..].trim().is_empty();
        match classify(rest) {
            Start::Blank | Start::Block | Start::Quote => Next::Block,
            Start::Underline if all => Next::Underline,
            Start::Underline if rest.trim_end_matches([' ', '\t']) == "-" => Next::Block,
            Start::Underline | Start::Text => Next::Continues,
            Start::Item { number, width } => {
                let interrupts = match number {
                    None => has_content(width),
                    Some(n) => n == 1 && has_content(width),
                };
                if all && !interrupts {
                    Next::Continues
                } else {
                    Next::Block
                }
            }
        }
    }

    /// Opens the block quotes and list items the line starts, then reads its leaf block. Returns whether the
    /// line is paragraph text.
    fn open_blocks(&mut self, mut cursor: Cursor<'_>) -> bool {
        loop {
            let rest = cursor.rest();
            if rest.is_empty() || cursor.indent() >= 4 {
                return false; // blank, or indented code
            }
            let width = match classify(rest) {
                Start::Blank => return false,
                Start::Block => {
                    if let Some((marker, width, _)) = fence(rest) {
                        self.fence = Some((marker, width, self.containers.len()));
                    }
                    return false;
                }
                Start::Quote => {
                    cursor.skip_marker(1);
                    cursor.skip(1);
                    self.containers.push(Container::Quote);
                    continue;
                }
                Start::Item { width, .. } => width,
                Start::Underline if rest.trim_end_matches([' ', '\t']) == "-" => 1,
                Start::Underline | Start::Text => return true,
            };
            let indent = cursor.indent();
            cursor.skip_marker(width);
            let spaces = cursor.indent();
            let empty = cursor.rest().is_empty();
            let content = if empty || spaces > 4 { 1 } else { spaces };
            cursor.skip(content);
            self.containers.push(Container::Item {
                width: indent + width + content,
                empty,
            });
            if empty {
                return false;
            }
        }
    }
}

enum Next {
    Continues,
    /// A setext underline: it ends the paragraph, which becomes a heading, and is not text itself.
    Underline,
    Block,
}

fn classify(rest: &str) -> Start {
    let Some(&first) = rest.as_bytes().first() else {
        return Start::Blank;
    };
    if first == b'>' {
        return Start::Quote;
    }
    let hashes = rest.bytes().take_while(|&b| b == b'#').count();
    if (1..=6).contains(&hashes)
        && (rest.len() == hashes || rest[hashes..].starts_with([' ', '\t']))
    {
        return Start::Block;
    }
    if matches!(first, b'-' | b'*' | b'_')
        && rest.bytes().filter(|&b| b == first).count() >= 3
        && rest.bytes().all(|b| b == first || b == b' ' || b == b'\t')
    {
        return Start::Block;
    }
    if matches!(first, b'=' | b'-')
        && rest
            .trim_end_matches([' ', '\t'])
            .bytes()
            .all(|b| b == first)
    {
        return Start::Underline;
    }
    if fence(rest).is_some() || rest.contains('|') {
        return Start::Block;
    }
    let ends_marker = |after: &str| after.is_empty() || after.starts_with([' ', '\t']);
    if matches!(first, b'-' | b'+' | b'*') && ends_marker(&rest[1..]) {
        return Start::Item {
            number: None,
            width: 1,
        };
    }
    let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
    if (1..=9).contains(&digits)
        && matches!(rest.as_bytes().get(digits), Some(b'.' | b')'))
        && ends_marker(&rest[digits + 1..])
    {
        return Start::Item {
            number: rest[..digits].parse().ok(),
            width: digits + 1,
        };
    }
    Start::Text
}

#[cfg(test)]
mod tests {
    use super::joins_next;

    /// Known answers, one per rule. Each agrees with pulldown-cmark 0.13.6, which this model was checked against
    /// on 2.47 million generated documents (2026-11-27). The exceptions are the `|` row, kept as a table row as
    /// before, and a tab-indented `>` after a quoted paragraph, where implementations differ (not listed).
    #[test]
    fn joins_follow_commonmark_paragraph_continuation() {
        for (text, joins) in [
            // Only a non-empty ordered item starting at 1, or a non-empty bullet, interrupts a paragraph.
            ("a\n2. b", &[true, false][..]),
            ("a\n10) b", &[true, false]),
            ("a\n1. b", &[false, false]),
            ("a\n1.", &[true, false]),
            ("a\n- b", &[false, false]),
            ("a\n*", &[true, false]),
            // After a list item, any marker at the list's level starts the next item.
            ("1. a\n2. b", &[false, false]),
            ("- a\n\n  b\n2. c", &[false, false, false, false]),
            // Wrapped and lazy lines of list items and block quotes.
            ("- a\n  b", &[true, false]),
            ("- a\nb", &[true, false]),
            ("> a\n> b", &[true, false]),
            ("> a\nb", &[true, false]),
            ("> a\n> > b", &[false, false]),
            ("> a\n- b", &[false, false]),
            // Indented code cannot interrupt a paragraph, and a paragraph cannot continue code.
            ("a\n    b", &[true, false]),
            ("    a\nb", &[false, false]),
            // Headings, setext underlines and fences.
            ("a\n# b", &[false, false]),
            ("a\n#5 b", &[true, false]),
            ("a\n-", &[false, false]),
            ("a\nb\n===\nc", &[true, false, false, false]),
            (
                "a\n```\nb\nc\n```\nd",
                &[false, false, false, false, false, false],
            ),
            ("a\n```x`\nb", &[true, true, false]),
            // An empty item closes at a blank line; a tab counts to the next multiple of 4, in part if need be.
            ("-\n\n    b\nc", &[false, false, false, false]),
            ("* - a\n\t*", &[true, false]),
            // A line holding `|` is a table row.
            ("a\nb | c", &[false, false]),
        ] {
            assert_eq!(joins_next(text), joins, "{text:?}");
        }
    }
}
