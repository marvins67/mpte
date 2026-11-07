mod chrome;
mod column;
mod document;
mod editor;
mod formatter;
mod outline;
mod search;
mod search_ui;
mod theme;

use document::Document;
use eframe::egui::{self, Color32, FontId, Key, Modifiers, RichText, ViewportCommand};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{Arc, Mutex, mpsc},
};
use syntect::{
    easy::HighlightLines, highlighting::ThemeSet, parsing::SyntaxSet, util::LinesWithEndings,
};

type Id = u64;
#[derive(Default)]
struct EditorWindow {
    tabs: Vec<Id>,
    active: usize,
    closing: Option<Close>,
    search: search_ui::Panel,
    quick: search_ui::Quick,
    selection: std::ops::Range<usize>,
    jump: Option<(Id, std::ops::Range<usize>)>,
}
#[derive(Clone, Copy)]
enum Close {
    Tab(Id),
    Window,
    Quit,
}
enum Action {
    New,
    Open,
    Save(bool),
    Format,
    Detach,
    Select(usize),
    Close(Close),
    NewWindow,
    Find,
    QuickFind,
}
struct FormatResult {
    id: Id,
    revision: u64,
    source: String,
    result: Result<String, String>,
    save: bool,
}
struct State {
    docs: BTreeMap<Id, Document>,
    windows: BTreeMap<Id, EditorWindow>,
    next: Id,
    error: Option<String>,
    syntax: SyntaxSet,
    themes: ThemeSet,
    quitting: bool,
    tx: mpsc::Sender<FormatResult>,
    rx: mpsc::Receiver<FormatResult>,
    busy: Vec<Id>,
}

impl State {
    fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        let mut themes = ThemeSet::load_defaults();
        themes.themes.insert("MPTE Light".into(), theme::syntax());
        let mut state = Self {
            docs: BTreeMap::new(),
            windows: BTreeMap::new(),
            next: 1,
            error: None,
            syntax: SyntaxSet::load_defaults_newlines(),
            themes,
            quitting: false,
            tx,
            rx,
            busy: vec![],
        };
        let mut window = EditorWindow::default();
        for path in std::env::args_os().skip(1) {
            state.open(&mut window, PathBuf::from(path));
        }
        if window.tabs.is_empty() {
            state.add(&mut window, Document::new());
        }
        state.windows.insert(0, window);
        state
    }
    fn id(&mut self) -> Id {
        let id = self.next;
        self.next += 1;
        id
    }
    fn add(&mut self, window: &mut EditorWindow, doc: Document) {
        let id = self.id();
        self.docs.insert(id, doc);
        window.tabs.push(id);
        window.active = window.tabs.len() - 1;
    }
    fn open(&mut self, window: &mut EditorWindow, path: PathBuf) {
        let path = path.canonicalize().unwrap_or(path);
        if let Some(id) = window
            .tabs
            .iter()
            .copied()
            .find(|id| self.docs[id].path.as_ref() == Some(&path))
        {
            self.remove_empty_untitled(window);
            window.active = window.tabs.iter().position(|tab| *tab == id).unwrap();
            return;
        }
        match Document::open(&path) {
            Ok(doc) => {
                if let Some(index) = self.remove_empty_untitled(window) {
                    let id = self.id();
                    self.docs.insert(id, doc);
                    window.tabs.insert(index, id);
                    window.active = index;
                } else {
                    self.add(window, doc);
                }
            }
            Err(e) => self.error = Some(e),
        }
    }
    fn remove_empty_untitled(&mut self, window: &mut EditorWindow) -> Option<usize> {
        let id = *window.tabs.get(window.active)?;
        let doc = &self.docs[&id];
        if doc.path.is_some() || !doc.text.is_empty() {
            return None;
        }
        let index = window.active;
        window.tabs.remove(index);
        self.docs.remove(&id);
        window.selection = 0..0;
        window.jump = None;
        Some(index)
    }
    fn save(&mut self, id: Id, save_as: bool) -> bool {
        let Some(path) = self.save_path(id, save_as) else {
            return false;
        };
        let doc = self.docs.get_mut(&id).unwrap();
        if doc.format_on_save {
            let extension = path
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("txt")
                .to_ascii_lowercase();
            match formatter::format(&doc.text, &extension) {
                Ok(text) => {
                    doc.text = text;
                    doc.revision += 1;
                }
                Err(e) => {
                    self.error = Some(e);
                    return false;
                }
            }
        }
        match doc.save(&path) {
            Ok(()) => true,
            Err(e) => {
                self.error = Some(e);
                false
            }
        }
    }
    fn save_path(&self, id: Id, save_as: bool) -> Option<PathBuf> {
        let doc = &self.docs[&id];
        if save_as || doc.path.is_none() {
            let mut dialog = rfd::FileDialog::new().set_file_name(doc.title());
            if let Some(parent) = doc.path.as_ref().and_then(|p| p.parent()) {
                dialog = dialog.set_directory(parent);
            }
            dialog.save_file()
        } else {
            doc.path.clone()
        }
    }
    fn format(&mut self, id: Id, save: bool, ctx: &egui::Context) {
        if self.busy.contains(&id) {
            return;
        }
        let doc = &self.docs[&id];
        let (source, extension, revision) = (doc.text.clone(), doc.extension(), doc.revision);
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        self.busy.push(id);
        std::thread::spawn(move || {
            let result = formatter::format(&source, &extension);
            let _ = tx.send(FormatResult {
                id,
                revision,
                source,
                result,
                save,
            });
            ctx.request_repaint();
            ctx.request_repaint_of(egui::ViewportId::ROOT);
        });
    }
    fn receive(&mut self) {
        while let Ok(message) = self.rx.try_recv() {
            self.busy.retain(|id| *id != message.id);
            let Some(doc) = self.docs.get_mut(&message.id) else {
                continue;
            };
            if doc.revision != message.revision || doc.text != message.source {
                self.error = Some("Le texte a changé pendant le formatage. Relancez le formatage ou l’enregistrement.".into());
                continue;
            }
            match message.result {
                Ok(text) => {
                    doc.text = text;
                    doc.revision += 1;
                    if message.save {
                        if let Some(path) = doc.path.clone() {
                            if let Err(error) = doc.save(&path) {
                                self.error = Some(error);
                            }
                        }
                    }
                }
                Err(error) => self.error = Some(error),
            }
        }
    }
}

