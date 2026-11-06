use eframe::egui::{self, Color32, FontId, Rect, Response, Sense, Stroke, pos2, vec2};
use syntect::highlighting::{Color, StyleModifier, Theme, ThemeItem, ThemeSettings};

pub const BACKGROUND: Color32 = Color32::from_rgb(247, 249, 252);
pub const TOOLBAR: Color32 = Color32::from_rgb(242, 245, 249);
pub const BORDER: Color32 = Color32::from_rgb(218, 225, 235);
pub const INPUT_BORDER: Color32 = Color32::from_rgb(154, 170, 191);
pub const TEXT: Color32 = Color32::from_rgb(36, 45, 58);
pub const MUTED: Color32 = Color32::from_rgb(88, 103, 124);
pub const BLUE: Color32 = Color32::from_rgb(35, 89, 181);
pub const BLUE_LIGHT: Color32 = Color32::from_rgb(232, 240, 255);
pub const CURRENT_LINE: Color32 = Color32::from_rgb(238, 243, 253);
pub const EDITOR_FONT_SIZE: f32 = 14.0;

pub fn install(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.visuals = egui::Visuals::light();
    style.visuals.override_text_color = Some(TEXT);
    style.visuals.panel_fill = BACKGROUND;
    style.visuals.window_fill = Color32::WHITE;
    style.visuals.window_stroke = Stroke::new(1.0, BORDER);
    style.visuals.window_corner_radius = egui::CornerRadius::same(10);
    style.visuals.selection.bg_fill = Color32::from_rgb(219, 225, 235);
    style.visuals.selection.stroke = Stroke::new(1.0, BLUE);
    style.visuals.extreme_bg_color = Color32::WHITE;
    style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
    style.visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, MUTED);
    style.visuals.widgets.inactive.bg_fill = Color32::WHITE;
    style.spacing.icon_width = 18.0;
    style.spacing.icon_width_inner = 10.0;
    style.visuals.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    style.visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, BORDER);
    style.visuals.widgets.hovered.weak_bg_fill = BLUE_LIGHT;
    style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, BORDER);
    style.visuals.widgets.active.weak_bg_fill = BLUE_LIGHT;
    style.visuals.widgets.active.bg_stroke = Stroke::new(1.0, BLUE);
    style.visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(4);
    style.visuals.widgets.hovered.corner_radius = egui::CornerRadius::same(4);
    style.visuals.widgets.active.corner_radius = egui::CornerRadius::same(4);
    style.spacing.item_spacing = vec2(10.0, 8.0);
    style.spacing.button_padding = vec2(12.0, 7.0);
    style.spacing.interact_size = vec2(32.0, 30.0);
    style
        .text_styles
        .insert(egui::TextStyle::Body, FontId::proportional(16.0));
    style
        .text_styles
        .insert(egui::TextStyle::Button, FontId::proportional(16.0));
    style
        .text_styles
        .insert(egui::TextStyle::Small, FontId::proportional(13.0));
    style.text_styles.insert(
        egui::TextStyle::Monospace,
        FontId::monospace(EDITOR_FONT_SIZE),
    );
    ctx.set_style(style);
}

pub fn syntax() -> Theme {
    let color = |r, g, b| Color { r, g, b, a: 255 };
    let mut theme = Theme {
        name: Some("MPTE Light".into()),
        settings: ThemeSettings {
            foreground: Some(color(36, 45, 58)),
            background: Some(color(255, 255, 255)),
            ..Default::default()
        },
        ..Default::default()
    };
    for (selector, foreground) in [
        ("comment", color(121, 134, 150)),
        ("string", color(14, 104, 85)),
        ("constant.numeric, constant.language", color(35, 89, 181)),
        ("keyword, storage, support.function", color(119, 58, 175)),
        ("entity.name, variable", color(35, 89, 181)),
        (
            "meta.mapping.key string, meta.structure.dictionary.key.json string",
            color(35, 89, 181),
        ),
    ] {
        theme.scopes.push(ThemeItem {
            scope: selector.parse().expect("static syntax selector"),
            style: StyleModifier {
                foreground: Some(foreground),
                ..Default::default()
            },
        });
    }
    theme
}

