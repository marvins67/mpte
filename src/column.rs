use std::ops::Range;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Point {
    pub row: usize,
    pub column: usize,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rectangle {
    pub rows: Vec<usize>,
    pub columns: Range<usize>,
}
#[derive(Default, Debug)]
pub struct State {
    pub anchor: Option<Point>,
    pub head: Option<Point>,
    pub rectangle: Option<Rectangle>,
    pub dragging: bool,
    pub revision: u64,
}
impl State {
    pub fn clear(&mut self) {
        self.anchor = None;
        self.head = None;
        self.rectangle = None;
        self.dragging = false;
    }
    pub fn validate(&mut self, revision: u64) {
        if self.revision != revision {
            self.clear();
            self.revision = revision;
        }
    }
    pub fn begin(&mut self, point: Point, revision: u64) {
        self.anchor = Some(point);
        self.head = Some(point);
        self.dragging = true;
        self.revision = revision;
        self.rectangle = Some(Rectangle {
            rows: vec![point.row],
            columns: point.column..point.column,
        });
    }
    pub fn extend(&mut self, head: Point, visible: &[bool]) {
        if let Some(anchor) = self.anchor {
            self.head = Some(head);
            self.rectangle = Some(Rectangle {
                rows: (anchor.row.min(head.row)..=anchor.row.max(head.row))
                    .filter(|row| visible.get(*row).copied().unwrap_or(false))
                    .collect(),
                columns: anchor.column.min(head.column)..anchor.column.max(head.column),
            });
        }
    }
}
fn width(grapheme: &str) -> usize {
    if grapheme == "\t" {
        4
    } else {
        grapheme.width()
    }
}
pub fn visual_width(text: &str) -> usize {
    text.graphemes(true).map(width).sum()
}
pub fn byte_at(line: &str, column: usize) -> usize {
    let mut current = 0;
    for (index, grapheme) in line.grapheme_indices(true) {
        if current + width(grapheme) > column {
            return index;
        }
        current += width(grapheme);
    }
    line.len()
}
fn extract(line: &str, columns: Range<usize>) -> String {
    let mut result = String::new();
    let mut current = 0;
    let mut copied = 0;
    for grapheme in line.graphemes(true) {
        let end = current + width(grapheme);
        let overlap = end
            .min(columns.end)
            .saturating_sub(current.max(columns.start));
        if overlap > 0 {
            if grapheme == "\t" {
                result.push_str(&" ".repeat(overlap));
                copied += overlap;
            } else {
                result.push_str(grapheme);
                copied += width(grapheme);
            }
        }
        current = end;
    }
    result.push_str(&" ".repeat(columns.len().saturating_sub(copied)));
    result
}
pub fn copy(text: &str, rectangle: &Rectangle) -> String {
    let lines: Vec<_> = text.split('\n').collect();
    rectangle
        .rows
        .iter()
        .map(|row| {
            extract(
                lines.get(*row).copied().unwrap_or(""),
                rectangle.columns.clone(),
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}
fn replace_line(line: &str, columns: Range<usize>, replacement: &str) -> String {
    if columns.start >= visual_width(line) && replacement.is_empty() {
        return line.to_owned();
    }
    let mut prefix = String::new();
    let mut suffix = String::new();
    let mut current = 0;
    for grapheme in line.graphemes(true) {
        let end = current + width(grapheme);
        if end <= columns.start {
            prefix.push_str(grapheme);
        } else if current >= columns.end {
            suffix.push_str(grapheme);
        } else if grapheme == "\t" {
            prefix.push_str(&" ".repeat(columns.start.saturating_sub(current).min(4)));
            suffix.push_str(&" ".repeat(end.saturating_sub(columns.end).min(4)));
        } else if columns.is_empty() {
            suffix.push_str(grapheme);
        }
        current = end;
    }
    if !replacement.is_empty() {
        prefix.push_str(&" ".repeat(columns.start.saturating_sub(current)));
    }
    prefix + replacement + &suffix
}
/// A single input line repeats on each selected row; multiline input is distributed
/// row by row, extending the document if necessary. Only selected columns change.
pub fn replace(text: &mut String, rectangle: &Rectangle, input: &str) -> usize {
    if rectangle.rows.is_empty() {
        return rectangle.columns.start;
    }
    let input = input.replace("\r\n", "\n");
    let replacements: Vec<_> = input.split('\n').collect();
    let mut lines: Vec<String> = text.split('\n').map(str::to_owned).collect();
    let count = rectangle.rows.len().max(replacements.len());
    for index in 0..count {
        let (row, columns) = if let Some(row) = rectangle.rows.get(index) {
            (*row, rectangle.columns.clone())
        } else {
            (
                rectangle.rows.last().unwrap() + index - rectangle.rows.len() + 1,
                rectangle.columns.start..rectangle.columns.start,
            )
        };
        while lines.len() <= row {
            lines.push(String::new());
        }
        let replacement = if replacements.len() == 1 {
            replacements[0]
        } else {
            replacements.get(index).copied().unwrap_or("")
        };
        lines[row] = replace_line(&lines[row], columns, replacement);
    }
    *text = lines.join("\n");
    rectangle.columns.start
        + replacements
            .iter()
            .map(|s| visual_width(s))
            .max()
            .unwrap_or(0)
}

use crate::{Id, document::Document};
use eframe::egui::{self, Color32, Rect, Shape, Stroke, pos2};
pub struct Geometry<'a> {
    pub galley: &'a egui::Galley,
    pub origin: egui::Pos2,
    pub clip: Rect,
    pub space: f32,
    pub visible: &'a [bool],
    pub layer: egui::LayerId,
}
impl Geometry<'_> {
    fn point(&self, position: egui::Pos2) -> Point {
        let y = position.y - self.origin.y;
        let row = self
            .galley
            .rows
            .iter()
            .enumerate()
            .filter(|(i, _)| self.visible.get(*i).copied().unwrap_or(true))
            .min_by(|(_, a), (_, b)| {
                let distance = |r: &egui::epaint::text::Row| {
                    if y < r.rect.top() {
                        r.rect.top() - y
                    } else if y > r.rect.bottom() {
                        y - r.rect.bottom()
                    } else {
                        0.0
                    }
                };
                distance(a).total_cmp(&distance(b))
            })
            .map_or(0, |(i, _)| i);
        Point {
            row,
            column: ((position.x - self.origin.x) / self.space).round().max(0.0) as usize,
        }
    }
    fn contains(&self, ctx: &egui::Context, position: egui::Pos2) -> bool {
        self.clip.contains(position)
            && position.x >= self.origin.x
            && ctx.layer_id_at(position) == Some(self.layer)
    }
}
fn apply(ctx: &egui::Context, id: Id, doc: &mut Document, rectangle: Rectangle, input: &str) {
    crate::editor::record_undo(ctx, id, &doc.text);
    let previous = doc.text.clone();
    let column = replace(&mut doc.text, &rectangle, input);
    if doc.text != previous {
        doc.revision += 1;
        doc.view.collapsed.clear();
    }
    let count = input.replace("\r\n", "\n").split('\n').count();
    let mut rows = rectangle.rows;
    if let Some(&last) = rows.last() {
        for offset in 1..=count.saturating_sub(rows.len()) {
            rows.push(last + offset);
        }
    }
    if let Some(head) = &mut doc.column.head {
        head.column = column;
    }
    if let Some(anchor) = &mut doc.column.anchor {
        anchor.column = column;
    }
    doc.column.rectangle = Some(Rectangle {
        rows,
        columns: column..column,
    });
    doc.column.revision = doc.revision;
    doc.column.dragging = false;
    ctx.request_repaint();
}
/// Consume only column gestures and column edits. TextEdit remains responsible
/// for normal selection, platform shortcuts, IME, navigation and undo/redo.
pub fn input(ctx: &egui::Context, id: Id, doc: &mut Document, geometry: &Geometry<'_>) {
    let editor_id = egui::Id::new(("text", id));
    if !ctx.input(|i| i.focused) {
        doc.column.dragging = false;
    }
    let events = ctx.input_mut(|i| std::mem::take(&mut i.events));
    let mut remaining = Vec::new();
    for event in events {
        match &event {
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers,
            } if geometry.contains(ctx, *pos) => {
                if modifiers.alt {
                    doc.column.begin(geometry.point(*pos), doc.revision);
                    ctx.request_repaint();
                    ctx.memory_mut(|m| m.request_focus(editor_id));
                    continue;
                }
                doc.column.clear();
            }
            egui::Event::PointerMoved(position) if doc.column.dragging => {
                doc.column
                    .extend(geometry.point(*position), geometry.visible);
                continue;
            }
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                ..
            } if doc.column.dragging => {
                doc.column.extend(geometry.point(*pos), geometry.visible);
                doc.column.dragging = false;
                ctx.request_repaint();
                continue;
            }
            _ => {}
        }
        if ctx.memory(|m| m.has_focus(editor_id)) {
            if let Some(rectangle) = doc.column.rectangle.clone() {
                match &event {
                    egui::Event::Copy | egui::Event::Cut => {
                        if !rectangle.columns.is_empty() {
                            ctx.copy_text(copy(&doc.text, &rectangle));
                            if matches!(event, egui::Event::Cut) {
                                apply(ctx, id, doc, rectangle, "");
                            }
                        }
                        continue;
                    }
                    egui::Event::Text(value)
                    | egui::Event::Paste(value)
                    | egui::Event::Ime(egui::ImeEvent::Commit(value)) => {
                        apply(ctx, id, doc, rectangle, value);
                        continue;
                    }
                    egui::Event::Key {
                        key,
                        pressed: true,
                        modifiers,
                        ..
                    } => match key {
                        egui::Key::Backspace | egui::Key::Delete => {
                            let mut rectangle = rectangle;
                            if rectangle.columns.is_empty() {
                                let c = rectangle.columns.start;
                                rectangle.columns = if *key == egui::Key::Backspace {
                                    c.saturating_sub(1)..c
                                } else {
                                    c..c + 1
                                };
                            }
                            apply(ctx, id, doc, rectangle, "");
                            continue;
                        }
                        egui::Key::Tab => {
                            apply(ctx, id, doc, rectangle, "\t");
                            continue;
                        }
                        egui::Key::Escape => {
                            doc.column.clear();
                            continue;
                        }
                        egui::Key::ArrowUp
                        | egui::Key::ArrowDown
                        | egui::Key::ArrowLeft
                        | egui::Key::ArrowRight
                        | egui::Key::Home
                        | egui::Key::End
                        | egui::Key::PageUp
                        | egui::Key::PageDown
                        | egui::Key::Enter => doc.column.clear(),
                        egui::Key::Z | egui::Key::A if modifiers.command => doc.column.clear(),
                        _ => {}
                    },
                    _ => {}
                }
            }
        }
        remaining.push(event);
    }
    ctx.input_mut(|i| i.events = remaining);
    if let Some(head) = doc.column.head {
        let lines: Vec<_> = doc.text.split('\n').collect();
        let row = head.row.min(lines.len().saturating_sub(1));
        let byte = lines.iter().take(row).map(|s| s.len() + 1).sum::<usize>()
            + byte_at(lines[row], head.column);
        let cursor = egui::text::CCursor::new(doc.text[..byte].chars().count());
        let mut state = egui::TextEdit::load_state(ctx, editor_id).unwrap_or_default();
        state
            .cursor
            .set_char_range(Some(egui::text::CCursorRange::one(cursor)));
        state.store(ctx, editor_id);
    }
}
pub fn paint(
    ctx: &egui::Context,
    ui: &egui::Ui,
    shape: egui::layers::ShapeIdx,
    doc: &Document,
    output: &egui::text_edit::TextEditOutput,
    space: f32,
) {
    let Some(rectangle) = &doc.column.rectangle else {
        return;
    };
    let focused = ctx.memory(|m| m.has_focus(output.response.id));
    let mut shapes = Vec::new();
    let x1 = output.galley_pos.x + rectangle.columns.start as f32 * space;
    let x2 = output.galley_pos.x + rectangle.columns.end as f32 * space;
    for row in &rectangle.rows {
        if let Some(row) = output.galley.rows.get(*row) {
            if row.height() <= 0.0 {
                continue;
            }
            let rect = Rect::from_min_max(
                pos2(x1, output.galley_pos.y + row.rect.top()),
                pos2(x2.max(x1 + 1.0), output.galley_pos.y + row.rect.bottom()),
            );
            if !ui.clip_rect().intersects(rect) {
                continue;
            }
            if rectangle.columns.is_empty() {
                shapes.push(Shape::line_segment(
                    [rect.left_top(), rect.left_bottom()],
                    Stroke::new(1.0, Color32::from_rgb(40, 90, 175)),
                ));
            } else {
                shapes.push(Shape::rect_filled(
                    rect,
                    0.0,
                    if focused {
                        Color32::from_rgb(180, 207, 243)
                    } else {
                        Color32::from_gray(210)
                    },
                ));
            }
        }
    }
    ui.painter().set(shape, Shape::Vec(shapes));
}