fn layout(state: &State, text: &str, extension: &str, dark: bool) -> egui::text::LayoutJob {
    if text.len() > 2 * 1024 * 1024 {
        return egui::text::LayoutJob::simple(
            text.to_owned(),
            FontId::monospace(theme::EDITOR_FONT_SIZE),
            if dark { Color32::WHITE } else { Color32::BLACK },
            f32::INFINITY,
        );
    }
    let mut job = egui::text::LayoutJob::default();
    let font = FontId::monospace(theme::EDITOR_FONT_SIZE);
    let syntax = state
        .syntax
        .find_syntax_by_extension(extension)
        .unwrap_or_else(|| state.syntax.find_syntax_plain_text());
    let theme = &state.themes.themes[if dark {
        "base16-ocean.dark"
    } else {
        "MPTE Light"
    }];
    let mut highlighter = HighlightLines::new(syntax, theme);
    for line in LinesWithEndings::from(text) {
        match highlighter.highlight_line(line, &state.syntax) {
            Ok(ranges) => {
                for (style, segment) in ranges {
                    job.append(
                        segment,
                        0.0,
                        egui::TextFormat {
                            font_id: font.clone(),
                            color: Color32::from_rgb(
                                style.foreground.r,
                                style.foreground.g,
                                style.foreground.b,
                            ),
                            ..Default::default()
                        },
                    );
                }
            }
            Err(_) => job.append(
                line,
                0.0,
                egui::TextFormat {
                    font_id: font.clone(),
                    color: if dark { Color32::WHITE } else { Color32::BLACK },
                    ..Default::default()
                },
            ),
        }
    }
    job
}

