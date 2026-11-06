use crate::{
    Action, Close, EditorWindow, State,
    theme::{self, Icon},
};
use eframe::egui::{self, Color32, FontId, Rect, Sense, Stroke, pos2, vec2};

pub fn toolbar(
    ctx: &egui::Context,
    state: &State,
    window: &EditorWindow,
    actions: &mut Vec<Action>,
) {
    egui::TopBottomPanel::top("toolbar")
        .show_separator_line(false)
        .exact_height(44.0)
        .frame(theme::panel_frame(theme::TOOLBAR).inner_margin(egui::Margin::symmetric(10, 5)))
        .show(ctx, |ui| {
            let buttons = [
                (
                    Icon::New,
                    "Nouveau",
                    "Nouveau · Cmd/Ctrl+N",
                    Some(Action::New),
                ),
                (
                    Icon::Folder,
                    "Ouvrir",
                    "Ouvrir · Cmd/Ctrl+O",
                    Some(Action::Open),
                ),
                (
                    Icon::Save,
                    "Enregistrer",
                    "Enregistrer · Cmd/Ctrl+S",
                    Some(Action::Save(false)),
                ),
                (
                    Icon::SaveAs,
                    "Enreg. sous…",
                    "Enregistrer sous… · Cmd/Ctrl+Maj+S",
                    Some(Action::Save(true)),
                ),
                (
                    Icon::Format,
                    "Formater",
                    "Formater · Cmd/Ctrl+Maj+F",
                    Some(Action::Format),
                ),
                (
                    Icon::NewWindow,
                    "Fenêtre",
                    "Nouvelle fenêtre · Cmd/Ctrl+Maj+N",
                    Some(Action::NewWindow),
                ),
                (
                    Icon::Detach,
                    "Détacher",
                    "Détacher l’onglet",
                    Some(Action::Detach),
                ),
                (
                    Icon::Close,
                    "Fermer",
                    "Fermer l’onglet · Cmd/Ctrl+W",
                    window
                        .tabs
                        .get(window.active)
                        .map(|&id| Action::Close(Close::Tab(id))),
                ),
                (
                    Icon::Search,
                    "Rechercher",
                    "Rechercher rapidement",
                    Some(Action::QuickFind),
                ),
                (
                    Icon::Settings,
                    "Avancée…",
                    "Recherche avancée… · Cmd/Ctrl+F",
                    Some(Action::Find),
                ),
                (
                    Icon::Quit,
                    "Quitter",
                    "Quitter MPTE · Cmd/Ctrl+Q",
                    Some(Action::Close(Close::Quit)),
                ),
            ];
            let primary = |icon| {
                matches!(
                    icon,
                    Icon::New | Icon::Folder | Icon::Save | Icon::Format | Icon::Search
                )
            };
            ui.spacing_mut().item_spacing.x = 4.0;
            let fits = |show_label: fn(Icon) -> bool| {
                let width = buttons
                    .iter()
                    .map(|(icon, label, _, _)| {
                        theme::toolbar_button_width(ui, if show_label(*icon) { label } else { "" })
                    })
                    .sum::<f32>()
                    + (buttons.len() - 1) as f32 * ui.spacing().item_spacing.x;
                width <= ui.available_width()
            };
            let show_all = fits(|_| true);
            let show_primary = show_all || fits(primary);

            ui.horizontal_centered(|ui| {
                for (icon, label, tooltip, action) in buttons {
                    let caption = if show_all || show_primary && primary(icon) {
                        label
                    } else {
                        ""
                    };
                    let response = ui
                        .add_enabled_ui(action.is_some(), |ui| {
                            theme::toolbar_button(
                                ui,
                                icon,
                                caption,
                                tooltip,
                                matches!(icon, Icon::Search),
                            )
                        })
                        .inner
                        .on_hover_text(tooltip);
                    if response.clicked() {
                        actions.extend(action);
                    }
                }
            });
        });
    egui::TopBottomPanel::top("tabs")
        .show_separator_line(false)
        .exact_height(46.0)
        .frame(
            egui::Frame::NONE
                .fill(theme::BACKGROUND)
                .stroke(Stroke::new(1.0, theme::BORDER)),
        )
        .show(ctx, |ui| {
            egui::ScrollArea::horizontal()
                .id_salt("tabs-scroll")
                .auto_shrink([false, true])
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    ui.horizontal(|ui| {
                        for (index, id) in window.tabs.iter().enumerate() {
                            let doc = &state.docs[id];
                            let active = index == window.active;
                            let color = if active { theme::BLUE } else { theme::TEXT };
                            let title =
                                format!("{}{}", doc.title(), if doc.dirty() { " •" } else { "" });
                            let galley = ui.fonts(|f| {
                                f.layout_no_wrap(title, FontId::proportional(16.0), color)
                            });
                            let width = (galley.size().x + 84.0).clamp(140.0, 340.0);
                            let (rect, response) =
                                ui.allocate_exact_size(vec2(width, 46.0), Sense::click());
                            ui.painter().rect_filled(
                                rect,
                                0.0,
                                if active {
                                    Color32::WHITE
                                } else if response.hovered() {
                                    theme::BLUE_LIGHT
                                } else {
                                    theme::BACKGROUND
                                },
                            );
                            if active {
                                ui.painter().rect_filled(
                                    Rect::from_min_max(
                                        pos2(rect.left(), rect.bottom() - 3.0),
                                        rect.right_bottom(),
                                    ),
                                    0.0,
                                    theme::BLUE,
                                );
                            }
                            let kind = if ["sh", "bash", "zsh", "rs", "java", "js", "ts", "py"]
                                .contains(&doc.extension().as_str())
                            {
                                Icon::Code
                            } else {
                                Icon::File
                            };
                            theme::icon(
                                ui,
                                Rect::from_center_size(
                                    pos2(rect.left() + 24.0, rect.center().y),
                                    vec2(22.0, 22.0),
                                ),
                                kind,
                                color,
                            );
                            ui.painter()
                                .with_clip_rect(rect.shrink2(vec2(40.0, 0.0)))
                                .galley(
                                    pos2(
                                        rect.left() + 44.0,
                                        rect.center().y - galley.size().y / 2.0,
                                    ),
                                    galley,
                                    color,
                                );
                            if response.clicked() {
                                actions.push(Action::Select(index));
                            }
                            if response.clicked_by(egui::PointerButton::Middle) {
                                actions.push(Action::Close(Close::Tab(*id)));
                            }
                            response.clone().on_hover_text(
                                doc.path
                                    .as_ref()
                                    .map(|p| p.display().to_string())
                                    .unwrap_or_else(|| "Document non enregistré".into()),
                            );
                            let close_rect = Rect::from_center_size(
                                pos2(rect.right() - 17.0, rect.center().y),
                                vec2(24.0, 24.0),
                            );
                            let close = ui.interact(
                                close_rect,
                                egui::Id::new(("tab-close", id)),
                                Sense::click(),
                            );
                            if active || response.hovered() || close.hovered() {
                                theme::icon(ui, close_rect.shrink(3.0), Icon::Close, theme::MUTED);
                            }
                            if close
                                .on_hover_text("Fermer l’onglet · Cmd/Ctrl+W")
                                .clicked()
                            {
                                actions.push(Action::Close(Close::Tab(*id)));
                            }
                            response.context_menu(|ui| {
                                if ui.button("Détacher dans une fenêtre").clicked() {
                                    actions.push(Action::Select(index));
                                    actions.push(Action::Detach);
                                    ui.close_menu();
                                }
                            });
                        }
                    });
                });
        });
}