#[derive(Clone, Copy)]
pub enum Icon {
    New,
    Folder,
    Save,
    Format,
    Search,
    More,
    File,
    Code,
    Up,
    Down,
    Settings,
    Close,
}
pub fn icon(ui: &egui::Ui, rect: Rect, kind: Icon, color: Color32) {
    let rect = rect.shrink(2.0);
    let stroke = Stroke::new(1.5, color);
    let p = |x: f32, y: f32| {
        pos2(
            rect.left() + rect.width() * x,
            rect.top() + rect.height() * y,
        )
    };
    let path = |points: &[(f32, f32)]| {
        ui.painter().add(egui::Shape::line(
            points.iter().map(|(x, y)| p(*x, *y)).collect(),
            stroke,
        ))
    };
    match kind {
        Icon::Search => {
            ui.painter()
                .circle_stroke(p(0.42, 0.42), rect.width() * 0.34, stroke);
            path(&[(0.68, 0.68), (0.98, 0.98)]);
        }
        Icon::Close => {
            path(&[(0.2, 0.2), (0.8, 0.8)]);
            path(&[(0.8, 0.2), (0.2, 0.8)]);
        }
        Icon::Up => {
            path(&[(0.22, 0.65), (0.5, 0.35), (0.78, 0.65)]);
        }
        Icon::Down => {
            path(&[(0.22, 0.35), (0.5, 0.65), (0.78, 0.35)]);
        }
        Icon::More => {
            for x in [0.15, 0.5, 0.85] {
                ui.painter().circle_filled(p(x, 0.5), 1.7, color);
            }
        }
        Icon::Format => {
            for (y, end) in [(0.2, 0.95), (0.5, 0.7), (0.8, 0.8)] {
                path(&[(0.05, y), (end, y)]);
            }
        }
        Icon::Settings => {
            for (y, x) in [(0.2, 0.65), (0.5, 0.3), (0.8, 0.65)] {
                path(&[(0.05, y), (0.95, y)]);
                path(&[(x, y - 0.12), (x, y + 0.12)]);
            }
        }
        Icon::Folder => {
            path(&[
                (0.05, 0.82),
                (0.05, 0.18),
                (0.36, 0.18),
                (0.48, 0.32),
                (0.88, 0.32),
                (0.88, 0.45),
            ]);
            path(&[
                (0.05, 0.82),
                (0.17, 0.45),
                (0.98, 0.45),
                (0.85, 0.82),
                (0.05, 0.82),
            ]);
        }
        Icon::Save => {
            path(&[
                (0.15, 0.05),
                (0.75, 0.05),
                (0.95, 0.25),
                (0.95, 0.95),
                (0.05, 0.95),
                (0.05, 0.05),
                (0.15, 0.05),
            ]);
            path(&[(0.25, 0.05), (0.25, 0.38), (0.72, 0.38), (0.72, 0.05)]);
            path(&[(0.25, 0.95), (0.25, 0.6), (0.75, 0.6), (0.75, 0.95)]);
        }
        Icon::New | Icon::File => {
            path(&[
                (0.7, 0.95),
                (0.1, 0.95),
                (0.1, 0.05),
                (0.62, 0.05),
                (0.88, 0.3),
                (0.88, 0.65),
            ]);
            path(&[(0.62, 0.05), (0.62, 0.3), (0.88, 0.3)]);
            if matches!(kind, Icon::New) {
                path(&[(0.65, 0.8), (1.0, 0.8)]);
                path(&[(0.83, 0.63), (0.83, 0.98)]);
            } else {
                for y in [0.45, 0.62, 0.79] {
                    path(&[(0.27, y), (0.65, y)]);
                }
            }
        }
        Icon::Code => {
            ui.painter()
                .rect_stroke(rect, 2.0, stroke, egui::StrokeKind::Inside);
            path(&[(0.22, 0.3), (0.4, 0.45), (0.22, 0.6)]);
            path(&[(0.52, 0.63), (0.78, 0.63)]);
        }
    }
}
pub fn button(ui: &mut egui::Ui, kind: Icon, label: &str, accent: bool) -> Response {
    let font = FontId::proportional(16.0);
    let text =
        ui.fonts(|f| f.layout_no_wrap(label.into(), font, if accent { BLUE } else { MUTED }));
    let width = if label.is_empty() {
        36.0
    } else {
        text.size().x + 52.0
    };
    let (rect, response) = ui.allocate_exact_size(vec2(width, 38.0), Sense::click());
    let fill = if response.hovered() {
        BLUE_LIGHT
    } else if accent {
        Color32::from_rgb(234, 241, 255)
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, 8.0, fill);
    if response.has_focus() {
        ui.painter()
            .rect_stroke(rect, 8.0, Stroke::new(1.0, BLUE), egui::StrokeKind::Inside);
    }
    icon(
        ui,
        Rect::from_center_size(pos2(rect.left() + 19.0, rect.center().y), vec2(22.0, 22.0)),
        kind,
        if accent { BLUE } else { MUTED },
    );
    if !label.is_empty() {
        ui.painter().galley(
            pos2(rect.left() + 37.0, rect.center().y - text.size().y / 2.0),
            text,
            TEXT,
        );
    }
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Button,
            ui.is_enabled(),
            if label.is_empty() {
                match kind {
                    Icon::New => "Nouveau",
                    Icon::Folder => "Ouvrir",
                    Icon::Save => "Enregistrer",
                    Icon::Format => "Formater",
                    Icon::Search => "Rechercher",
                    Icon::More => "Autres actions",
                    Icon::File => "Document",
                    Icon::Code => "Code",
                    Icon::Up => "Occurrence précédente",
                    Icon::Down => "Occurrence suivante",
                    Icon::Settings => "Options avancées",
                    Icon::Close => "Fermer",
                }
            } else {
                label
            },
        )
    });
    response
}
pub fn panel_frame(fill: Color32) -> egui::Frame {
    egui::Frame::NONE
        .fill(fill)
        .inner_margin(egui::Margin::symmetric(16, 8))
        .stroke(Stroke::new(1.0, BORDER))
}