fn search_highlight(job: &mut egui::text::LayoutJob, range: &std::ops::Range<usize>) {
    if range.is_empty() {
        return;
    }
    let mut sections = Vec::new();
    for section in &job.sections {
        let a = section.byte_range.start;
        let b = section.byte_range.end;
        let lo = a.max(range.start);
        let hi = b.min(range.end);
        if lo >= hi {
            sections.push(section.clone());
            continue;
        }
        if a < lo {
            let mut s = section.clone();
            s.byte_range = a..lo;
            sections.push(s);
        }
        let mut s = section.clone();
        s.byte_range = lo..hi;
        s.format.background = Color32::from_rgb(255, 223, 100);
        sections.push(s);
        if hi < b {
            let mut s = section.clone();
            s.byte_range = hi..b;
            sections.push(s);
        }
    }
    job.sections = sections;
}

fn draw(ctx: &egui::Context, state: &mut State, window_id: Id) {
    if state.quitting {
        return;
    }
    state.receive();
    let Some(mut window) = state.windows.remove(&window_id) else {
        return;
    };
    let mut actions = Vec::new();
    let active = window.tabs.get(window.active).copied();
    if ctx.input(|i| i.viewport().close_requested()) {
        ctx.send_viewport_cmd(ViewportCommand::CancelClose);
        actions.push(Action::Close(if window_id == 0 {
            Close::Quit
        } else {
            Close::Window
        }));
    }
    ctx.input_mut(|i| {
        for (modifiers, key, action) in [
            (
                Modifiers::COMMAND | Modifiers::SHIFT,
                Key::S,
                Action::Save(true),
            ),
            (
                Modifiers::COMMAND | Modifiers::SHIFT,
                Key::N,
                Action::NewWindow,
            ),
            (
                Modifiers::COMMAND | Modifiers::SHIFT,
                Key::F,
                Action::Format,
            ),
            (Modifiers::COMMAND, Key::N, Action::New),
            (Modifiers::COMMAND, Key::O, Action::Open),
            (Modifiers::COMMAND, Key::S, Action::Save(false)),
            (Modifiers::COMMAND, Key::F, Action::Find),
        ] {
            if i.consume_key(modifiers, key) {
                actions.push(action);
            }
        }
        if i.consume_key(Modifiers::COMMAND, Key::W) {
            if let Some(id) = active {
                actions.push(Action::Close(Close::Tab(id)));
            }
        }
        if i.consume_key(Modifiers::COMMAND, Key::Q) {
            actions.push(Action::Close(Close::Quit));
        }
    });
    chrome::toolbar(ctx, state, &window, &mut actions);
    search_ui::quick_bar(ctx, state, &mut window, window_id, &mut actions);
    chrome::status(ctx, state, &window);
    if let Some(id) = active {
        let doc = &state.docs[&id];
        ctx.send_viewport_cmd(ViewportCommand::Title(format!(
            "{}{} — MPTE",
            doc.title(),
            if doc.dirty() { " •" } else { "" }
        )));
        // Cache by content revision: cursor movement does not re-run syntax highlighting.
        let extension = doc.extension();
        let revision = doc.revision;
        let dark = ctx.style().visuals.dark_mode;
        let cache_id = egui::Id::new(("syntax", id));
        let cached =
            ctx.data_mut(|d| d.get_temp::<(u64, bool, String, egui::text::LayoutJob)>(cache_id));
        let mut job = match cached {
            Some((r, d, e, job)) if r == revision && d == dark && e == extension => job,
            _ => layout(state, &doc.text, &extension, dark),
        };
        ctx.data_mut(|d| d.insert_temp(cache_id, (revision, dark, extension.clone(), job.clone())));
        window.quick.highlight(id, revision, &mut job);
        {
            if let Some((did, rev, range)) = &window.search.highlight {
                if *did == id && *rev == revision {
                    search_highlight(&mut job, range);
                }
            }
        }
        let doc = state.docs.get_mut(&id).unwrap();
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(Color32::WHITE))
            .show(ctx, |ui| {
                editor::show(
                    ctx,
                    ui,
                    id,
                    doc,
                    job,
                    &mut window.selection,
                    &mut window.jump,
                );
            });
    } else {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.centered_and_justified(|ui| {
                ui.label(RichText::new("Ouvrez un fichier ou créez un nouvel onglet.").size(18.0));
            });
        });
    }
    let dropped: Vec<_> = ctx.input(|i| {
        i.raw
            .dropped_files
            .iter()
            .filter_map(|f| f.path.clone())
            .collect()
    });
    for path in dropped {
        state.open(&mut window, path);
    }
    if !actions.is_empty() {
        ctx.request_repaint_of(egui::ViewportId::ROOT);
    }
    for action in actions {
        let active = window.tabs.get(window.active).copied();
        match action {
            Action::New => state.add(&mut window, Document::new()),
            Action::Open => {
                if let Some(paths) = rfd::FileDialog::new().pick_files() {
                    for path in paths {
                        state.open(&mut window, path);
                    }
                }
            }
            Action::Save(save_as) => {
                if let Some(id) = active {
                    if state.docs[&id].format_on_save && !save_as && state.docs[&id].path.is_some()
                    {
                        state.format(id, true, ctx);
                    } else {
                        state.save(id, save_as);
                    }
                }
            }
            Action::Format => {
                if let Some(id) = active {
                    state.format(id, false, ctx);
                }
            }
            Action::QuickFind => {
                let selected =
                    active.and_then(|id| state.docs[&id].text.get(window.selection.clone()));
                window.quick.show(selected);
            }
            Action::Find => {
                window.search.show_find(active, state, &window.selection);
            }
            Action::Select(index) => window.active = index,
            Action::Detach => {
                if let Some(id) = active {
                    window.tabs.remove(window.active);
                    window.active = window.active.saturating_sub(1);
                    let wid = state.id();
                    state.windows.insert(
                        wid,
                        EditorWindow {
                            tabs: vec![id],
                            ..Default::default()
                        },
                    );
                }
            }
            Action::NewWindow => {
                let wid = state.id();
                let mut w = EditorWindow::default();
                state.add(&mut w, Document::new());
                state.windows.insert(wid, w);
            }
            Action::Close(close) => window.closing = Some(close),
        }
    }
    search_ui::draw(ctx, state, &mut window, window_id);
    let mut remove_window = false;
    if let Some(close) = window.closing {
        let ids: Vec<Id> = match close {
            Close::Tab(id) => vec![id],
            Close::Window => window.tabs.clone(),
            Close::Quit => state.docs.keys().copied().collect(),
        };
        let dirty: Vec<_> = ids
            .iter()
            .copied()
            .filter(|id| state.docs.get(id).is_some_and(Document::dirty))
            .collect();
        let mut proceed = dirty.is_empty();
        if !proceed {
            egui::Window::new("Modifications non enregistrées")
                .title_bar(false)
                .frame(theme::dialog_frame())
                .default_width(560.0)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    if theme::dialog_header(ui, theme::Icon::File, "Modifications non enregistrées")
                    {
                        window.closing = None;
                    }
                    ui.label("Enregistrer les documents modifiés avant de fermer ?");
                    for id in &dirty {
                        ui.label(state.docs[id].title());
                    }
                    ui.horizontal(|ui| {
                        if theme::primary_button(ui, "Enregistrer et fermer").clicked() {
                            proceed = dirty.iter().all(|id| state.save(*id, false));
                        }
                        if ui.button("Ne pas enregistrer").clicked() {
                            proceed = true;
                        }
                        if ui.button("Annuler").clicked() {
                            window.closing = None;
                        }
                    });
                });
        }
        if proceed {
            if !matches!(close, Close::Quit) {
                for id in &ids {
                    state.docs.remove(id);
                }
            }
            match close {
                Close::Tab(id) => {
                    window.tabs.retain(|i| *i != id);
                    window.active = window.active.min(window.tabs.len().saturating_sub(1));
                }
                Close::Window => {
                    remove_window = true;
                    ctx.send_viewport_cmd(ViewportCommand::Close);
                }
                Close::Quit => {
                    state.quitting = true;
                    ctx.send_viewport_cmd_to(egui::ViewportId::ROOT, ViewportCommand::Close);
                }
            }
            window.closing = None;
        }
    }
    if let Some(error) = state.error.clone() {
        egui::Window::new("MPTE — erreur")
            .id(egui::Id::new(("error-dialog", window_id)))
            .title_bar(false)
            .frame(theme::dialog_frame())
            .default_width(500.0)
            .max_width((ctx.screen_rect().width() - 24.0).max(300.0))
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                if theme::dialog_header(ui, theme::Icon::File, "Une erreur est survenue") {
                    state.error = None;
                }
                egui::Frame::NONE
                    .fill(theme::BACKGROUND)
                    .corner_radius(8)
                    .inner_margin(12)
                    .show(ui, |ui| {
                        egui::ScrollArea::vertical()
                            .max_height(220.0)
                            .show(ui, |ui| {
                                ui.add(egui::Label::new(error).wrap());
                            });
                    });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if theme::primary_button(ui, "OK").clicked() {
                            state.error = None;
                        }
                    });
                });
            });
    }
    if !remove_window {
        state.windows.insert(window_id, window);
    }
}