pub fn status(ctx: &egui::Context, state: &mut State, window: &EditorWindow) {
    let Some(&id) = window.tabs.get(window.active) else {
        return;
    };
    let busy = state.busy.contains(&id);
    let doc = state.docs.get_mut(&id).unwrap();
    let caret = egui::TextEdit::load_state(ctx, egui::Id::new(("text", id)))
        .and_then(|s| s.cursor.char_range())
        .map_or(0, |r| r.primary.index);
    let byte = doc
        .text
        .char_indices()
        .nth(caret)
        .map_or(doc.text.len(), |(i, _)| i);
    let line = doc.text[..byte].bytes().filter(|b| *b == b'\n').count() + 1;
    let start = doc.text[..byte].rfind('\n').map_or(0, |i| i + 1);
    let column = crate::column::visual_width(&doc.text[start..byte]) + 1;
    let lines = doc.text.split('\n').count();
    let language = match doc.extension().as_str() {
        "json" => "JSON",
        "xml" | "plist" => "XML",
        "html" | "htm" => "HTML",
        "java" => "Java",
        "sh" | "bash" | "zsh" => "Shell",
        "rs" => "Rust",
        "py" => "Python",
        "js" => "JavaScript",
        "ts" => "TypeScript",
        _ => "Texte",
    };
    egui::TopBottomPanel::bottom("status")
        .show_separator_line(false)
        .exact_height(40.0)
        .frame(theme::panel_frame(theme::BACKGROUND).inner_margin(egui::Margin::symmetric(16, 4)))
        .show(ctx, |ui| {
            ui.style_mut().override_font_id = Some(FontId::proportional(13.0));
            let narrow = ui.available_width() < 760.0;
            ui.horizontal_centered(|ui| {
                ui.label(
                    egui::RichText::new(if narrow {
                        format!("L{line}:C{column} · {lines} l.")
                    } else {
                        format!("Ligne {line}, col. {column}   ·   {lines} lignes")
                    })
                    .color(theme::MUTED),
                );
                if busy {
                    ui.spinner();
                }
                if let Some(rectangle) = &doc.column.rectangle {
                    ui.label(
                        egui::RichText::new(format!(
                            "Colonne {} × {}",
                            rectangle.rows.len(),
                            rectangle.columns.len()
                        ))
                        .color(theme::BLUE),
                    );
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(language);
                    ui.label("UTF-8");
                    ui.label("·");
                    ui.checkbox(
                        &mut doc.format_on_save,
                        if narrow {
                            "Formater à l’enreg."
                        } else {
                            "Formater à l’enregistrement"
                        },
                    )
                    .on_hover_text("Formater ce document avant de l’enregistrer");
                });
            });
        });
}
