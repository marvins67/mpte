use crate::{Id, document::Document};
use eframe::egui::{self, Color32, FontId, Rect, Shape, Stroke, pos2, vec2};
use std::{ops::Range, sync::Arc};

fn paint_pair(job: &mut egui::text::LayoutJob, pair: Option<(usize, usize)>) {
    let Some((a, b)) = pair else {
        return;
    };
    let mut sections = Vec::new();
    for section in &job.sections {
        let mut start = section.byte_range.start;
        for position in [a, b] {
            if section.byte_range.contains(&position) {
                if start < position {
                    let mut s = section.clone();
                    s.byte_range = start..position;
                    sections.push(s);
                }
                let mut s = section.clone();
                s.byte_range = position..position + 1;
                s.format.color = Color32::from_rgb(220, 25, 35);
                s.format.background = Color32::from_rgb(255, 228, 228);
                sections.push(s);
                start = position + 1;
            }
        }
        if start < section.byte_range.end {
            let mut s = section.clone();
            s.byte_range = start..section.byte_range.end;
            sections.push(s);
        }
    }
    job.sections = sections;
}

/// Keep every source character and row for TextEdit's cursor, clipboard and undo.
/// Only geometry and meshes change; folding never substitutes a shortened buffer.
fn folded_galley(galley: Arc<egui::Galley>, visible: &[bool]) -> Arc<egui::Galley> {
    if visible.iter().all(|v| *v) {
        return galley;
    }
    let mut result = (*galley).clone();
    let mut removed = 0.0;
    let mut width: f32 = 0.0;
    result.mesh_bounds = Rect::NOTHING;
    result.num_indices = 0;
    result.num_vertices = 0;
    for (index, row) in result.rows.iter_mut().enumerate() {
        let height = row.height();
        let shift = vec2(0.0, -removed);
        row.rect = row.rect.translate(shift);
        for glyph in &mut row.glyphs {
            glyph.pos += shift;
        }
        if !visible.get(index).copied().unwrap_or(true) {
            row.rect.max.y = row.rect.min.y;
            row.visuals = Default::default();
            removed += height;
        } else {
            row.visuals.mesh.translate(shift);
            row.visuals.mesh_bounds = row.visuals.mesh_bounds.translate(shift);
            width = width.max(row.rect.right());
            result.mesh_bounds = result.mesh_bounds.union(row.visuals.mesh_bounds);
            result.num_indices += row.visuals.mesh.indices.len();
            result.num_vertices += row.visuals.mesh.vertices.len();
        }
    }
    result.rect.max.y -= removed;
    result.rect.max.x = width;
    Arc::new(result)
}
fn char_to_byte(text: &str, index: usize) -> usize {
    text.char_indices()
        .nth(index)
        .map_or(text.len(), |(i, _)| i)
}