pub fn dialog_frame() -> egui::Frame {
    egui::Frame::window(&egui::Style::default())
        .fill(Color32::WHITE)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(12)
        .shadow(egui::epaint::Shadow {
            offset: [0, 5],
            blur: 18,
            spread: 0,
            color: Color32::from_black_alpha(35),
        })
        .inner_margin(16)
}
pub fn dialog_header(ui: &mut egui::Ui, kind: Icon, title: &str) -> bool {
    let mut close = false;
    egui::Frame::NONE
        .fill(TOOLBAR)
        .corner_radius(8)
        .inner_margin(12)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(vec2(26.0, 26.0), Sense::hover());
                icon(ui, rect, kind, BLUE);
                ui.label(egui::RichText::new(title).size(19.0).strong().color(TEXT));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    close = button(ui, Icon::Close, "", false)
                        .on_hover_text("Fermer")
                        .clicked();
                });
            });
        });
    ui.add_space(8.0);
    close
}
pub fn primary_button(ui: &mut egui::Ui, label: &str) -> Response {
    ui.add(
        egui::Button::new(egui::RichText::new(label).color(BLUE))
            .fill(BLUE_LIGHT)
            .stroke(Stroke::NONE)
            .corner_radius(8)
            .wrap_mode(egui::TextWrapMode::Extend)
            .min_size(vec2(96.0, 36.0)),
    )
}