const ZOOM_PREFERENCE: &str = "zoom_factor";

struct App {
    state: Arc<Mutex<State>>,
    ctx: egui::Context,
}

impl App {
    fn new(ctx: &egui::Context, storage: Option<&dyn eframe::Storage>) -> Self {
        theme::install(ctx);
        if let Some(zoom) = storage
            .and_then(|storage| storage.get_string(ZOOM_PREFERENCE))
            .and_then(|value| value.parse::<f32>().ok())
            .filter(|zoom| (0.2..=5.0).contains(zoom))
        {
            ctx.set_zoom_factor(zoom);
        }
        Self {
            state: Arc::new(Mutex::new(State::new())),
            ctx: ctx.clone(),
        }
    }
}

impl eframe::App for App {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string(ZOOM_PREFERENCE, self.ctx.zoom_factor().to_string());
    }

    fn persist_egui_memory(&self) -> bool {
        false
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        theme::BACKGROUND.to_normalized_gamma_f32()
    }
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let windows = {
            let mut state = self.state.lock().unwrap();
            state.receive();
            draw(ctx, &mut state, 0);
            state
                .windows
                .keys()
                .copied()
                .filter(|id| *id != 0)
                .collect::<Vec<_>>()
        };
        for id in windows {
            let state = self.state.clone();
            ctx.show_viewport_deferred(
                egui::ViewportId::from_hash_of(id),
                egui::ViewportBuilder::default()
                    .with_title("MPTE")
                    .with_inner_size([1000.0, 720.0])
                    .with_min_inner_size([540.0, 360.0]),
                move |ctx, _| {
                    let mut state = state.lock().unwrap();
                    draw(ctx, &mut state, id);
                },
            );
        }
    }
}

