//! Per-character score typography and A4 geometry, independent of the plain-text buffer.
use eframe::egui::{self, Color32, FontId, Rect, Shape, Stroke, pos2, vec2};
use serde::{Deserialize, Serialize};
use std::{ops::Range, sync::Arc};

pub const DEFAULT_SIZE: f32 = 14.0;
pub const MIN_SIZE: f32 = 8.0;
pub const MAX_SIZE: f32 = 72.0;
// Typographic points: A4 is exactly 210 × 297 mm, with 15 mm margins.
pub const PAGE_WIDTH: f32 = 210.0 * 72.0 / 25.4;
pub const PAGE_HEIGHT: f32 = 297.0 * 72.0 / 25.4;
pub const MARGIN: f32 = 15.0 * 72.0 / 25.4;
pub const PAGE_GAP: f32 = 28.0;
pub const CONTENT_WIDTH: f32 = PAGE_WIDTH - 2.0 * MARGIN;
const CONTENT_HEIGHT: f32 = PAGE_HEIGHT - 2.0 * MARGIN;

pub fn font_family() -> egui::FontFamily {
    egui::FontFamily::Name("Roboto Mono".into())
}

/// Read the primary caret, rather than the sorted start of a selection.
pub fn caret(ctx: &egui::Context, id: crate::Id, text: &str) -> usize {
    let index = egui::TextEdit::load_state(ctx, egui::Id::new(("text", id)))
        .and_then(|s| s.cursor.char_range())
        .map_or(0, |r| r.primary.index);
    text.char_indices()
        .nth(index)
        .map_or(text.len(), |(i, _)| i)
}

const SEPARATOR: &str = "─";

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
struct LineStyle {
    centered: bool,
    separator: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Run {
    chars: usize,
    size: f32,
}

/// The text fingerprint is deliberately the full source: stale metadata is never
/// applied to unrelated characters after a file was changed in another editor.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Data {
    version: u8,
    text: String,
    active: bool,
    runs: Vec<Run>,
    #[serde(default)]
    lines: Vec<LineStyle>,
}

#[derive(Clone, Debug, PartialEq)]
struct Snapshot {
    text: String,
    sizes: Vec<f32>,
    lines: Vec<LineStyle>,
    selection: Range<usize>,
}

#[derive(Debug)]
pub struct Partition {
    pub active: bool,
    text: String,
    sizes: Vec<f32>,
    lines: Vec<LineStyle>,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
}