#[cfg(test)]
mod tests {
    use super::*;
    fn rect(columns: Range<usize>, rows: Vec<usize>) -> Rectangle {
        Rectangle { rows, columns }
    }
    #[test]
    fn rectangular_copy_and_delete_preserve_other_columns_and_newlines() {
        let mut text = "ab123cd\nef456gh\nij789kl\n".to_owned();
        let rectangle = rect(2..5, vec![0, 1, 2]);
        assert_eq!(copy(&text, &rectangle), "123\n456\n789");
        replace(&mut text, &rectangle, "");
        assert_eq!(text, "abcd\nefgh\nijkl\n");
    }
    #[test]
    fn short_lines_virtual_space_tabs_and_unicode() {
        let mut text = "é\nx\tend\ne\u{301}té".to_owned();
        let rectangle = rect(2..4, vec![0, 1, 2]);
        assert_eq!(copy(&text, &rectangle), "  \n  \né ");
        replace(&mut text, &rectangle, "Z");
        assert_eq!(text, "é Z\nx Z end\ne\u{301}tZ");
    }
    #[test]
    fn multiline_paste_distributes_rows_and_preserves_suffix() {
        let mut text = "abcDEFghi\njklMNOxyz".to_owned();
        replace(&mut text, &rect(3..6, vec![0, 1]), "1\r\n2");
        assert_eq!(text, "abc1ghi\njkl2xyz");
        replace(&mut text, &rect(3..3, vec![1]), "A\nB");
        assert_eq!(text, "abc1ghi\njklA2xyz\n   B");
    }
    #[test]
    fn reverse_drag_skips_folded_rows_and_invalidation_clears_selection() {
        let mut state = State::default();
        state.begin(Point { row: 4, column: 8 }, 1);
        state.extend(
            Point { row: 1, column: 2 },
            &[true, true, false, false, true],
        );
        assert_eq!(state.rectangle, Some(rect(2..8, vec![1, 4])));
        state.validate(2);
        assert!(state.rectangle.is_none());
    }
}