pub fn show(
    ctx: &egui::Context,
    ui: &mut egui::Ui,
    id: Id,
    doc: &mut Document,
    mut job: egui::text::LayoutJob,
    selection: &mut Range<usize>,
    jump: &mut Option<(Id, Range<usize>)>,
) {
    let extension = doc.extension();
    doc.view.update(&doc.text, &extension, doc.revision);
    doc.column.validate(doc.revision);
    let editor_id = egui::Id::new(("text", id));
    let initial_cursor = egui::TextEdit::load_state(ctx, editor_id)
        .and_then(|s| s.cursor.char_range())
        .map_or(0, |r| r.primary.index);
    let caret = char_to_byte(&doc.text, initial_cursor);
    if let Some((did, range)) = jump.as_ref() {
        if *did == id {
            doc.column.clear();
            doc.view.reveal(range.clone());
        }
    }
    let editing = ctx.input(|i| {
        i.events.iter().any(|e| {
            matches!(
                e,
                egui::Event::Text(_)
                    | egui::Event::Paste(_)
                    | egui::Event::Cut
                    | egui::Event::Key {
                        pressed: true,
                        key: egui::Key::Backspace
                            | egui::Key::Delete
                            | egui::Key::Enter
                            | egui::Key::Tab
                            | egui::Key::Z,
                        ..
                    }
            )
        })
    });
    if editing && ctx.memory(|m| m.has_focus(editor_id)) {
        doc.view.collapsed.clear();
    }
    paint_pair(&mut job, doc.view.outline.matching(&doc.text, caret));
    let viewport_height = ui.available_height();
    egui::ScrollArea::both()
        .id_salt(("editor", id))
        .auto_shrink([false, false])
        .drag_to_scroll(false)
        .show(ui, |ui| {
            let viewport = ui.clip_rect();
            let background = ui.painter().add(Shape::Noop);
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.horizontal_top(|ui| {
                let font = FontId::monospace(crate::theme::EDITOR_FONT_SIZE);
                let line_height = ui.fonts(|f| f.row_height(&font));
                let space = ui.fonts(|f| f.glyph_width(&font, ' '));
                let mut visible = doc.view.outline.visibility(&doc.view.collapsed);
                let count = visible.iter().filter(|v| **v).count();
                let height = (count as f32 * line_height + 4.0).max(viewport_height);
                let gutter_width = (doc.view.outline.lines.len().to_string().len() as f32 * space
                    + 38.0)
                    .max(56.0);
                let (gutter, _) =
                    ui.allocate_exact_size(vec2(gutter_width, height), egui::Sense::hover());
                ui.painter()
                    .rect_filled(gutter, 0.0, crate::theme::BACKGROUND);
                let mut display_row = 0;
                let mut toggled = false;
                for (line, is_visible) in visible.iter().enumerate() {
                    if !is_visible {
                        continue;
                    }
                    let y = gutter.top() + 2.0 + display_row as f32 * line_height;
                    display_row += 1;
                    if y + line_height < viewport.top() || y > viewport.bottom() {
                        continue;
                    }
                    ui.painter().text(
                        pos2(gutter.right() - 26.0, y),
                        egui::Align2::RIGHT_TOP,
                        (line + 1).to_string(),
                        font.clone(),
                        if line == doc.view.outline.line_at(caret) {
                            crate::theme::BLUE
                        } else {
                            crate::theme::MUTED
                        },
                    );
                    let x = gutter.right() - 12.0;
                    if doc
                        .view
                        .outline
                        .blocks
                        .iter()
                        .any(|b| b.start < line && line <= b.end)
                    {
                        ui.painter().line_segment(
                            [pos2(x, y), pos2(x, y + line_height)],
                            Stroke::new(1.0, Color32::from_gray(190)),
                        );
                    }
                    // A single control folds all multiline blocks beginning on this line.
                    let blocks: Vec<_> = doc
                        .view
                        .outline
                        .blocks
                        .iter()
                        .filter(|b| b.start == line)
                        .cloned()
                        .collect();
                    if !blocks.is_empty() {
                        let closed = blocks
                            .iter()
                            .any(|b| doc.view.collapsed.contains(&b.anchor));
                        let rect = Rect::from_center_size(
                            pos2(x, y + line_height / 2.0),
                            vec2(11.0, 11.0),
                        );
                        let response = ui
                            .interact(
                                rect.expand(3.0),
                                egui::Id::new(("fold", id, line)),
                                egui::Sense::click(),
                            )
                            .on_hover_text(if closed {
                                "Déplier le bloc"
                            } else {
                                "Replier le bloc"
                            });
                        ui.painter().rect_filled(rect, 0.0, Color32::WHITE);
                        ui.painter().rect_stroke(
                            rect,
                            0.0,
                            Stroke::new(1.0, Color32::from_gray(145)),
                            egui::StrokeKind::Inside,
                        );
                        ui.painter().line_segment(
                            [
                                pos2(x - 3.0, rect.center().y),
                                pos2(x + 3.0, rect.center().y),
                            ],
                            Stroke::new(1.0, Color32::from_gray(70)),
                        );
                        if closed {
                            ui.painter().line_segment(
                                [
                                    pos2(x, rect.center().y - 3.0),
                                    pos2(x, rect.center().y + 3.0),
                                ],
                                Stroke::new(1.0, Color32::from_gray(70)),
                            );
                        }
                        if response.clicked() {
                            doc.column.clear();
                            for block in &blocks {
                                if closed {
                                    doc.view.collapsed.remove(&block.anchor);
                                } else {
                                    doc.view.collapsed.insert(block.anchor);
                                }
                            }
                            if !closed
                                && blocks.iter().any(|b| {
                                    b.start < doc.view.outline.line_at(caret)
                                        && doc.view.outline.line_at(caret) < b.end
                                })
                            {
                                let end = doc
                                    .view
                                    .outline
                                    .lines
                                    .get(line + 1)
                                    .copied()
                                    .unwrap_or(doc.text.len())
                                    .saturating_sub(1);
                                *jump = Some((id, end..end));
                            }
                            toggled = true;
                            ctx.request_repaint();
                        }
                    }
                }
                if toggled {
                    visible = doc.view.outline.visibility(&doc.view.collapsed);
                }
                let requested_jump = jump.take().filter(|(did, r)| {
                    *did == id
                        && r.end <= doc.text.len()
                        && doc.text.is_char_boundary(r.start)
                        && doc.text.is_char_boundary(r.end)
                });
                if let Some((_, range)) = &requested_jump {
                    let start = doc.text[..range.start].chars().count();
                    let end = doc.text[..range.end].chars().count();
                    let mut edit = egui::TextEdit::load_state(ctx, editor_id).unwrap_or_default();
                    edit.cursor
                        .set_char_range(Some(egui::text::CCursorRange::two(
                            egui::text::CCursor::new(start),
                            egui::text::CCursor::new(end),
                        )));
                    edit.store(ctx, editor_id);
                }
                let column_layer = ui.painter().add(Shape::Noop);
                let geometry = folded_galley(ui.fonts(|f| f.layout_job(job.clone())), &visible);
                crate::column::input(
                    ctx,
                    id,
                    doc,
                    &crate::column::Geometry {
                        galley: &geometry,
                        origin: ui.next_widget_position() + vec2(4.0, 2.0),
                        clip: viewport,
                        space,
                        visible: &visible,
                        layer: ui.layer_id(),
                    },
                );
                let mut layouter = |ui: &egui::Ui, text: &str, _: f32| {
                    if text == job.text {
                        folded_galley(ui.fonts(|f| f.layout_job(job.clone())), &visible)
                    } else {
                        ui.fonts(|f| {
                            f.layout_job(egui::text::LayoutJob::simple(
                                text.to_owned(),
                                font.clone(),
                                ui.visuals().text_color(),
                                f32::INFINITY,
                            ))
                        })
                    }
                };
                let column_focused =
                    doc.column.rectangle.is_some() && ctx.memory(|m| m.has_focus(editor_id));
                let output = egui::TextEdit::multiline(&mut doc.text)
                    .id(editor_id)
                    .interactive(doc.column.rectangle.is_none())
                    .font(font.clone())
                    .code_editor()
                    .frame(false)
                    .margin(egui::Margin::symmetric(4, 2))
                    .desired_width(f32::INFINITY)
                    .min_size(vec2(0.0, viewport_height))
                    .desired_rows(1)
                    .layouter(&mut layouter)
                    .show(ui);
                if doc.column.rectangle.is_some() {
                    // TextEdit is read-only during rectangular selection; register
                    // its ID as keyboard-focusable so egui keeps focus across frames.
                    ui.interact(
                        output.response.rect,
                        editor_id,
                        egui::Sense::focusable_noninteractive(),
                    );
                    if column_focused {
                        ctx.memory_mut(|m| {
                            m.request_focus(editor_id);
                            m.set_focus_lock_filter(
                                editor_id,
                                egui::EventFilter {
                                    tab: true,
                                    horizontal_arrows: true,
                                    vertical_arrows: true,
                                    escape: true,
                                },
                            );
                        });
                    }
                    if column_focused {
                        if let Some(range) = output.state.cursor.char_range() {
                            let cursor_rect = output
                                .galley
                                .pos_from_ccursor(range.primary)
                                .translate(output.galley_pos.to_vec2());
                            let transform = ctx
                                .layer_transform_to_global(ui.layer_id())
                                .unwrap_or_default();
                            ctx.output_mut(|o| {
                                o.ime = Some(egui::output::IMEOutput {
                                    rect: transform * output.response.rect,
                                    cursor_rect: transform * cursor_rect,
                                })
                            });
                        }
                    }
                    if output.response.hovered() {
                        ctx.set_cursor_icon(egui::CursorIcon::Text);
                    }
                }
                crate::column::paint(ctx, ui, column_layer, doc, &output, space);
                if let Some(range) = output.state.cursor.char_range() {
                    let a = char_to_byte(&doc.text, range.primary.index);
                    let b = char_to_byte(&doc.text, range.secondary.index);
                    *selection = a.min(b)..a.max(b);
                    if range.primary.index != initial_cursor {
                        ctx.request_repaint();
                    }
                    let row = output.galley.from_ccursor(range.primary).rcursor.row;
                    if let Some(row) = output.galley.rows.get(row) {
                        let r = Rect::from_min_max(
                            pos2(viewport.left(), output.galley_pos.y + row.rect.top()),
                            pos2(viewport.right(), output.galley_pos.y + row.rect.bottom()),
                        );
                        ui.painter().set(
                            background,
                            Shape::rect_filled(r, 0.0, crate::theme::CURRENT_LINE),
                        );
                    }
                    let line = doc.view.outline.line_at(a);
                    if !toggled && !visible.get(line).copied().unwrap_or(true) {
                        doc.view.reveal(a..a);
                        ctx.request_repaint();
                    }
                }
                // Dotted indentation guides follow visible rows without changing source positions.
                for block in &doc.view.outline.blocks {
                    if doc.view.collapsed.contains(&block.anchor) {
                        continue;
                    }
                    let start = doc.view.outline.lines[block.start];
                    let indent: usize = doc
                        .text
                        .get(start..)
                        .unwrap_or("")
                        .chars()
                        .take_while(|c| *c == ' ' || *c == '\t')
                        .map(|c| if c == '\t' { 4 } else { 1 })
                        .sum();
                    let x = output.galley_pos.x + indent as f32 * space;
                    let rows = &output.galley.rows;
                    if let (Some(a), Some(b)) = (rows.get(block.start + 1), rows.get(block.end)) {
                        let mut y = (output.galley_pos.y + a.rect.top()).max(viewport.top());
                        let end = (output.galley_pos.y + b.rect.bottom()).min(viewport.bottom());
                        while y < end {
                            ui.painter().line_segment(
                                [pos2(x, y), pos2(x, (y + 1.0).min(end))],
                                Stroke::new(1.0, Color32::from_gray(195)),
                            );
                            y += 3.0;
                        }
                    }
                }
                if let Some((_, range)) = requested_jump {
                    let c = egui::text::CCursor::new(
                        doc.text.get(..range.start).unwrap_or("").chars().count(),
                    );
                    ui.scroll_to_rect(
                        output
                            .galley
                            .pos_from_ccursor(c)
                            .translate(output.galley_pos.to_vec2()),
                        Some(egui::Align::Center),
                    );
                }
                if output.response.changed() {
                    doc.revision += 1;
                    doc.view.collapsed.clear();
                }
            });
        });
}

