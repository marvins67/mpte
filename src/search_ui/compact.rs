//! Compact presentation of the existing search commands. No search or replacement
//! implementation lives here; each control emits the parent's Command enum.
use super::{Command, Mode, Panel, Tab};
use crate::{Id, theme};
use eframe::egui::{self, Color32, FontId, Response, Stroke, Ui, vec2};

const PAPER: Color32 = Color32::WHITE;
const CHROME: Color32 = Color32::from_rgb(241, 243, 246);
const SOFT: Color32 = Color32::from_rgb(247, 248, 250);
const LINE: Color32 = Color32::from_rgb(217, 223, 232);
const INPUT_LINE: Color32 = Color32::from_rgb(169, 183, 203);
const INK: Color32 = Color32::from_rgb(32, 41, 56);
const SECONDARY: Color32 = Color32::from_rgb(82, 97, 118);
const BLUE: Color32 = Color32::from_rgb(34, 86, 161);
const ACTIVE: Color32 = Color32::from_rgb(231, 239, 253);
const PRIMARY: Color32 = Color32::from_rgb(37, 95, 185);
const ERROR: Color32 = Color32::from_rgb(159, 38, 51);

fn button(ui: &mut Ui, label: &str, primary: bool) -> Response {
    enabled_button(ui, label, primary, true)
}
fn enabled_button(ui: &mut Ui, label: &str, primary: bool, enabled: bool) -> Response {
    let enter = label.strip_suffix(" ↵");
    let text = enter.map_or_else(|| label.to_owned(), |s| format!("{s}    "));
    let color = if primary { PAPER } else { INK };
    let response = ui.add_enabled(
        enabled,
        egui::Button::new(egui::RichText::new(text).size(13.0).color(color))
            .fill(if primary { PRIMARY } else { PAPER })
            .stroke(Stroke::new(1.0, if primary { PRIMARY } else { LINE }))
            .corner_radius(6)
            .min_size(vec2(0.0, 29.0))
            .wrap_mode(egui::TextWrapMode::Extend),
    );
    if enter.is_some() {
        let x = response.rect.right() - 9.0;
        let y = response.rect.center().y;
        ui.painter().add(egui::Shape::line(
            vec![
                egui::pos2(x, y - 4.0),
                egui::pos2(x, y + 2.0),
                egui::pos2(x - 8.0, y + 2.0),
            ],
            Stroke::new(1.2, color),
        ));
        ui.painter().add(egui::Shape::line(
            vec![
                egui::pos2(x - 5.0, y - 1.0),
                egui::pos2(x - 8.0, y + 2.0),
                egui::pos2(x - 5.0, y + 5.0),
            ],
            Stroke::new(1.2, color),
        ));
        response.clone().on_hover_text("Entrée");
    }
    response
}