fn main() -> eframe::Result {
    let viewport = egui::ViewportBuilder::default()
        .with_inner_size([1100.0, 760.0])
        .with_min_inner_size([540.0, 360.0]);
    // Keep the bundle's Dock icon instead of eframe's default runtime icon.
    #[cfg(target_os = "macos")]
    let viewport = viewport.with_icon(egui::IconData::default());

    eframe::run_native(
        "MPTE",
        eframe::NativeOptions {
            viewport,
            persist_window: false,
            ..Default::default()
        },
        Box::new(|cc| Ok(Box::new(App::new(&cc.egui_ctx, cc.storage)))),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::Storage as _;

    #[derive(Default)]
    struct TestStorage(BTreeMap<String, String>);

    impl eframe::Storage for TestStorage {
        fn get_string(&self, key: &str) -> Option<String> {
            self.0.get(key).cloned()
        }

        fn set_string(&mut self, key: &str, value: String) {
            self.0.insert(key.into(), value);
        }

        fn flush(&mut self) {}
    }

    fn zoom_shortcut(ctx: &egui::Context, key: Key) {
        let input = egui::RawInput {
            modifiers: Modifiers::COMMAND,
            events: vec![egui::Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::COMMAND,
            }],
            ..Default::default()
        };
        let _ = ctx.run(input, |_| {});
        let _ = ctx.run(egui::RawInput::default(), |_| {});
    }

    #[test]
    fn zoom_shortcuts_are_saved_without_restoring_documents() {
        let ctx = egui::Context::default();
        let mut app = App::new(&ctx, None);
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("document.txt");
        std::fs::write(&path, "Document ouvert avant de quitter").unwrap();
        {
            let mut state = app.state.lock().unwrap();
            let mut window = state.windows.remove(&0).unwrap();
            state.open(&mut window, path);
            assert!(state.docs[&window.tabs[window.active]].path.is_some());
            state.windows.insert(0, window);
        }

        zoom_shortcut(&ctx, Key::Plus);
        zoom_shortcut(&ctx, Key::Plus);
        zoom_shortcut(&ctx, Key::Minus);
        assert!((ctx.zoom_factor() - 1.1).abs() < f32::EPSILON);

        let mut storage = TestStorage::default();
        eframe::App::save(&mut app, &mut storage);
        assert_eq!(storage.0.len(), 1);
        assert_eq!(storage.get_string(ZOOM_PREFERENCE).as_deref(), Some("1.1"));
        assert!(!eframe::App::persist_egui_memory(&app));

        let restored_ctx = egui::Context::default();
        let mut restored = App::new(&restored_ctx, Some(&storage));
        let _ = restored_ctx.run(egui::RawInput::default(), |_| {});
        assert!((restored_ctx.zoom_factor() - 1.1).abs() < f32::EPSILON);
        {
            let state = restored.state.lock().unwrap();
            assert_eq!(state.docs.len(), 1);
            let doc = state.docs.values().next().unwrap();
            assert!(doc.path.is_none());
            assert!(doc.text.is_empty());
        }

        zoom_shortcut(&restored_ctx, Key::Num0);
        eframe::App::save(&mut restored, &mut storage);
        let reset_ctx = egui::Context::default();
        let _reset = App::new(&reset_ctx, Some(&storage));
        let _ = reset_ctx.run(egui::RawInput::default(), |_| {});
        assert_eq!(reset_ctx.zoom_factor(), 1.0);
    }

    #[test]
    fn invalid_zoom_preferences_use_the_default() {
        for value in ["", "invalide", "NaN", "inf", "-1", "0", "0.1", "5.1"] {
            let storage = TestStorage(BTreeMap::from([(ZOOM_PREFERENCE.into(), value.into())]));
            let ctx = egui::Context::default();
            let _app = App::new(&ctx, Some(&storage));
            let _ = ctx.run(egui::RawInput::default(), |_| {});
            assert_eq!(ctx.zoom_factor(), 1.0, "{value}");
        }
    }

    fn render(state: &mut State) {
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1100.0, 760.0),
            )),
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| draw(ctx, state, 0));
    }
    #[test]
    fn dirty_tab_close_waits_for_confirmation() {
        let mut state = State::new();
        let id = state.windows[&0].tabs[0];
        state.docs.get_mut(&id).unwrap().text = "À conserver".into();
        state.windows.get_mut(&0).unwrap().closing = Some(Close::Tab(id));
        render(&mut state);
        assert!(state.docs.contains_key(&id));
        assert!(state.windows[&0].closing.is_some());
    }
    #[test]
    fn clean_tab_closes_and_empty_window_renders() {
        let mut state = State::new();
        let id = state.windows[&0].tabs[0];
        state.windows.get_mut(&0).unwrap().closing = Some(Close::Tab(id));
        render(&mut state);
        assert!(!state.docs.contains_key(&id));
        render(&mut state);
    }
    #[test]
    fn stale_formatter_result_never_overwrites_edits() {
        let mut state = State::new();
        let id = state.windows[&0].tabs[0];
        state.docs.get_mut(&id).unwrap().text = "Nouvelle saisie".into();
        state
            .tx
            .send(FormatResult {
                id,
                revision: 0,
                source: "Ancienne saisie".into(),
                result: Ok("Résultat périmé".into()),
                save: true,
            })
            .unwrap();
        state.receive();
        assert_eq!(state.docs[&id].text, "Nouvelle saisie");
        assert!(state.error.is_some());
    }
    #[test]
    fn syntax_layout_preserves_every_character() {
        let state = State::new();
        for (ext, text) in [
            ("json", "{\"été\":1}\n"),
            ("xml", "<p>été</p>\n"),
            ("html", "<b>été</b>\n"),
            ("java", "class A { int n = 1; }\n"),
        ] {
            let job = layout(&state, text, ext, false);
            assert_eq!(job.text, text);
            assert!(job.sections.len() > 1, "Coloration manquante pour {ext}");
        }
    }
    #[test]
    fn shifted_new_shortcut_creates_window_instead_of_tab() {
        let mut state = State::new();
        let before = state.windows[&0].tabs.len();
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            events: vec![egui::Event::Key {
                key: Key::N,
                physical_key: Some(Key::N),
                pressed: true,
                repeat: false,
                modifiers: Modifiers::COMMAND | Modifiers::SHIFT,
            }],
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| draw(ctx, &mut state, 0));
        assert_eq!(state.windows.len(), 2);
        assert_eq!(state.windows[&0].tabs.len(), before);
    }
    #[test]
    fn command_f_opens_search_without_modifying_document() {
        let mut state = State::new();
        let id = state.windows[&0].tabs[0];
        let before = state.docs[&id].text.clone();
        let ctx = egui::Context::default();
        let _ = ctx.run(
            egui::RawInput {
                events: vec![egui::Event::Key {
                    key: Key::F,
                    physical_key: Some(Key::F),
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers::COMMAND,
                }],
                ..Default::default()
            },
            |ctx| draw(ctx, &mut state, 0),
        );
        assert!(state.windows[&0].search.open);
        assert_eq!(state.docs[&id].text, before);
    }
    #[test]
    fn opening_file_replaces_active_empty_untitled_in_place() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("file.txt");
        std::fs::write(&path, "contenu").unwrap();
        let mut state = State::new();
        let mut window = state.windows.remove(&0).unwrap();
        let placeholder = window.tabs[0];
        state.add(&mut window, Document::new());
        let other = window.tabs[1];
        window.active = 0;
        state.open(&mut window, path);
        assert_eq!(window.tabs.len(), 2);
        assert_eq!(window.active, 0);
        assert!(!state.docs.contains_key(&placeholder));
        assert_eq!(window.tabs[1], other);
        assert_eq!(state.docs[&window.tabs[0]].text, "contenu");
    }
    #[test]
    fn failed_open_and_nonempty_or_saved_documents_are_preserved() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("file.txt");
        std::fs::write(&path, "contenu").unwrap();
        let mut state = State::new();
        let mut window = state.windows.remove(&0).unwrap();
        let placeholder = window.tabs[0];
        state.open(&mut window, dir.path().join("missing.txt"));
        assert_eq!(window.tabs, vec![placeholder]);
        assert!(state.docs.contains_key(&placeholder));
        state.docs.get_mut(&placeholder).unwrap().text = " ".into();
        state.open(&mut window, path.clone());
        assert_eq!(window.tabs.len(), 2);
        assert_eq!(state.docs[&placeholder].text, " ");
        let saved = window.tabs[1];
        state.docs.get_mut(&saved).unwrap().text.clear();
        let another = dir.path().join("another.txt");
        std::fs::write(&another, "autre").unwrap();
        state.open(&mut window, another);
        assert_eq!(window.tabs.len(), 3);
        assert!(state.docs.contains_key(&saved));
    }
    #[test]
    fn reopening_existing_file_removes_active_empty_untitled_without_duplicate() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("file.txt");
        std::fs::write(&path, "contenu").unwrap();
        let mut state = State::new();
        let mut window = state.windows.remove(&0).unwrap();
        state.open(&mut window, path.clone());
        let file = window.tabs[0];
        state.add(&mut window, Document::new());
        let placeholder = window.tabs[1];
        state.open(&mut window, path);
        assert_eq!(window.tabs, vec![file]);
        assert_eq!(window.active, 0);
        assert!(!state.docs.contains_key(&placeholder));
    }
}