pub(crate) fn record_undo(ctx: &egui::Context, id: Id, text: &str) {
    let editor_id = egui::Id::new(("text", id));
    let mut state = egui::TextEdit::load_state(ctx, editor_id).unwrap_or_default();
    let cursor = state.cursor.char_range().unwrap_or_default();
    let mut undoer = state.undoer();
    undoer.add_undo(&(cursor, text.to_owned()));
    state.set_undoer(undoer);
    state.store(ctx, editor_id);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn folding_preserves_full_text_cursor_offsets_and_shortens_geometry() {
        let ctx = egui::Context::default();
        let _ = ctx.run(Default::default(), |ctx| {
            let text = "{\n  \"été\": 1,\n  \"two\": 2\n}";
            let galley = ctx.fonts(|f| {
                f.layout_no_wrap(
                    text.into(),
                    FontId::monospace(crate::theme::EDITOR_FONT_SIZE),
                    Color32::BLACK,
                )
            });
            let original = galley.clone();
            let folded = folded_galley(galley, &[true, false, false, true]);
            assert_eq!(folded.job.text, text);
            assert_eq!(folded.end().ccursor.index, text.chars().count());
            assert_eq!(folded.rows[1].glyphs.len(), original.rows[1].glyphs.len());
            assert!(folded.rows[1].visuals.mesh.is_empty());
            assert!(folded.size().y < original.size().y);
            let cursor = folded.cursor_from_pos(vec2(0.0, folded.rows[3].rect.center().y));
            assert_eq!(cursor.rcursor.row, 3);
            assert_eq!(cursor.ccursor.index, text.find('}').unwrap() - 2);
        });
    }
    fn column_frame(
        ctx: &egui::Context,
        doc: &mut Document,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1100.0, 760.0))),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(ctx, |ui| {
                        let job = egui::text::LayoutJob::simple(
                            doc.text.clone(),
                            FontId::monospace(crate::theme::EDITOR_FONT_SIZE),
                            Color32::BLACK,
                            f32::INFINITY,
                        );
                        show(ctx, ui, 1, doc, job, &mut (0..0), &mut None);
                    });
            },
        )
    }
    fn column_position(ctx: &egui::Context, column: usize, row: usize) -> egui::Pos2 {
        let font = FontId::monospace(crate::theme::EDITOR_FONT_SIZE);
        let (space, height) = ctx.fonts(|f| (f.glyph_width(&font, ' '), f.row_height(&font)));
        pos2(
            60.0 + column as f32 * space,
            2.0 + (row as f32 + 0.5) * height,
        )
    }
    fn select_column(ctx: &egui::Context, doc: &mut Document) {
        column_frame(
            ctx,
            doc,
            vec![egui::Event::PointerButton {
                pos: column_position(ctx, 2, 0),
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::ALT,
            }],
        );
        column_frame(
            ctx,
            doc,
            vec![egui::Event::PointerMoved(column_position(ctx, 5, 2))],
        );
        column_frame(
            ctx,
            doc,
            vec![egui::Event::PointerButton {
                pos: column_position(ctx, 5, 2),
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::ALT,
            }],
        );
    }
    #[test]
    fn alt_drag_copy_replace_and_undo_work_across_frames() {
        let ctx = egui::Context::default();
        let mut doc = Document::new();
        doc.text = "ab123cd\nef456gh\nij789kl\n".into();
        let original = doc.text.clone();
        column_frame(&ctx, &mut doc, vec![]);
        select_column(&ctx, &mut doc);
        assert_eq!(doc.column.rectangle.as_ref().unwrap().columns, 2..5);
        assert!(ctx.memory(|m| m.has_focus(egui::Id::new(("text", 1_u64)))));
        let output = column_frame(&ctx, &mut doc, vec![egui::Event::Copy]);
        assert!(output.platform_output.ime.is_some());
        assert!(
            output
                .platform_output
                .commands
                .iter()
                .any(|c| matches!(c, egui::OutputCommand::CopyText(s) if s == "123\n456\n789"))
        );
        column_frame(&ctx, &mut doc, vec![egui::Event::Text("X".into())]);
        assert_eq!(doc.text, "abXcd\nefXgh\nijXkl\n");
        column_frame(
            &ctx,
            &mut doc,
            vec![egui::Event::Key {
                key: egui::Key::Z,
                physical_key: Some(egui::Key::Z),
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::COMMAND,
            }],
        );
        assert_eq!(doc.text, original);
        assert!(doc.column.rectangle.is_none());
    }
    #[test]
    fn rectangular_paste_and_escape_return_to_normal_editing() {
        let ctx = egui::Context::default();
        let mut doc = Document::new();
        doc.text = "ab123cd\nef456gh\nij789kl".into();
        column_frame(&ctx, &mut doc, vec![]);
        select_column(&ctx, &mut doc);
        column_frame(
            &ctx,
            &mut doc,
            vec![egui::Event::Paste("A\r\nB\r\nC".into())],
        );
        assert_eq!(doc.text, "abAcd\nefBgh\nijCkl");
        column_frame(
            &ctx,
            &mut doc,
            vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        assert!(doc.column.rectangle.is_none());
        column_frame(&ctx, &mut doc, vec![egui::Event::Text("!".into())]);
        assert_eq!(doc.text, "abAcd\nefBgh\nijC!kl");
    }
    #[test]
    fn column_typing_accepts_letter_keys_and_ime_commits() {
        let ctx = egui::Context::default();
        let mut doc = Document::new();
        doc.text = "ab123cd\nef456gh\nij789kl".into();
        column_frame(&ctx, &mut doc, vec![]);
        select_column(&ctx, &mut doc);
        column_frame(
            &ctx,
            &mut doc,
            vec![
                egui::Event::Key {
                    key: egui::Key::A,
                    physical_key: Some(egui::Key::A),
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                },
                egui::Event::Text("a".into()),
            ],
        );
        assert_eq!(doc.text, "abacd\nefagh\nijakl");
        column_frame(
            &ctx,
            &mut doc,
            vec![egui::Event::Ime(egui::ImeEvent::Commit("é".into()))],
        );
        assert_eq!(doc.text, "abaécd\nefaégh\nijaékl");
    }
    #[test]
    fn rectangular_cut_copies_only_block_and_preserves_line_structure() {
        let ctx = egui::Context::default();
        let mut doc = Document::new();
        doc.text = "ab123cd\nef456gh\nij789kl\n".into();
        column_frame(&ctx, &mut doc, vec![]);
        select_column(&ctx, &mut doc);
        let output = column_frame(&ctx, &mut doc, vec![egui::Event::Cut]);
        assert_eq!(doc.text, "abcd\nefgh\nijkl\n");
        assert!(
            output
                .platform_output
                .commands
                .iter()
                .any(|c| matches!(c, egui::OutputCommand::CopyText(s) if s == "123\n456\n789"))
        );
    }
}