fn label(ui: &mut Ui, text: &str) {
    ui.label(egui::RichText::new(text).size(12.0).color(SECONDARY));
}
fn field(ui: &mut Ui, value: &mut String, width: f32, search: bool) -> Response {
    let field = egui::Frame::NONE
        .fill(PAPER)
        .stroke(Stroke::new(1.0, INPUT_LINE))
        .corner_radius(6)
        .inner_margin(egui::Margin::symmetric(9, 1))
        .show(ui, |ui| {
            ui.set_width((width - 20.0).max(80.0));
            ui.spacing_mut().item_spacing.x = 7.0;
            ui.horizontal(|ui| {
                if search {
                    let (rect, _) = ui.allocate_exact_size(vec2(15.0, 15.0), egui::Sense::hover());
                    theme::icon(ui, rect, theme::Icon::Search, SECONDARY);
                }
                ui.add_sized(
                    [
                        (width - 20.0 - if search { 22.0 } else { 0.0 }).max(50.0),
                        29.0,
                    ],
                    egui::TextEdit::singleline(value)
                        .font(FontId::proportional(13.0))
                        .frame(false),
                )
            })
            .inner
        });
    if field.inner.has_focus() {
        ui.painter().rect_stroke(
            field.response.rect,
            6.0,
            Stroke::new(1.2, BLUE),
            egui::StrokeKind::Inside,
        );
    }
    field.inner
}
fn history_field(
    ui: &mut Ui,
    title: &str,
    value: &mut String,
    history: &[String],
    key: (Id, &str),
    search: bool,
) -> Response {
    label(ui, title);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 5.0;
        let width = ui.available_width() - 37.0;
        let response = field(ui, value, width, search);
        ui.scope(|ui| {
            ui.spacing_mut().interact_size = vec2(32.0, 33.0);
            ui.spacing_mut().button_padding.x = 5.0;
            ui.spacing_mut().icon_spacing = 4.0;
            egui::ComboBox::from_id_salt(key)
                .width(32.0)
                .selected_text("")
                .show_ui(ui, |ui| {
                    if history.is_empty() {
                        ui.add_enabled(false, egui::Label::new("Historique vide"));
                    }
                    for item in history {
                        if ui.selectable_label(false, item).clicked() {
                            value.clone_from(item);
                        }
                    }
                })
                .response
                .on_hover_text("Historique");
        });
        response
    })
    .inner
}
fn inputs(ui: &mut Ui, panel: &mut Panel, wid: Id) {
    let response = history_field(
        ui,
        "Texte à rechercher",
        &mut panel.query.needle,
        &panel.history,
        (wid, "query"),
        true,
    );
    if panel.focus {
        response.request_focus();
        panel.focus = false;
    }
    if panel.tab != Tab::Find {
        ui.add_space(4.0);
        history_field(
            ui,
            "Remplacer par",
            &mut panel.query.replacement,
            &panel.replacements,
            (wid, "replacement"),
            false,
        );
    }
    if panel.tab == Tab::Files {
        ui.add_space(4.0);
        if ui.available_width() >= 430.0 {
            let total = ui.available_width();
            ui.horizontal_top(|ui| {
                ui.allocate_ui_with_layout(
                    vec2(total - 119.0, 0.0),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| folder(ui, &mut panel.folder),
                );
                ui.allocate_ui_with_layout(
                    vec2(105.0, 0.0),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        label(ui, "Filtre de fichiers");
                        let width = ui.available_width();
                        field(ui, &mut panel.filters, width, false)
                            .on_hover_text("*.json;*.xml · * pour tous les fichiers");
                    },
                );
            });
        } else {
            folder(ui, &mut panel.folder);
            label(ui, "Filtre de fichiers");
            let width = ui.available_width();
            field(ui, &mut panel.filters, width, false);
        }
    }
}
fn folder(ui: &mut Ui, value: &mut String) {
    label(ui, "Dossier");
    ui.horizontal(|ui| {
        let width = (ui.available_width() - 38.0).max(80.0);
        field(ui, value, width, false);
        if button(ui, "…", false)
            .on_hover_text("Choisir un dossier")
            .clicked()
        {
            if let Some(path) = rfd::FileDialog::new().pick_folder() {
                *value = path.display().to_string();
            }
        }
    });
}
fn options(ui: &mut Ui, panel: &mut Panel) {
    ui.label(egui::RichText::new("Options").size(12.0).strong());
    ui.spacing_mut().item_spacing.y = 2.0;
    ui.checkbox(&mut panel.query.word, "Mot entier uniquement");
    ui.checkbox(&mut panel.query.case, "Respecter la casse");
    ui.add_enabled(
        panel.tab != Tab::Files,
        egui::Checkbox::new(&mut panel.backward, "Rechercher vers l’arrière"),
    );
    ui.add_enabled(
        panel.tab != Tab::Files,
        egui::Checkbox::new(&mut panel.wrap, "Boucler"),
    );
    ui.add_enabled(
        panel.tab != Tab::Files && panel.scope.is_some(),
        egui::Checkbox::new(&mut panel.in_selection, "Dans la sélection"),
    );
    if panel.tab == Tab::Files {
        ui.add_space(6.0);
        ui.checkbox(&mut panel.recursive, "Sous-dossiers");
        ui.checkbox(&mut panel.hidden, "Dossiers et fichiers cachés");
    }
}
fn modes(ui: &mut Ui, panel: &mut Panel) {
    ui.label(egui::RichText::new("Mode de recherche").size(12.0).strong());
    ui.spacing_mut().item_spacing.y = 2.0;
    ui.radio_value(&mut panel.query.mode, Mode::Normal, "Normal");
    ui.horizontal_wrapped(|ui| {
        ui.radio_value(&mut panel.query.mode, Mode::Extended, "Étendu");
        ui.label(
            egui::RichText::new(r"\n, \r, \t…")
                .font(FontId::monospace(11.0))
                .color(SECONDARY),
        );
    });
    ui.radio_value(&mut panel.query.mode, Mode::Regex, "Expression régulière");
    ui.add_enabled(
        panel.query.mode == Mode::Regex,
        egui::Checkbox::new(
            &mut panel.query.dot_newline,
            "Le point comprend les fins de ligne",
        ),
    );
    ui.label(egui::RichText::new("Groupes de remplacement : $1, ${nom}").size(11.0).color(SECONDARY))
        .on_hover_text("Syntaxe regex Rust : les assertions avant/arrière et références arrière dans le motif ne sont pas prises en charge.");
}
fn settings(ui: &mut Ui, panel: &mut Panel) {
    ui.separator();
    if ui.available_width() >= 480.0 {
        let top = ui.cursor().min.y;
        let mut divider = 0.0;
        ui.columns(2, |columns| {
            divider = columns[1].max_rect().left() - 7.0;
            options(&mut columns[0], panel);
            modes(&mut columns[1], panel);
        });
        ui.painter().line_segment(
            [
                egui::pos2(divider, top),
                egui::pos2(divider, ui.cursor().min.y - 6.0),
            ],
            Stroke::new(1.0, LINE),
        );
    } else {
        options(ui, panel);
        ui.separator();
        modes(ui, panel);
    }
}
fn scope(ui: &mut Ui, panel: &Panel, command: &mut Option<Command>) {
    if panel.tab == Tab::Files {
        return;
    }
    ui.separator();
    ui.horizontal_wrapped(|ui| {
        label(
            ui,
            if panel.tab == Tab::Find {
                "Lister les résultats"
            } else {
                "Remplacer dans"
            },
        );
        if button(ui, "Document actuel", false).clicked() {
            *command = Some(if panel.tab == Tab::Find {
                Command::Current
            } else {
                Command::ReplaceAll
            });
        }
        if button(ui, "Documents ouverts", false).clicked() {
            *command = Some(Command::AllOpen(panel.tab == Tab::Replace));
        }
    });
}
fn results(
    ui: &mut Ui,
    panel: &mut Panel,
    busy: bool,
    command: &mut Option<Command>,
    clicked: &mut Option<usize>,
) {
    if let Some((count, files)) = panel.report.as_ref().map(|r| (r.count, r.plan.len())) {
        ui.separator();
        ui.label(format!(
            "Confirmer {count} remplacement(s) dans {files} fichier(s) sur disque ?"
        ));
        label(
            ui,
            "Les fichiers ouverts seront ignorés. Les autres seront enregistrés immédiatement.",
        );
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled_ui(!busy, |ui| button(ui, "Confirmer les remplacements", true))
                .inner
                .clicked()
            {
                *command = Some(Command::Confirm);
            }
            if button(ui, "Abandonner", false).clicked() {
                panel.report = None;
            }
        });
    }
    if !panel.errors.is_empty() {
        ui.collapsing(format!("{} problème(s)", panel.errors.len()), |ui| {
            for error in &panel.errors {
                ui.label(error);
            }
        });
    }
    if !panel.results.is_empty() {
        ui.separator();
        ui.label(egui::RichText::new("Résultats").size(12.0).strong());
        egui::ScrollArea::vertical()
            .id_salt("search-results")
            .max_height(180.0)
            .show(ui, |ui| {
                for (index, hit) in panel.results.iter().enumerate() {
                    if ui
                        .selectable_label(
                            false,
                            format!("{}:{}  {}", hit.path.display(), hit.line, hit.preview),
                        )
                        .clicked()
                    {
                        *clicked = Some(index);
                    }
                }
            });
    }
}
fn footer_actions(
    ui: &mut Ui,
    panel: &mut Panel,
    busy: bool,
    command: &mut Option<Command>,
    reverse: bool,
) {
    let mut buttons = match panel.tab {
        Tab::Find => vec![
            ("Fermer", None, false),
            ("Compter", Some(Command::Count), false),
            ("Suivant ↵", Some(Command::Next), true),
        ],
        Tab::Replace => vec![
            ("Fermer", None, false),
            ("Compter", Some(Command::Count), false),
            ("Suivant ↵", Some(Command::Next), false),
            ("Tout remplacer", Some(Command::ReplaceAll), false),
            ("Remplacer", Some(Command::Replace), true),
        ],
        Tab::Files => vec![
            ("Fermer", None, false),
            ("Remplacer tout…", Some(Command::Files(true)), false),
            ("Rechercher ↵", Some(Command::Files(false)), true),
        ],
    };
    if reverse {
        buttons.reverse();
    }
    for (text, action, primary) in buttons {
        if enabled_button(ui, text, primary, action.is_none() || !busy).clicked() {
            if let Some(action) = action {
                *command = Some(action);
            } else {
                panel.open = false;
            }
        }
    }
}
fn footer(ui: &mut Ui, panel: &mut Panel, busy: bool, command: &mut Option<Command>) {
    ui.separator();
    egui::Frame::NONE
        .fill(SOFT)
        .inner_margin(egui::Margin::symmetric(14, 8))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            let status = if busy {
                "Traitement en cours…"
            } else if panel.message.is_empty() {
                "Prêt"
            } else {
                &panel.message
            }
            .to_owned();
            let color = if panel.message_error {
                ERROR
            } else {
                SECONDARY
            };
            let actions_width = match panel.tab {
                Tab::Find => 250.0,
                Tab::Replace => 430.0,
                Tab::Files => 290.0,
            };
            if ui.available_width() < actions_width + 120.0 || status.chars().count() > 80 {
                ui.add(
                    egui::Label::new(egui::RichText::new(&status).size(12.0).color(color)).wrap(),
                );
                ui.horizontal_wrapped(|ui| footer_actions(ui, panel, busy, command, false));
            } else {
                ui.horizontal(|ui| {
                    ui.allocate_ui(
                        vec2(
                            (ui.available_width() - actions_width - 10.0).max(80.0),
                            29.0,
                        ),
                        |ui| {
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(&status).size(12.0).color(color),
                                )
                                .wrap(),
                            );
                        },
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        footer_actions(ui, panel, busy, command, true)
                    });
                });
            }
            if busy && button(ui, "Annuler le traitement", false).clicked() {
                *command = Some(Command::Cancel);
            }
        });
}