impl Partition {
    pub fn new(text: &str) -> Self {
        Self {
            active: true,
            text: text.into(),
            sizes: vec![DEFAULT_SIZE; text.chars().count()],
            lines: vec![LineStyle::default(); text.split('\n').count()],
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }

    pub fn from_data(mut data: Data, text: &str) -> Result<Self, String> {
        // Companions created before line formatting only contain font sizes.
        if data.lines.is_empty() {
            data.lines = vec![LineStyle::default(); text.split('\n').count()];
        }
        let valid = data.version == 1
            && data.text == text
            && data.lines.len() == text.split('\n').count()
            && data
                .lines
                .iter()
                .zip(text.split('\n'))
                .all(|(style, line)| !style.separator || (line == SEPARATOR && !style.centered))
            && data.runs.iter().all(|run| {
                run.chars > 0 && run.size.is_finite() && (MIN_SIZE..=MAX_SIZE).contains(&run.size)
            })
            && data
                .runs
                .iter()
                .try_fold(0_usize, |n, r| n.checked_add(r.chars))
                == Some(text.chars().count());
        if !valid {
            return Err("Le fichier compagnon Partition est invalide ou ne correspond plus au texte. La mise en page n’a pas été chargée.".into());
        }
        Ok(Self {
            active: data.active,
            text: text.into(),
            sizes: data
                .runs
                .iter()
                .flat_map(|r| std::iter::repeat_n(r.size, r.chars))
                .collect(),
            lines: data.lines,
            undo: Vec::new(),
            redo: Vec::new(),
        })
    }

    pub fn data(&self) -> Data {
        let mut runs: Vec<Run> = Vec::new();
        for &size in &self.sizes {
            if let Some(last) = runs.last_mut().filter(|r| r.size == size) {
                last.chars += 1;
            } else {
                runs.push(Run { chars: 1, size });
            }
        }
        Data {
            version: 1,
            text: self.text.clone(),
            active: self.active,
            runs,
            lines: self.lines.clone(),
        }
    }

    fn snapshot(&self, selection: Range<usize>) -> Snapshot {
        Snapshot {
            text: self.text.clone(),
            sizes: self.sizes.clone(),
            lines: self.lines.clone(),
            selection,
        }
    }

    fn checkpoint(&mut self, selection: Range<usize>) {
        self.undo.push(self.snapshot(selection));
        if self.undo.len() > 64 {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    /// Carry sizes through arbitrary UTF-8 edits (typing, paste, search, formatter).
    /// Insertions inherit the replaced character, or the preceding character.
    pub fn sync(&mut self, text: &str, selection: Range<usize>) {
        if text == self.text {
            return;
        }
        self.checkpoint(selection);
        let prefix = self
            .text
            .chars()
            .zip(text.chars())
            .take_while(|(a, b)| a == b)
            .count();
        let old_len = self.sizes.len();
        let new_len = text.chars().count();
        let suffix = self
            .text
            .chars()
            .rev()
            .zip(text.chars().rev())
            .take((old_len - prefix).min(new_len - prefix))
            .take_while(|(a, b)| a == b)
            .count();
        let size = self
            .sizes
            .get(prefix)
            .or_else(|| prefix.checked_sub(1).and_then(|i| self.sizes.get(i)))
            .copied()
            .unwrap_or(DEFAULT_SIZE);
        self.sizes.splice(
            prefix..old_len - suffix,
            std::iter::repeat_n(size, new_len - prefix - suffix),
        );
        // Keep unchanged logical lines attached to their formatting. A split
        // keeps the first fragment's alignment; new following lines start left.
        let old_lines: Vec<_> = self.text.split('\n').collect();
        let new_lines: Vec<_> = text.split('\n').collect();
        let prefix = old_lines
            .iter()
            .zip(&new_lines)
            .take_while(|(a, b)| a == b)
            .count();
        let suffix = old_lines
            .iter()
            .rev()
            .zip(new_lines.iter().rev())
            .take((old_lines.len() - prefix).min(new_lines.len() - prefix))
            .take_while(|(a, b)| a == b)
            .count();
        let inherited = if prefix < old_lines.len() - suffix {
            self.lines[prefix].clone()
        } else {
            LineStyle::default()
        };
        let added = new_lines.len() - prefix - suffix;
        self.lines.splice(
            prefix..old_lines.len() - suffix,
            (0..added).map(|i| {
                if i == 0 {
                    inherited.clone()
                } else {
                    LineStyle::default()
                }
            }),
        );
        for (style, line) in self.lines.iter_mut().zip(new_lines) {
            if line != SEPARATOR {
                style.separator = false;
            }
        }
        self.text = text.into();
    }

    fn line_at(&self, caret: usize) -> Option<usize> {
        Some(
            self.text
                .get(..caret)?
                .bytes()
                .filter(|b| *b == b'\n')
                .count(),
        )
    }

    pub fn is_centered(&self, caret: usize) -> bool {
        self.line_at(caret)
            .is_some_and(|line| self.lines[line].centered)
    }

    pub fn toggle_center(&mut self, caret: usize, selection: Range<usize>) {
        if let Some(line) = self.line_at(caret) {
            if !self.lines[line].separator {
                self.checkpoint(selection);
                self.lines[line].centered = !self.lines[line].centered;
            }
        }
    }

    /// Insert after the current line, or on the current empty line. Leave a
    /// normal editable line below the rule, without replacing selected text.
    pub fn insert_separator(
        &mut self,
        text: &mut String,
        caret: usize,
        selection: Range<usize>,
    ) -> Option<Range<usize>> {
        let before = text.get(..caret)?;
        let after = text.get(caret..)?;
        let start = before.rfind('\n').map_or(0, |i| i + 1);
        let end = caret + after.find('\n').unwrap_or(after.len());
        let prefix = if start == end { "" } else { "\n" };
        let suffix = if end == text.len() { "\n" } else { "" };
        text.insert_str(end, &format!("{prefix}{SEPARATOR}{suffix}"));
        self.sync(text, selection);
        let rule = end + prefix.len();
        let line = self.line_at(rule)?;
        self.lines[line] = LineStyle {
            centered: false,
            separator: true,
        };
        if !suffix.is_empty() {
            self.lines[line + 1] = LineStyle::default();
        }
        let next = rule + SEPARATOR.len() + 1;
        Some(next..next)
    }

    fn char_range(&self, range: Range<usize>) -> Option<Range<usize>> {
        if range.is_empty() || range.start > range.end {
            return None;
        }
        let start = self.text.get(..range.start)?.chars().count();
        let end = start + self.text.get(range)?.chars().count();
        Some(start..end)
    }

    pub fn resize(&mut self, range: Range<usize>, delta: f32, reset: bool) -> bool {
        let Some(chars) = self.char_range(range.clone()) else {
            return false;
        };
        let change = |size: f32| {
            if reset {
                DEFAULT_SIZE
            } else {
                (size + delta).clamp(MIN_SIZE, MAX_SIZE)
            }
        };
        if self.sizes[chars.clone()]
            .iter()
            .all(|&size| change(size) == size)
        {
            return false;
        }
        self.checkpoint(range);
        for size in &mut self.sizes[chars] {
            *size = change(*size);
        }
        true
    }

    pub fn selection_size(&self, range: Range<usize>) -> String {
        let Some(chars) = self.char_range(range) else {
            return "Sélectionnez du texte".into();
        };
        let slice = &self.sizes[chars];
        if slice.iter().all(|s| *s == slice[0]) {
            format!("{} pt", slice[0])
        } else {
            "Tailles mixtes".into()
        }
    }

    pub fn undo(
        &mut self,
        text: &mut String,
        selection: Range<usize>,
        redo: bool,
    ) -> Option<Range<usize>> {
        let next = if redo {
            self.redo.pop()
        } else {
            self.undo.pop()
        }?;
        let current = self.snapshot(selection);
        if redo {
            self.undo.push(current);
        } else {
            self.redo.push(current);
        }
        self.text = next.text;
        self.sizes = next.sizes;
        self.lines = next.lines;
        text.clone_from(&self.text);
        Some(next.selection)
    }

    pub fn layout(&self, text: &str) -> egui::text::LayoutJob {
        let mut job = egui::text::LayoutJob {
            text: text.into(),
            ..Default::default()
        };
        job.wrap.max_width = f32::INFINITY;
        let mut end = 0;
        let mut current = None;
        for (index, (byte, ch)) in text.char_indices().enumerate() {
            let size = self.sizes.get(index).copied().unwrap_or(DEFAULT_SIZE);
            if current != Some(size) {
                if let Some(last) = job.sections.last_mut() {
                    last.byte_range.end = byte;
                }
                job.sections.push(egui::text::LayoutSection {
                    leading_space: 0.0,
                    byte_range: byte..byte,
                    format: egui::TextFormat {
                        font_id: FontId::new(size, font_family()),
                        color: Color32::from_rgb(36, 45, 58),
                        ..Default::default()
                    },
                });
                current = Some(size);
            }
            end = byte + ch.len_utf8();
        }
        if let Some(last) = job.sections.last_mut() {
            last.byte_range.end = end;
        }
        // An empty score still needs a Roboto Mono caret and a full A4 sheet.
        if job.sections.is_empty() {
            job.sections.push(egui::text::LayoutSection {
                leading_space: 0.0,
                byte_range: 0..0,
                format: egui::TextFormat {
                    font_id: FontId::new(DEFAULT_SIZE, font_family()),
                    ..Default::default()
                },
            });
        }
        job
    }
}

/// Shift whole rows onto successive pages without inserting source characters.
/// TextEdit uses this same geometry for hit testing, selection, caret and IME.
pub fn paginate(galley: Arc<egui::Galley>, partition: &Partition) -> Arc<egui::Galley> {
    let mut result = (*galley).clone();
    let mut page = 0;
    let mut used = 0.0;
    result.mesh_bounds = Rect::NOTHING;
    result.rect.max.x = 0.0;
    for (index, row) in result.rows.iter_mut().enumerate() {
        let height = row.height();
        if used > 0.0 && used + height > CONTENT_HEIGHT {
            page += 1;
            used = 0.0;
        }
        let top = page as f32 * (PAGE_HEIGHT + PAGE_GAP) + used;
        let style = partition.lines.get(index);
        let separator = style.is_some_and(|s| s.separator) && row.text() == SEPARATOR;
        if separator {
            row.rect.max.x = row.rect.min.x + CONTENT_WIDTH;
            row.glyphs[0].advance_width = CONTENT_WIDTH;
            row.visuals = Default::default();
            let rect = Rect::from_min_size(
                pos2(0.0, row.rect.center().y - 0.5),
                vec2(CONTENT_WIDTH, 1.0),
            );
            row.visuals.mesh.add_colored_rect(rect, crate::theme::TEXT);
            row.visuals.mesh_bounds = rect;
        }
        let x = if style.is_some_and(|s| s.centered) && !separator {
            ((CONTENT_WIDTH - row.rect.width()) / 2.0).max(0.0)
        } else {
            0.0
        };
        let shift = vec2(x, top - row.rect.top());
        row.rect = row.rect.translate(shift);
        for glyph in &mut row.glyphs {
            glyph.pos += shift;
        }
        row.visuals.mesh.translate(shift);
        row.visuals.mesh_bounds = row.visuals.mesh_bounds.translate(shift);
        result.mesh_bounds = result.mesh_bounds.union(row.visuals.mesh_bounds);
        result.rect.max.x = result.rect.max.x.max(row.rect.right());
        used += height;
    }
    result.rect.max.y = page as f32 * (PAGE_HEIGHT + PAGE_GAP) + used;
    result.num_vertices = result
        .rows
        .iter()
        .map(|r| r.visuals.mesh.vertices.len())
        .sum();
    result.num_indices = result
        .rows
        .iter()
        .map(|r| r.visuals.mesh.indices.len())
        .sum();
    Arc::new(result)
}

pub fn page_count(galley: &egui::Galley) -> usize {
    galley.rows.last().map_or(1, |row| {
        (row.rect.top() / (PAGE_HEIGHT + PAGE_GAP)).floor() as usize + 1
    })
}

pub fn show(
    ctx: &egui::Context,
    ui: &mut egui::Ui,
    id: crate::Id,
    doc: &mut crate::document::Document,
    job: egui::text::LayoutJob,
    selection: &mut Range<usize>,
    jump: &mut Option<(crate::Id, Range<usize>)>,
) {
    let editor_id = egui::Id::new(("text", id));
    let previous_selection = selection.clone();
    let requested = jump
        .take()
        .filter(|(did, r)| *did == id && doc.text.get(r.clone()).is_some());
    if let Some((_, range)) = &requested {
        let mut state = egui::TextEdit::load_state(ctx, editor_id).unwrap_or_default();
        state
            .cursor
            .set_char_range(Some(egui::text::CCursorRange::two(
                egui::text::CCursor::new(doc.text[..range.start].chars().count()),
                egui::text::CCursor::new(doc.text[..range.end].chars().count()),
            )));
        state.store(ctx, editor_id);
    }
    egui::ScrollArea::both()
        .id_salt(("score", id))
        .auto_shrink([false, false])
        .drag_to_scroll(false)
        .show(ui, |ui| {
            let start = ui.next_widget_position() + vec2(20.0, 20.0);
            let left = start.x + ((ui.available_width() - PAGE_WIDTH - 40.0) / 2.0).max(0.0);
            let page_origin = pos2(left, start.y);
            let text_origin = page_origin + vec2(MARGIN, MARGIN);
            let background = ui.painter().add(Shape::Noop);
            let mut layouter = |ui: &egui::Ui, text: &str, _: f32| {
                let layout = if job.text == text {
                    job.clone()
                } else {
                    doc.partition.as_ref().unwrap().layout(text)
                };
                paginate(
                    ui.fonts(|f| f.layout_job(layout)),
                    doc.partition.as_ref().unwrap(),
                )
            };
            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(Rect::from_min_size(
                        text_origin,
                        vec2(CONTENT_WIDTH, ui.available_height()),
                    ))
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            );
            let output = egui::TextEdit::multiline(&mut doc.text)
                .id(editor_id)
                .font(FontId::new(DEFAULT_SIZE, font_family()))
                .code_editor()
                .frame(false)
                .margin(egui::Margin::ZERO)
                .desired_width(CONTENT_WIDTH)
                .desired_rows(1)
                .layouter(&mut layouter)
                .show(&mut child);
            let pages = page_count(&output.galley);
            let mut shapes = Vec::new();
            let clip = ui.clip_rect();
            for page in 0..pages {
                let rect = Rect::from_min_size(
                    page_origin + vec2(0.0, page as f32 * (PAGE_HEIGHT + PAGE_GAP)),
                    vec2(PAGE_WIDTH, PAGE_HEIGHT),
                );
                if !rect.intersects(clip) {
                    continue;
                }
                shapes.push(Shape::rect_filled(
                    rect.translate(vec2(3.0, 3.0)),
                    0.0,
                    Color32::from_gray(207),
                ));
                shapes.push(Shape::rect_filled(rect, 0.0, Color32::WHITE));
                shapes.push(Shape::rect_stroke(
                    rect,
                    0.0,
                    Stroke::new(1.0, crate::theme::INPUT_BORDER),
                    egui::StrokeKind::Inside,
                ));
                shapes.push(Shape::rect_stroke(
                    rect.shrink(MARGIN),
                    0.0,
                    Stroke::new(1.0, crate::theme::BORDER),
                    egui::StrokeKind::Inside,
                ));
                ui.painter().text(
                    pos2(rect.center().x, rect.bottom() - 18.0),
                    egui::Align2::CENTER_CENTER,
                    format!("{} / {}", page + 1, pages),
                    FontId::proportional(10.0),
                    crate::theme::MUTED,
                );
            }
            ui.painter().set(background, Shape::Vec(shapes));
            let mut overflow = 0;
            for row in &output.galley.rows {
                if row.rect.width() > CONTENT_WIDTH {
                    overflow += 1;
                    let y = output.galley_pos.y + row.rect.center().y;
                    if y >= clip.top() && y <= clip.bottom() {
                        let x = text_origin.x + CONTENT_WIDTH;
                        ui.painter().line_segment(
                            [
                                pos2(x, y - row.height() / 2.0),
                                pos2(x, y + row.height() / 2.0),
                            ],
                            Stroke::new(2.0, Color32::RED),
                        );
                    }
                }
            }
            let bounds = Rect::from_min_size(
                start,
                vec2(
                    (left - start.x + PAGE_WIDTH).max(output.response.rect.right() - start.x)
                        + 20.0,
                    pages as f32 * (PAGE_HEIGHT + PAGE_GAP) - PAGE_GAP + 20.0,
                ),
            );
            ui.allocate_rect(bounds, egui::Sense::hover());
            if let Some(range) = output.state.cursor.char_range() {
                let byte = |c: usize| {
                    doc.text
                        .char_indices()
                        .nth(c)
                        .map_or(doc.text.len(), |(i, _)| i)
                };
                let a = byte(range.primary.index);
                let b = byte(range.secondary.index);
                *selection = a.min(b)..a.max(b);
            }
            if let Some((_, range)) = requested {
                let c = egui::text::CCursor::new(doc.text[..range.start].chars().count());
                ui.scroll_to_rect(
                    output
                        .galley
                        .pos_from_ccursor(c)
                        .translate(output.galley_pos.to_vec2()),
                    Some(egui::Align::Center),
                );
            }
            doc.partition_pages = pages;
            doc.partition_overflow = overflow;
            if output.response.changed() {
                doc.revision += 1;
                doc.partition
                    .as_mut()
                    .unwrap()
                    .sync(&doc.text, previous_selection.clone());
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn centering_tracks_unicode_lines_edits_splits_and_undo() {
        let mut text = "été\n🦀 Am\nfin".to_string();
        let mut score = Partition::new(&text);
        score.toggle_center(6, 0..text.len());
        assert!(!score.is_centered(0));
        assert!(score.is_centered(6));
        assert!(!score.is_centered(text.len()));
        score.sync("Intro\nété\n🦀 Am\nfin", 0..0);
        assert!(score.is_centered(12));
        score.sync("Intro\nété\n🦀\n Am\nfin", 12..12);
        assert!(score.is_centered(12));
        assert!(!score.is_centered(17));
        score.undo(&mut text, 0..0, false);
        assert_eq!(text, "Intro\nété\n🦀 Am\nfin");
        assert!(score.is_centered(12));
        score.undo(&mut text, 0..0, false);
        score.undo(&mut text, 0..0, false);
        assert!(!score.is_centered(6));
        score.undo(&mut text, 0..0, true);
        assert!(score.is_centered(6));
        score.toggle_center(6, 6..6);
        assert!(!score.is_centered(6));
        score.toggle_center(7, 0..0); // inside the crab's UTF-8 encoding
        assert!(!score.is_centered(6));
        let mut score = Partition::new("Titre");
        score.toggle_center(0, 0..0);
        score.sync("\nTitre", 0..0);
        assert!(!score.is_centered(0));
        assert!(score.is_centered(1));
        score.sync("\nTitre\n", 6..6);
        assert!(!score.is_centered(7));
    }

    #[test]
    fn separator_insertion_empty_middle_eof_and_undo_redo() {
        for (original, caret, expected) in [
            ("", 0, "─\n"),
            ("été", 2, "été\n─\n"),
            ("a\nb", 0, "a\n─\nb"),
            ("a\n\nb", 2, "a\n─\nb"),
            ("a\n", 2, "a\n─\n"),
        ] {
            let mut text = original.to_string();
            let mut score = Partition::new(&text);
            score.toggle_center(caret, caret..caret);
            let centered = score.data();
            let next = score
                .insert_separator(&mut text, caret, 0..original.len())
                .unwrap();
            assert_eq!(text, expected);
            assert!(text.is_char_boundary(next.start));
            assert!(!score.is_centered(next.start));
            let rule = text.find(SEPARATOR).unwrap();
            assert!(score.lines[score.line_at(rule).unwrap()].separator);
            let data = score.data();
            score.undo(&mut text, next.clone(), false);
            assert_eq!(text, original);
            assert_eq!(score.data(), centered);
            assert_eq!(score.undo(&mut text, 0..0, true), Some(next));
            assert_eq!(score.data(), data);
            // Editing a rule into ordinary text removes its graphical status.
            text.insert(rule, 'x');
            score.sync(&text, rule..rule);
            assert!(!score.lines[score.line_at(rule).unwrap()].separator);
        }
    }

    #[test]
    fn line_metadata_roundtrip_accepts_old_companions_and_rejects_invalid_rules() {
        let mut text = "Titre\nfin".to_string();
        let mut score = Partition::new(&text);
        score.toggle_center(0, 0..0);
        score.insert_separator(&mut text, 0, 0..0);
        let data = score.data();
        let restored = Partition::from_data(data.clone(), &text).unwrap();
        assert_eq!(restored.data(), data);
        let mut legacy = serde_json::to_value(&data).unwrap();
        legacy.as_object_mut().unwrap().remove("lines");
        let restored =
            Partition::from_data(serde_json::from_value(legacy).unwrap(), &text).unwrap();
        assert!(
            restored
                .lines
                .iter()
                .all(|line| *line == LineStyle::default())
        );
        let mut invalid = data.clone();
        invalid.lines[0].separator = true;
        assert!(Partition::from_data(invalid, &text).is_err());
        let mut invalid = data;
        invalid.lines.pop();
        assert!(Partition::from_data(invalid, &text).is_err());
    }

    #[test]
    fn centered_caret_and_full_width_rule_geometry_follow_page_margins() {
        let ctx = egui::Context::default();
        crate::theme::install(&ctx);
        let _ = ctx.run(Default::default(), |ctx| {
            let mut text = "été 🦀\nfin".to_string();
            let mut score = Partition::new(&text);
            score.toggle_center(0, 0..0);
            score.insert_separator(&mut text, 0, 0..0);
            let galley = paginate(ctx.fonts(|f| f.layout_job(score.layout(&text))), &score);
            let first = &galley.rows[0];
            assert!((first.rect.center().x - CONTENT_WIDTH / 2.0).abs() < 0.01);
            assert!(first.rect.left() > 0.0);
            let caret = galley.pos_from_ccursor(egui::text::CCursor::new(0));
            assert_eq!(caret.left(), first.rect.left());
            assert_eq!(
                galley
                    .cursor_from_pos(caret.center().to_vec2())
                    .ccursor
                    .index,
                0
            );
            let rule = &galley.rows[1];
            assert_eq!(rule.rect.width(), CONTENT_WIDTH);
            assert_eq!(rule.visuals.mesh_bounds.width(), CONTENT_WIDTH);
            assert_eq!(rule.x_offset(1), CONTENT_WIDTH);
            assert_eq!(galley.rows[2].rect.left(), 0.0);
            assert_eq!(galley.end().ccursor.index, text.chars().count());
        });
    }

    #[test]
    fn unicode_selection_changes_only_selected_characters_and_survives_edits() {
        let mut score = Partition::new("été 🦀 accord");
        assert!(score.resize(6..10, 6.0, false)); // the crab is four bytes, one character
        assert_eq!(
            score.sizes,
            [14., 14., 14., 14., 20., 14., 14., 14., 14., 14., 14., 14.]
        );
        score.sync("été 🦀 gros accord", 0..0);
        assert_eq!(score.sizes[4], 20.0);
        assert_eq!(score.sizes.len(), "été 🦀 gros accord".chars().count());
        assert!(!score.resize(7..10, 1.0, false)); // invalid UTF-8 boundary
        assert!(!score.resize(0..0, 1.0, false));
        assert!(!score.resize(9..999, 1.0, false));
    }

    #[test]
    fn mixed_sizes_limits_typing_inheritance_and_undo_redo() {
        let mut score = Partition::new("ab");
        let mut text = "ab".to_string();
        score.resize(0..1, 100.0, false);
        assert_eq!(score.selection_size(0..2), "Tailles mixtes");
        text.insert(0, 'X');
        score.sync(&text, 0..1);
        assert_eq!(score.sizes, [72., 72., 14.]);
        score.undo(&mut text, 0..1, false);
        assert_eq!(text, "ab");
        assert_eq!(score.sizes, [72., 14.]);
        score.undo(&mut text, 0..1, false);
        assert_eq!(score.sizes, [14., 14.]);
        score.undo(&mut text, 0..1, true);
        assert_eq!(score.sizes, [72., 14.]);
        score.resize(0..2, -100.0, false);
        assert_eq!(score.sizes, [8., 8.]);
        score.resize(0..2, 0.0, true);
        assert_eq!(score.sizes, [14., 14.]);
    }

    #[test]
    fn data_roundtrip_rejects_stale_text_and_invalid_runs() {
        let mut score = Partition::new("Café\nAm");
        score.resize(0..5, 4., false);
        let data = score.data();
        assert_eq!(data.runs.len(), 2);
        let json = serde_json::to_string(&data).unwrap();
        let restored =
            Partition::from_data(serde_json::from_str(&json).unwrap(), "Café\nAm").unwrap();
        assert_eq!(restored.sizes, score.sizes);
        assert!(Partition::from_data(data.clone(), "Other").is_err());
        let mut invalid = data;
        invalid.runs[0].size = 0.;
        assert!(Partition::from_data(invalid, "Café\nAm").is_err());
    }

    #[test]
    fn pagination_respects_margins_mixed_heights_and_preserves_cursor_offsets() {
        let ctx = egui::Context::default();
        crate::theme::install(&ctx);
        let _ = ctx.run(Default::default(), |ctx| {
            let text = "Café 🦀\n".repeat(100);
            let mut score = Partition::new(&text);
            score.resize(0..10, 30., false);
            let original = ctx.fonts(|f| f.layout_job(score.layout(&text)));
            let paged = paginate(original.clone(), &score);
            assert!(page_count(&paged) > 1);
            assert_eq!(paged.job.text, text);
            assert_eq!(paged.end().ccursor.index, text.chars().count());
            for (a, b) in original.rows.iter().zip(&paged.rows) {
                assert_eq!(a.glyphs.len(), b.glyphs.len());
                let offset = b.rect.top() % (PAGE_HEIGHT + PAGE_GAP);
                assert!(offset + b.height() <= CONTENT_HEIGHT + 0.01);
                let cursor = paged.cursor_from_pos(vec2(0., b.rect.center().y));
                assert_eq!(
                    paged.from_ccursor(cursor.ccursor).rcursor.row,
                    cursor.rcursor.row
                );
            }
            assert!((PAGE_WIDTH / PAGE_HEIGHT - 210. / 297.).abs() < 0.00001);
        });
    }
}
