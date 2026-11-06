use std::{collections::BTreeSet, ops::Range};

#[derive(Clone, Debug)]
pub struct Block {
    pub start: usize,
    pub end: usize,
    pub anchor: usize,
}
#[derive(Default, Debug)]
pub struct Outline {
    pub lines: Vec<usize>,
    pub pairs: Vec<(usize, usize)>,
    pub blocks: Vec<Block>,
}
impl Outline {
    pub fn parse(text: &str, extension: &str) -> Self {
        let mut result = Self {
            lines: vec![0],
            ..Default::default()
        };
        result.lines.extend(
            text.bytes()
                .enumerate()
                .filter(|(_, b)| *b == b'\n')
                .map(|(i, _)| i + 1),
        );
        let bytes = text.as_bytes();
        let mut stack = Vec::new();
        let mut tags: Vec<(String, usize)> = Vec::new();
        let markup = ["xml", "html", "htm"].contains(&extension);
        let mut i = 0;
        while i < bytes.len() {
            if markup && text[i..].starts_with("<!--") {
                let end = text[i + 4..]
                    .find("-->")
                    .map_or(bytes.len(), |n| i + 4 + n + 3);
                result.block(i, end.saturating_sub(1));
                i = end;
                continue;
            }
            if markup && bytes[i] == b'<' {
                let start = i;
                let mut quote = None;
                i += 1;
                while i < bytes.len() {
                    let b = bytes[i];
                    if let Some(q) = quote {
                        if b == q {
                            quote = None;
                        }
                    } else if b == b'\'' || b == b'"' {
                        quote = Some(b);
                    } else if b == b'>' {
                        break;
                    }
                    i += 1;
                }
                if i == bytes.len() {
                    break;
                }
                let tag = text[start + 1..i].trim();
                let closing = tag.starts_with('/');
                let raw_name = tag
                    .trim_start_matches('/')
                    .split(|c: char| c.is_whitespace() || c == '/')
                    .next()
                    .unwrap_or("");
                let name = if extension == "xml" {
                    raw_name.to_owned()
                } else {
                    raw_name.to_ascii_lowercase()
                };
                let void = extension != "xml"
                    && [
                        "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta",
                        "param", "source", "track", "wbr",
                    ]
                    .contains(&name.as_str());
                if closing {
                    if let Some(index) = tags.iter().rposition(|(n, _)| *n == name) {
                        let (_, open) = tags[index].clone();
                        tags.truncate(index);
                        result.block(open, start);
                    }
                } else if !tag.ends_with('/')
                    && !void
                    && name.chars().next().is_some_and(char::is_alphabetic)
                {
                    tags.push((name, start));
                }
                i += 1;
                continue;
            }
            if !markup && bytes[i..].starts_with(b"//") {
                i += 2;
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
                continue;
            }
            if !markup && bytes[i..].starts_with(b"/*") {
                let start = i;
                i += 2;
                while i < bytes.len() && !bytes[i..].starts_with(b"*/") {
                    i += 1;
                }
                i = (i + 2).min(bytes.len());
                result.block(start, i.saturating_sub(1));
                continue;
            }
            if !markup && bytes[i..].starts_with(b"\"\"\"") {
                i += 3;
                while i < bytes.len() && !bytes[i..].starts_with(b"\"\"\"") {
                    i += if bytes[i] == b'\\' && i + 1 < bytes.len() {
                        2
                    } else {
                        1
                    };
                }
                i = (i + 3).min(bytes.len());
                continue;
            }
            if bytes[i] == b'"' || (!markup && bytes[i] == b'\'') {
                let quote = bytes[i];
                i += 1;
                while i < bytes.len() && bytes[i] != quote && bytes[i] != b'\n' {
                    i += if bytes[i] == b'\\' && i + 1 < bytes.len() {
                        2
                    } else {
                        1
                    };
                }
                if i < bytes.len() && bytes[i] == quote {
                    i += 1;
                }
                continue;
            }
            match bytes[i] {
                b'{' | b'[' | b'(' => stack.push((bytes[i], i)),
                b'}' | b']' | b')' => {
                    let expected = match bytes[i] {
                        b'}' => b'{',
                        b']' => b'[',
                        _ => b'(',
                    };
                    if let Some((kind, open)) = stack.pop() {
                        if kind == expected {
                            result.pairs.push((open, i));
                            result.block(open, i);
                        } else {
                            stack.clear();
                        }
                    }
                }
                _ => {}
            }
            // Advancing a full scalar keeps all slices valid for Unicode source.
            i += text[i..].chars().next().unwrap().len_utf8();
        }
        result
            .blocks
            .sort_by_key(|b| (b.start, std::cmp::Reverse(b.end)));
        result
            .blocks
            .dedup_by(|a, b| a.start == b.start && a.end == b.end);
        result.pairs.sort_unstable();
        result
    }
    fn block(&mut self, open: usize, close: usize) {
        let start = self.line_at(open);
        let end = self.line_at(close);
        if end > start + 1 {
            self.blocks.push(Block {
                start,
                end,
                anchor: open,
            });
        }
    }
    pub fn line_at(&self, byte: usize) -> usize {
        self.lines.partition_point(|b| *b <= byte).saturating_sub(1)
    }
    pub fn matching(&self, text: &str, caret: usize) -> Option<(usize, usize)> {
        for position in [Some(caret), caret.checked_sub(1)].into_iter().flatten() {
            if text
                .as_bytes()
                .get(position)
                .is_some_and(|b| b"{}[]()".contains(b))
            {
                if let Some(pair) = self
                    .pairs
                    .iter()
                    .find(|(a, b)| *a == position || *b == position)
                {
                    return Some(*pair);
                }
            }
        }
        None
    }
    pub fn visibility(&self, collapsed: &BTreeSet<usize>) -> Vec<bool> {
        let mut visible = vec![true; self.lines.len()];
        for block in &self.blocks {
            if collapsed.contains(&block.anchor) {
                visible[block.start + 1..block.end].fill(false);
            }
        }
        visible
    }
}
#[derive(Default, Debug)]
pub struct View {
    revision: Option<u64>,
    extension: String,
    pub outline: Outline,
    pub collapsed: BTreeSet<usize>,
}
impl View {
    pub fn update(&mut self, text: &str, extension: &str, revision: u64) {
        if self.revision != Some(revision) || self.extension != extension {
            self.outline = Outline::parse(text, extension);
            self.collapsed.clear();
            self.revision = Some(revision);
            self.extension = extension.into();
        }
    }
    pub fn reveal(&mut self, range: Range<usize>) {
        let first = self.outline.line_at(range.start);
        let last = self.outline.line_at(range.end);
        for block in &self.outline.blocks {
            if first < block.end && last > block.start {
                self.collapsed.remove(&block.anchor);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nested_pairs_ignore_strings_and_comments() {
        let text = "{\n  \"été\": [\n  \"} ] (\",\n  1\n ]\n}";
        let outline = Outline::parse(text, "json");
        assert_eq!(outline.pairs.len(), 2);
        assert_eq!(outline.blocks.len(), 2);
        assert_eq!(outline.matching(text, 0), Some((0, text.len() - 1)));
        let java = Outline::parse(
            "class A { /* } */ String s = \"[\"; char c = ')'; // }\nvoid f() {}\n}",
            "java",
        );
        assert_eq!(java.pairs.len(), 3);
    }
    #[test]
    fn nested_folds_keep_headers_and_closing_lines() {
        let text = "{\n [\n  1,\n  2\n ]\n}";
        let mut view = View::default();
        view.update(text, "json", 0);
        for b in &view.outline.blocks {
            view.collapsed.insert(b.anchor);
        }
        assert_eq!(
            view.outline.visibility(&view.collapsed),
            [true, false, false, false, false, true]
        );
        view.collapsed.remove(&0);
        assert_eq!(
            view.outline.visibility(&view.collapsed),
            [true, true, false, false, true, true]
        );
        view.reveal(7..8);
        assert!(view.collapsed.is_empty());
        assert_eq!(view.outline.lines.len(), 6);
    }
    #[test]
    fn markup_folding_respects_attributes_comments_and_void_tags() {
        let text = "<div title=\"a > b\">\n<br>\n<section>\nhello\n</section>\n</div>";
        let outline = Outline::parse(text, "html");
        assert_eq!(outline.blocks.len(), 2);
        assert_eq!((outline.blocks[0].start, outline.blocks[0].end), (0, 5));
        let xml = Outline::parse("<root>\n<!-- <wrong> -->\n<item/>\n</root>", "xml");
        assert_eq!(xml.blocks.len(), 1);
    }
    #[test]
    fn edits_invalidate_folds_and_unmatched_delimiters_are_not_paired() {
        let mut view = View::default();
        view.update("{\na\n}", "json", 0);
        view.collapsed.insert(0);
        view.update("{\nb\n}", "json", 1);
        assert!(view.collapsed.is_empty());
        assert!(Outline::parse("([)]", "java").pairs.is_empty());
    }
}