pub(super) fn show(
    ctx: &egui::Context,
    panel: &mut Panel,
    wid: Id,
    open: &mut bool,
    command: &mut Option<Command>,
    clicked: &mut Option<usize>,
) {
    // Scope compact typography and colours to this floating dialog only.
    let previous_style = ctx.style();
    let mut style = (*previous_style).clone();
    for text in [
        egui::TextStyle::Body,
        egui::TextStyle::Button,
        egui::TextStyle::Heading,
    ] {
        style.text_styles.insert(text, FontId::proportional(13.0));
    }
    style
        .text_styles
        .insert(egui::TextStyle::Small, FontId::proportional(12.0));
    style.visuals.override_text_color = Some(INK);
    style.spacing.item_spacing = vec2(7.0, 6.0);
    style.spacing.button_padding = vec2(9.0, 4.0);
    style.spacing.interact_size = vec2(24.0, 24.0);
    style.spacing.icon_width = 14.0;
    style.spacing.icon_width_inner = 8.0;
    style.visuals.widgets.inactive.bg_fill = PAPER;
    style.visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, LINE);
    style.visuals.widgets.hovered.weak_bg_fill = ACTIVE;
    style.visuals.selection.bg_fill = ACTIVE;
    ctx.set_style(style);
    let busy = panel.job.is_some();
    egui::Window::new(egui::RichText::new("Recherche — MPTE").size(13.0))
        .id(egui::Id::new(("search-dialog", wid)))
        .open(open)
        .collapsible(false)
        .resizable(true)
        .default_height((ctx.screen_rect().height() - 64.0).max(240.0))
        .max_height((ctx.screen_rect().height() - 64.0).max(240.0))
        .frame(
            theme::dialog_frame()
                .fill(CHROME)
                .inner_margin(egui::Margin {
                    left: 0,
                    right: 0,
                    top: 6,
                    bottom: 6,
                }),
        )
        .default_width(662.0)
        .min_width(300.0)
        .max_width((ctx.screen_rect().width() - 24.0).max(300.0))
        .show(ctx, |ui| {
            // Keep the action row reachable when a small screen needs scrolling.
            let footer_space = if ui.available_width() < 550.0 || panel.message.chars().count() > 80
            {
                112.0
            } else {
                64.0
            };
            egui::ScrollArea::vertical()
                .id_salt("compact-content")
                .auto_shrink([false, true])
                .max_height((ctx.screen_rect().height() - 64.0 - footer_space).max(120.0))
                .show(ui, |ui| {
                    egui::Frame::NONE
                        .fill(PAPER)
                        .inner_margin(egui::Margin {
                            left: 14,
                            right: 14,
                            top: 12,
                            bottom: 12,
                        })
                        .show(ui, |ui| {
                            ui.set_min_width(ui.available_width());
                            egui::Frame::NONE
                                .fill(CHROME)
                                .stroke(Stroke::new(1.0, LINE))
                                .corner_radius(8)
                                .inner_margin(2)
                                .show(ui, |ui| {
                                    ui.spacing_mut().item_spacing.x = 2.0;
                                    ui.horizontal_wrapped(|ui| {
                                        for (tab, title) in [
                                            (Tab::Find, "Rechercher"),
                                            (Tab::Replace, "Remplacer"),
                                            (Tab::Files, "Dans les fichiers"),
                                        ] {
                                            let selected = panel.tab == tab;
                                            if ui
                                                .add(
                                                    egui::Button::new(
                                                        egui::RichText::new(title).color(
                                                            if selected { BLUE } else { SECONDARY },
                                                        ),
                                                    )
                                                    .fill(if selected {
                                                        PAPER
                                                    } else {
                                                        Color32::TRANSPARENT
                                                    })
                                                    .stroke(if selected {
                                                        Stroke::new(1.0, LINE)
                                                    } else {
                                                        Stroke::NONE
                                                    })
                                                    .corner_radius(6)
                                                    .min_size(vec2(0.0, 27.0))
                                                    .wrap_mode(egui::TextWrapMode::Extend),
                                                )
                                                .clicked()
                                            {
                                                panel.tab = tab;
                                            }
                                        }
                                    });
                                });
                            ui.add_space(4.0);
                            ui.add_enabled_ui(!busy, |ui| {
                                inputs(ui, panel, wid);
                                settings(ui, panel);
                                scope(ui, panel, command);
                            });
                            results(ui, panel, busy, command, clicked);
                        });
                });
            footer(ui, panel, busy, command);
        });
    ctx.set_style(previous_style);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compact_dialog_fits_reference_width_and_restores_editor_style() {
        for width in [1100.0, 450.0] {
            let ctx = egui::Context::default();
            theme::install(&ctx);
            let original = ctx.style().text_styles.clone();
            let mut panel = Panel::default();
            panel.open = true;
            for (tab_index, tab) in [Tab::Find, Tab::Replace, Tab::Files]
                .into_iter()
                .enumerate()
            {
                panel.tab = tab;
                for _ in 0..3 {
                    let _ = ctx.run(
                        egui::RawInput {
                            screen_rect: Some(egui::Rect::from_min_size(
                                egui::Pos2::ZERO,
                                vec2(width, 760.0),
                            )),
                            ..Default::default()
                        },
                        |ctx| {
                            show(ctx, &mut panel, 0, &mut true, &mut None, &mut None);
                        },
                    );
                }
                let rect = ctx
                    .memory(|m| m.area_rect(egui::Id::new(("search-dialog", 0_u64))))
                    .unwrap();
                assert!(
                    rect.width() <= (width - 20.0).min(666.0),
                    "tab {tab_index}, window {width}: {rect:?}"
                );
                assert!(rect.height() < 750.0, "{rect:?}");
            }
            assert_eq!(ctx.style().text_styles, original);
        }
    }
}
