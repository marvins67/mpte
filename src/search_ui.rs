use super::{EditorWindow, Id, State};
use crate::editor::record_undo;
use crate::search::{self, FileHit, FileOptions, Hit, Mode, Pattern, Query, Report};
use crate::theme;
use eframe::egui;
use std::{
    ops::Range,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
};

#[derive(Clone, Copy, Default, PartialEq)]
enum Tab {
    #[default]
    Find,
    Replace,
    Files,
}
enum Command {
    Next,
    Count,
    Current,
    Replace,
    ReplaceAll,
    AllOpen(bool),
    Files(bool),
    Confirm,
    Cancel,
}
enum JobResult {
    Scanned(Result<Report, String>),
    Applied(usize, Vec<String>),
}
pub struct Panel {
    pub open: bool,
    tab: Tab,
    pub query: Query,
    backward: bool,
    wrap: bool,
    in_selection: bool,
    scope: Option<(Id, u64, Range<usize>)>,
    focus: bool,
    folder: String,
    filters: String,
    recursive: bool,
    hidden: bool,
    pub message: String,
    history: Vec<String>,
    replacements: Vec<String>,
    results: Vec<FileHit>,
    report: Option<Report>,
    job: Option<mpsc::Receiver<JobResult>>,
    cancel: Arc<AtomicBool>,
    errors: Vec<String>,
    pub highlight: Option<(Id, u64, Range<usize>)>,
}

#[derive(Default)]
pub struct Quick {
    pub open: bool,
    focus: bool,
    query: String,
    cache: Option<(Id, u64, String, Vec<Range<usize>>)>,
}
impl Quick {
    pub fn show(&mut self, selected: Option<&str>) {
        self.open = true;
        self.focus = true;
        if let Some(text) = selected.filter(|s| !s.is_empty() && !s.contains('\n')) {
            self.query = text.into();
        }
    }
    pub fn highlight(&self, id: Id, revision: u64, job: &mut egui::text::LayoutJob) {
        if !self.open {
            return;
        }
        let Some((did, rev, _, ranges)) = &self.cache else {
            return;
        };
        if *did != id || *rev != revision {
            return;
        }
        let mut sections = Vec::new();
        let mut index = 0;
        for section in &job.sections {
            let mut start = section.byte_range.start;
            let end = section.byte_range.end;
            while index < ranges.len() && ranges[index].end <= start {
                index += 1;
            }
            let mut hit = index;
            while hit < ranges.len() && ranges[hit].start < end {
                let lo = start.max(ranges[hit].start);
                let hi = end.min(ranges[hit].end);
                if start < lo {
                    let mut s = section.clone();
                    s.byte_range = start..lo;
                    sections.push(s);
                }
                if lo < hi {
                    let mut s = section.clone();
                    s.byte_range = lo..hi;
                    s.format.background = egui::Color32::from_rgb(255, 234, 165);
                    sections.push(s);
                    start = hi;
                }
                hit += 1;
            }
            if start < end {
                let mut s = section.clone();
                s.byte_range = start..end;
                sections.push(s);
            }
        }
        job.sections = sections;
    }
}
pub fn quick_bar(
    ctx: &egui::Context,
    state: &State,
    window: &mut EditorWindow,
    wid: Id,
    actions: &mut Vec<super::Action>,
) {
    use crate::theme::{self, Icon};
    let mut quick = std::mem::take(&mut window.quick);
    if !quick.open {
        window.quick = quick;
        return;
    }
    let id = window.tabs.get(window.active).copied();
    let input_id = egui::Id::new(("quick-search", wid));
    let focused = ctx.memory(|m| m.has_focus(input_id));
    let mut direction = None;
    let mut changed = false;
    let mut close = (focused || ctx.memory(|m| m.had_focus_last_frame(input_id)))
        && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
    if focused {
        ctx.input_mut(|i| {
            if i.consume_key(egui::Modifiers::SHIFT, egui::Key::Enter) {
                direction = Some(-1);
            } else if i.consume_key(egui::Modifiers::NONE, egui::Key::Enter) {
                direction = Some(1);
            }
        });
    }
    let count = quick.cache.as_ref().map_or(0, |c| c.3.len());
    let current = quick
        .cache
        .as_ref()
        .and_then(|(_, _, _, ranges)| {
            window.search.highlight.as_ref().and_then(|(did, _, r)| {
                if Some(*did) == id {
                    ranges.iter().position(|h| h == r).map(|i| i + 1)
                } else {
                    None
                }
            })
        })
        .unwrap_or(0);
    egui::TopBottomPanel::top("quick-search")
        .show_separator_line(false)
        .exact_height(56.0)
        .frame(theme::panel_frame(theme::BACKGROUND))
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                let width = (ui.available_width() - 255.0).clamp(150.0, 480.0);
                egui::Frame::NONE
                    .fill(egui::Color32::WHITE)
                    .stroke(egui::Stroke::new(1.0, theme::BORDER))
                    .corner_radius(8)
                    .inner_margin(egui::Margin::symmetric(10, 5))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let (rect, _) = ui
                                .allocate_exact_size(egui::vec2(22.0, 22.0), egui::Sense::hover());
                            theme::icon(ui, rect, Icon::Search, theme::MUTED);
                            let response = ui.add_sized(
                                [width - 45.0, 24.0],
                                egui::TextEdit::singleline(&mut quick.query)
                                    .id(input_id)
                                    .frame(false)
                                    .hint_text("Rechercher dans ce fichier"),
                            );
                            if quick.focus {
                                response.request_focus();
                                quick.focus = false;
                            }
                            changed = response.changed();
                        });
                    });
                ui.label(egui::RichText::new(format!("{current} / {count}")).color(theme::MUTED));
                if theme::button(ui, Icon::Up, "", false)
                    .on_hover_text("Occurrence précédente · Maj+Entrée")
                    .clicked()
                {
                    direction = Some(-1);
                }
                if theme::button(ui, Icon::Down, "", false)
                    .on_hover_text("Occurrence suivante · Entrée")
                    .clicked()
                {
                    direction = Some(1);
                }
                if theme::button(ui, Icon::Settings, "", false)
                    .on_hover_text("Recherche avancée · Cmd/Ctrl+F")
                    .clicked()
                {
                    window.search.query.needle.clone_from(&quick.query);
                    actions.push(super::Action::Find);
                }
                if theme::button(ui, Icon::Close, "", false)
                    .on_hover_text("Fermer la recherche · Échap")
                    .clicked()
                {
                    close = true;
                }
            });
        });
    if let Some(id) = id {
        let doc = &state.docs[&id];
        let refresh = quick
            .cache
            .as_ref()
            .is_none_or(|(did, rev, q, _)| *did != id || *rev != doc.revision || *q != quick.query);
        if refresh {
            let hits = if quick.query.is_empty() {
                vec![]
            } else {
                Pattern::new(&Query {
                    needle: quick.query.clone(),
                    ..Default::default()
                })
                .map(|p| {
                    p.hits(&doc.text, 0..doc.text.len())
                        .into_iter()
                        .map(|h| h.range)
                        .collect()
                })
                .unwrap_or_default()
            };
            quick.cache = Some((id, doc.revision, quick.query.clone(), hits));
            ctx.request_repaint();
        }
        if changed || direction.is_some() {
            let ranges = &quick.cache.as_ref().unwrap().3;
            let caret = if changed || direction == Some(-1) {
                window.selection.start
            } else {
                window.selection.end
            };
            let hit = if direction == Some(-1) {
                ranges
                    .iter()
                    .rev()
                    .find(|r| r.end <= caret)
                    .or_else(|| ranges.last())
            } else {
                ranges
                    .iter()
                    .find(|r| r.start >= caret)
                    .or_else(|| ranges.first())
            };
            window.search.highlight = hit.map(|r| (id, doc.revision, r.clone()));
            if let Some(range) = hit {
                window.selection = range.clone();
                window.jump = Some((id, range.clone()));
            }
            ctx.request_repaint();
        }
    }
    if close {
        quick.open = false;
        window.search.highlight = None;
        if let Some(id) = id {
            ctx.memory_mut(|m| m.request_focus(egui::Id::new(("text", id))));
        }
        ctx.request_repaint();
    }
    window.quick = quick;
}
impl Default for Panel {
    fn default() -> Self {
        Self {
            open: false,
            tab: Tab::Find,
            query: Query::default(),
            backward: false,
            wrap: true,
            in_selection: false,
            scope: None,
            focus: false,
            folder: String::new(),
            filters: "*".into(),
            recursive: true,
            hidden: false,
            message: String::new(),
            history: vec![],
            replacements: vec![],
            results: vec![],
            report: None,
            job: None,
            cancel: search::cancel_token(),
            errors: vec![],
            highlight: None,
        }
    }
}
impl Drop for Panel {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}
impl Panel {
    pub fn show_find(&mut self, id: Option<Id>, state: &State, selection: &Range<usize>) {
        self.open = true;
        self.tab = Tab::Find;
        self.focus = true;
        self.scope = id
            .and_then(|id| {
                state
                    .docs
                    .get(&id)
                    .map(|d| (id, d.revision, selection.clone()))
            })
            .filter(|(_, _, r)| !r.is_empty());
        if self.scope.is_none() {
            self.in_selection = false;
        }
        if let Some((id, _, range)) = &self.scope {
            if let Some(s) = state.docs[id].text.get(range.clone()) {
                if !s.contains('\n') {
                    self.query.needle = s.to_owned();
                }
            }
        }
        if self.folder.is_empty() {
            self.folder = id
                .and_then(|id| {
                    state.docs[&id]
                        .path
                        .as_ref()?
                        .parent()
                        .map(|p| p.display().to_string())
                })
                .unwrap_or_default();
        }
    }
    fn scope(&self, state: &State, id: Id) -> Result<Range<usize>, String> {
        let doc = &state.docs[&id];
        if !self.in_selection {
            return Ok(0..doc.text.len());
        }
        match &self.scope {
            Some((sid, revision, range)) if *sid == id && *revision == doc.revision => {
                Ok(range.clone())
            }
            _ => Err(
                "La sélection a changé. Sélectionnez le texte puis rouvrez Rechercher avec Cmd+F."
                    .into(),
            ),
        }
    }
}
fn history_field(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut String,
    history: &[String],
    width: f32,
) -> egui::Response {
    ui.label(label);
    let response = ui.add_sized([width, 32.0], egui::TextEdit::singleline(value));
    egui::ComboBox::from_id_salt(label)
        .width(20.0)
        .selected_text("")
        .show_ui(ui, |ui| {
            for item in history {
                if ui.selectable_label(false, item).clicked() {
                    value.clone_from(item);
                }
            }
        });
    ui.end_row();
    response
}
fn remember(history: &mut Vec<String>, value: &str) {
    if value.is_empty() {
        return;
    }
    history.retain(|v| v != value);
    history.insert(0, value.to_owned());
    history.truncate(20);
}
pub fn draw(ctx: &egui::Context, state: &mut State, window: &mut EditorWindow, wid: Id) {
    let mut panel = std::mem::take(&mut window.search);
    if let Some(job) = &panel.job {
        match job.try_recv() {
            Ok(result) => {
                panel.job = None;
                match result {
                    JobResult::Scanned(Ok(report)) => {
                        panel.message = format!(
                            "{} occurrence(s) · {} fichier(s) lu(s) · {} ignoré(s){}",
                            report.count,
                            report.scanned,
                            report.skipped,
                            if report.limited {
                                " · affichage limité à 5 000 résultats"
                            } else {
                                ""
                            }
                        );
                        panel.results = report.hits.clone();
                        panel.errors = report.errors.clone();
                        if !report.plan.is_empty() {
                            panel.report = Some(report);
                        }
                    }
                    JobResult::Scanned(Err(error)) => panel.message = error,
                    JobResult::Applied(count, errors) => {
                        panel.message =
                            format!("{count} remplacement(s) enregistré(s) sur disque.");
                        panel.errors = errors;
                        panel.results.clear();
                    }
                }
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                panel.job = None;
                panel.message = "La tâche s’est arrêtée sans résultat.".into();
            }
            Err(mpsc::TryRecvError::Empty) => {}
        }
    }
    if !panel.open {
        window.search = panel;
        return;
    }
    let previous = (
        panel.tab,
        panel.query.clone(),
        panel.folder.clone(),
        panel.filters.clone(),
        panel.recursive,
        panel.hidden,
    );
    let enter = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
    let mut open = panel.open;
    let mut command = None;
    let mut clicked = None;
    let busy = panel.job.is_some();
    egui::Window::new("Rechercher").id(egui::Id::new(("search-dialog", wid)))
        .open(&mut open).title_bar(false).frame(theme::dialog_frame())
        .default_width(660.0).max_width((ctx.screen_rect().width()-24.0).max(320.0))
        .resizable(true).collapsible(false).show(ctx, |ui| {
            if theme::dialog_header(ui,theme::Icon::Search,"Rechercher") { panel.open = false; }
            egui::Frame::NONE.fill(theme::BACKGROUND).stroke(egui::Stroke::new(1.0,theme::BORDER)).show(ui, |ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.horizontal_wrapped(|ui| {
                    for (tab,label) in [(Tab::Find,"Rechercher"),(Tab::Replace,"Remplacer"),(Tab::Files,"Rechercher dans les fichiers")] {
                        if theme::dialog_tab(ui,label,panel.tab == tab).clicked() { panel.tab = tab; }
                    }
                });
            });
            ui.add_space(8.0);
            let field_width = (ui.available_width()-180.0).clamp(150.0,360.0);
            ui.add_enabled_ui(!busy, |ui| {
                egui::Grid::new(("search-fields", wid)).num_columns(3).show(ui, |ui| {
                    let response = history_field(ui, "Recherche :", &mut panel.query.needle, &panel.history, field_width);
                    if panel.focus { response.request_focus(); panel.focus = false; }
                    if panel.tab != Tab::Find { history_field(ui, "Remplacer par :", &mut panel.query.replacement, &panel.replacements, field_width); }
                    if panel.tab == Tab::Files {
                        history_field(ui, "Filtres :", &mut panel.filters, &[], field_width).on_hover_text("Exemples : *.json;*.xml ou src/*.java. * pour tous les fichiers.");
                        ui.label("Dossier :"); ui.add_sized([field_width, 32.0], egui::TextEdit::singleline(&mut panel.folder));
                        if ui.button("…").clicked() { if let Some(path) = rfd::FileDialog::new().pick_folder() { panel.folder = path.display().to_string(); } } ui.end_row();
                    }
                });
                ui.horizontal_wrapped(|ui| {
                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                    ui.checkbox(&mut panel.query.word, "Mot entier uniquement"); ui.checkbox(&mut panel.query.case, "Respecter la casse");
                    if panel.tab == Tab::Files { ui.checkbox(&mut panel.recursive, "Sous-dossiers"); ui.checkbox(&mut panel.hidden, "Dossiers et fichiers cachés"); }
                    else { ui.checkbox(&mut panel.backward, "Rechercher vers l’arrière"); ui.checkbox(&mut panel.wrap, "Boucler");
                        ui.add_enabled(panel.scope.is_some(), egui::Checkbox::new(&mut panel.in_selection, "Dans la sélection")); }
                });
                ui.group(|ui| {
                    ui.label("Mode de recherche"); ui.horizontal_wrapped(|ui| {
                        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                        ui.radio_value(&mut panel.query.mode, Mode::Normal, "Normal");
                        ui.radio_value(&mut panel.query.mode, Mode::Extended, r"Étendu (\n, \r, \t, \xNN, \uNNNN…)");
                        ui.radio_value(&mut panel.query.mode, Mode::Regex, "Expression régulière");
                    });
                    if panel.query.mode == Mode::Regex {
                        ui.checkbox(&mut panel.query.dot_newline, "Le point comprend les fins de ligne");
                        ui.small("Groupes de remplacement : $1, ${nom}. Les références arrière et assertions avant/arrière ne sont pas prises en charge.");
                    }
                });
                ui.horizontal_wrapped(|ui| {
                    if panel.tab == Tab::Files {
                        if theme::primary_button(ui,"Trouver tout").clicked() { command = Some(Command::Files(false)); }
                        if ui.button("Remplacer tout…").clicked() { command = Some(Command::Files(true)); }
                    } else {
                        if theme::primary_button(ui,"Suivant").clicked() { command = Some(Command::Next); }
                        if panel.tab == Tab::Find {
                            if ui.button("Compter").clicked() { command = Some(Command::Count); }
                            if ui.button("Rechercher dans le document actuel").clicked() { command = Some(Command::Current); }
                            if ui.button("Rechercher dans tous les documents ouverts").clicked() { command = Some(Command::AllOpen(false)); }
                        } else {
                            if ui.button("Remplacer").clicked() { command = Some(Command::Replace); }
                            if ui.button("Remplacer tout").clicked() { command = Some(Command::ReplaceAll); }
                            if ui.button("Remplacer dans tous les documents ouverts").clicked() { command = Some(Command::AllOpen(true)); }
                        }
                    }
                });
            });
            if busy { ui.horizontal(|ui| { ui.spinner(); ui.label("Traitement en cours…"); if ui.button("Annuler le traitement").clicked() { command = Some(Command::Cancel); } }); }
            if let Some((count, files)) = panel.report.as_ref().map(|r| (r.count, r.plan.len())) {
                ui.group(|ui| {
                    ui.label(format!("Confirmer {count} remplacement(s) dans {files} fichier(s) sur disque ?"));
                    ui.label("Les fichiers ouverts dans MPTE seront ignorés. Les autres modifications seront enregistrées immédiatement.");
                    ui.horizontal(|ui| {
                        if ui.add_enabled(!busy, egui::Button::new("Confirmer les remplacements")).clicked() { command = Some(Command::Confirm); }
                        if ui.button("Abandonner").clicked() { panel.report = None; }
                    });
                });
            }
            if !panel.message.is_empty() {
                egui::Frame::NONE.fill(theme::BACKGROUND).corner_radius(8).inner_margin(10).show(ui, |ui| {
                    ui.label(egui::RichText::new(&panel.message).color(theme::MUTED));
                });
            }
            if !panel.errors.is_empty() { ui.collapsing(format!("{} problème(s)", panel.errors.len()), |ui| { for error in &panel.errors { ui.label(error); } }); }
            if !panel.results.is_empty() {
                egui::ScrollArea::vertical().id_salt("search-results").max_height(220.0).show(ui, |ui| {
                    for (index, hit) in panel.results.iter().enumerate() {
                        if ui.selectable_label(false, format!("{}:{}  {}", hit.path.display(), hit.line, hit.preview)).clicked() { clicked = Some(index); }
                    }
                });
            }
            if ui.button("Fermer").clicked() { panel.open = false; }
        });
    if previous
        != (
            panel.tab,
            panel.query.clone(),
            panel.folder.clone(),
            panel.filters.clone(),
            panel.recursive,
            panel.hidden,
        )
    {
        panel.report = None;
        panel.results.clear();
        panel.highlight = None;
        panel.message.clear();
    }
    panel.open &= open;
    if panel.open && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
        panel.open = false;
    }
    if panel.open && !busy && enter {
        command = Some(if panel.tab == Tab::Files {
            Command::Files(false)
        } else {
            Command::Next
        });
    }
    if let Some(index) = clicked {
        let hit = panel.results[index].clone();
        if let Err(error) = navigate_result(ctx, state, window, &mut panel, hit) {
            panel.message = error;
        }
    }
    if let Some(command) = command {
        if let Err(error) = execute(ctx, state, window, &mut panel, command) {
            panel.message = error;
        }
    }
    window.search = panel;
}
fn execute(
    ctx: &egui::Context,
    state: &mut State,
    window: &mut EditorWindow,
    panel: &mut Panel,
    command: Command,
) -> Result<(), String> {
    if matches!(command, Command::Cancel) {
        panel.cancel.store(true, Ordering::Relaxed);
        return Ok(());
    }
    if matches!(command, Command::Confirm) {
        let report = panel
            .report
            .take()
            .ok_or("Aucun remplacement à confirmer")?;
        let excluded: Vec<_> = state.docs.values().filter_map(|d| d.path.clone()).collect();
        let cancel = search::cancel_token();
        panel.cancel = cancel.clone();
        let (tx, rx) = mpsc::channel();
        panel.job = Some(rx);
        let ctx = ctx.clone();
        let viewport = ctx.viewport_id();
        std::thread::spawn(move || {
            let (n, errors) = search::apply_plan(report.plan, &excluded, &cancel);
            let _ = tx.send(JobResult::Applied(n, errors));
            ctx.request_repaint_of(viewport);
            ctx.request_repaint_of(egui::ViewportId::ROOT);
        });
        return Ok(());
    }
    let mut query = panel.query.clone();
    if matches!(
        command,
        Command::Next
            | Command::Count
            | Command::Current
            | Command::AllOpen(false)
            | Command::Files(false)
    ) {
        query.replacement.clear();
    }
    let pattern = Pattern::new(&query)?;
    remember(&mut panel.history, &panel.query.needle);
    remember(&mut panel.replacements, &panel.query.replacement);
    if let Command::Files(replace) = command {
        panel.report = None;
        panel.results.clear();
        panel.errors.clear();
        let options = FileOptions {
            root: PathBuf::from(&panel.folder),
            filters: panel.filters.clone(),
            recursive: panel.recursive,
            hidden: panel.hidden,
        };
        let cancel = search::cancel_token();
        panel.cancel = cancel.clone();
        let (tx, rx) = mpsc::channel();
        panel.job = Some(rx);
        let ctx = ctx.clone();
        let viewport = ctx.viewport_id();
        std::thread::spawn(move || {
            let result = search::scan(&pattern, &options, replace, &cancel);
            let _ = tx.send(JobResult::Scanned(result));
            ctx.request_repaint_of(viewport);
            ctx.request_repaint_of(egui::ViewportId::ROOT);
        });
        return Ok(());
    }
    if let Command::AllOpen(replace) = command {
        let mut count = 0;
        let mut files = 0;
        panel.results.clear();
        // All open documents includes detached windows. Unsaved buffers are used.
        for (id, doc) in &mut state.docs {
            let hits = pattern.hits(&doc.text, 0..doc.text.len());
            if !hits.is_empty() {
                files += 1;
            }
            count += hits.len();
            if replace && !hits.is_empty() {
                record_undo(ctx, *id, &doc.text);
                let len = doc.text.len();
                pattern.replace_all(&mut doc.text, 0..len);
                doc.revision += 1;
            } else if !replace {
                for hit in hits {
                    if panel.results.len() < 5000 {
                        panel.results.push(buffer_hit(*id, doc, hit.range));
                    }
                }
            }
        }
        panel.message = format!(
            "{count} occurrence(s) dans {files} document(s) ouvert(s){}. Les documents modifiés restent à enregistrer.",
            if replace { " remplacée(s)" } else { "" }
        );
        ctx.request_repaint_of(egui::ViewportId::ROOT);
        return Ok(());
    }
    let id = *window
        .tabs
        .get(window.active)
        .ok_or("Aucun document actif.")?;
    let scope = panel.scope(state, id)?;
    let doc = state.docs.get_mut(&id).unwrap();
    let hits = pattern.hits(&doc.text, scope.clone());
    match command {
        Command::Current => {
            panel.results = hits
                .iter()
                .take(5000)
                .map(|h| buffer_hit(id, doc, h.range.clone()))
                .collect();
            panel.message = format!(
                "{} occurrence(s), affichage limité à 5 000 résultats.",
                hits.len()
            );
        }
        Command::Count => {
            panel.message = format!("{} occurrence(s).", hits.len());
        }
        Command::ReplaceAll => {
            if !hits.is_empty() {
                record_undo(ctx, id, &doc.text);
            }
            let old_len = doc.text.len();
            let count = pattern.replace_all(&mut doc.text, scope.clone());
            if count > 0 {
                doc.revision += 1;
            }
            if panel.in_selection {
                panel.scope = Some((
                    id,
                    doc.revision,
                    scope.start
                        ..scope
                            .end
                            .saturating_add_signed(doc.text.len() as isize - old_len as isize),
                ));
            }
            panel.highlight = None;
            panel.message = format!("{count} remplacement(s). Le document reste à enregistrer.");
        }
        Command::Next | Command::Replace => {
            let mut selected = window.selection.clone();
            if matches!(command, Command::Replace) {
                if let Some(hit) = hits.iter().find(|h| h.range == selected) {
                    record_undo(ctx, id, &doc.text);
                    doc.text.replace_range(hit.range.clone(), &hit.replacement);
                    doc.revision += 1;
                    let new_end = hit.range.start + hit.replacement.len();
                    selected = if panel.backward {
                        hit.range.start..hit.range.start
                    } else {
                        new_end..new_end
                    };
                    let end = scope.end.saturating_add_signed(
                        hit.replacement.len() as isize - hit.range.len() as isize,
                    );
                    if panel.in_selection {
                        panel.scope = Some((id, doc.revision, scope.start..end));
                    }
                    window.selection = selected.clone();
                }
            }
            let scope = if panel.in_selection {
                panel.scope.as_ref().unwrap().2.clone()
            } else {
                0..doc.text.len()
            };
            let hits = pattern.hits(&doc.text, scope);
            let skip_zero = panel
                .highlight
                .as_ref()
                .filter(|(did, rev, _)| *did == id && *rev == doc.revision)
                .map(|(_, _, r)| r.clone());
            let next = choose(
                &hits,
                &selected,
                panel.backward,
                panel.wrap,
                skip_zero.as_ref(),
            );
            if let Some(hit) = next {
                window.jump = Some((id, hit.range.clone()));
                window.selection = hit.range.clone();
                panel.highlight = Some((id, doc.revision, hit.range.clone()));
                panel.message = format!(
                    "Occurrence {} sur {}.",
                    hits.iter().position(|h| h.range == hit.range).unwrap() + 1,
                    hits.len()
                );
                ctx.request_repaint();
            } else {
                panel.message = "Aucune occurrence suivante dans le périmètre choisi.".into();
            }
        }
        _ => {}
    }
    Ok(())
}
fn choose<'a>(
    hits: &'a [Hit],
    selection: &Range<usize>,
    backward: bool,
    wrap: bool,
    previous: Option<&Range<usize>>,
) -> Option<&'a Hit> {
    let eligible = |h: &&Hit| !(h.range.is_empty() && previous == Some(&h.range));
    if backward {
        hits.iter()
            .rev()
            .filter(eligible)
            .find(|h| h.range.end <= selection.start)
            .or_else(|| if wrap { hits.last() } else { None })
    } else {
        hits.iter()
            .filter(eligible)
            .find(|h| h.range.start >= selection.end)
            .or_else(|| if wrap { hits.first() } else { None })
    }
}

fn buffer_hit(id: Id, doc: &crate::document::Document, range: Range<usize>) -> FileHit {
    let line = doc.text[..range.start]
        .bytes()
        .filter(|b| *b == b'\n')
        .count()
        + 1;
    let start = doc.text[..range.start].rfind('\n').map_or(0, |i| i + 1);
    let end = doc.text[range.start..]
        .find('\n')
        .map_or(doc.text.len(), |i| range.start + i);
    FileHit {
        path: doc
            .path
            .clone()
            .unwrap_or_else(|| PathBuf::from(format!("Sans titre #{id}"))),
        matched: doc.text[range.clone()].to_owned(),
        document_id: Some(id),
        range,
        line,
        preview: doc.text[start..end].chars().take(200).collect(),
    }
}
fn navigate_result(
    ctx: &egui::Context,
    state: &mut State,
    window: &mut EditorWindow,
    panel: &mut Panel,
    hit: FileHit,
) -> Result<(), String> {
    let existing = hit
        .document_id
        .filter(|id| state.docs.contains_key(id))
        .or_else(|| {
            state
                .docs
                .iter()
                .find(|(_, d)| d.path.as_ref() == Some(&hit.path))
                .map(|(id, _)| *id)
        });
    if hit.document_id.is_some() && existing.is_none() {
        return Err("Ce document a été fermé. Relancez la recherche.".into());
    }
    let id = if let Some(id) = existing {
        id
    } else {
        let previous_error = state.error.take();
        state.open(window, hit.path);
        if let Some(error) = state.error.take() {
            state.error = previous_error;
            return Err(error);
        }
        state.error = previous_error;
        *window
            .tabs
            .get(window.active)
            .ok_or("Ouverture impossible")?
    };
    let doc = &state.docs[&id];
    if doc.text.get(hit.range.clone()) != Some(hit.matched.as_str()) {
        return Err("Le contenu a changé depuis la recherche. Relancez-la avant de naviguer vers ce résultat.".into());
    }
    if let Some(index) = window.tabs.iter().position(|i| *i == id) {
        window.active = index;
        window.jump = Some((id, hit.range.clone()));
        panel.highlight = Some((id, doc.revision, hit.range));
    } else if let Some((wid, other)) = state.windows.iter_mut().find(|(_, w)| w.tabs.contains(&id))
    {
        other.active = other.tabs.iter().position(|i| *i == id).unwrap();
        other.jump = Some((id, hit.range.clone()));
        other.search.highlight = Some((id, doc.revision, hit.range));
        let viewport = if *wid == 0 {
            egui::ViewportId::ROOT
        } else {
            egui::ViewportId::from_hash_of(*wid)
        };
        ctx.send_viewport_cmd_to(viewport, egui::ViewportCommand::Focus);
        ctx.request_repaint_of(viewport);
    }
    ctx.request_repaint();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(text: &str) -> (State, EditorWindow, Panel, Id) {
        let mut state = State::new();
        let window = state.windows.remove(&0).unwrap();
        let id = window.tabs[0];
        state.docs.get_mut(&id).unwrap().text = text.into();
        let mut panel = Panel::default();
        panel.query.needle = "été".into();
        panel.query.replacement = "fin".into();
        (state, window, panel, id)
    }
    #[test]
    fn next_reverse_and_wrap_use_byte_ranges_for_unicode() {
        let (mut state, mut window, mut panel, id) = fixture("été été");
        let ctx = egui::Context::default();
        execute(&ctx, &mut state, &mut window, &mut panel, Command::Next).unwrap();
        assert_eq!(window.jump, Some((id, 0..5)));
        execute(&ctx, &mut state, &mut window, &mut panel, Command::Next).unwrap();
        assert_eq!(window.jump, Some((id, 6..11)));
        panel.wrap = false;
        execute(&ctx, &mut state, &mut window, &mut panel, Command::Next).unwrap();
        assert!(panel.message.contains("Aucune"));
        panel.wrap = true;
        execute(&ctx, &mut state, &mut window, &mut panel, Command::Next).unwrap();
        assert_eq!(window.jump, Some((id, 0..5)));
        panel.backward = true;
        execute(&ctx, &mut state, &mut window, &mut panel, Command::Next).unwrap();
        assert_eq!(window.jump, Some((id, 6..11)));
    }
    #[test]
    fn selection_replacement_and_stale_scope_are_safe() {
        let (mut state, mut window, mut panel, id) = fixture("x été été x");
        panel.scope = Some((id, 0, 2..7));
        panel.in_selection = true;
        let ctx = egui::Context::default();
        execute(
            &ctx,
            &mut state,
            &mut window,
            &mut panel,
            Command::ReplaceAll,
        )
        .unwrap();
        assert_eq!(state.docs[&id].text, "x fin été x");
        assert!(state.docs[&id].dirty());
        state.docs.get_mut(&id).unwrap().revision += 1;
        assert!(
            execute(
                &ctx,
                &mut state,
                &mut window,
                &mut panel,
                Command::ReplaceAll
            )
            .is_err()
        );
    }
    #[test]
    fn all_open_includes_unsaved_documents_and_detached_windows() {
        let (mut state, mut window, mut panel, _) = fixture("été");
        let mut other = EditorWindow::default();
        let mut doc = crate::document::Document::new();
        doc.text = "été été".into();
        state.add(&mut other, doc);
        state.windows.insert(500, other);
        let ctx = egui::Context::default();
        execute(
            &ctx,
            &mut state,
            &mut window,
            &mut panel,
            Command::AllOpen(false),
        )
        .unwrap();
        assert_eq!(panel.results.len(), 3);
        execute(
            &ctx,
            &mut state,
            &mut window,
            &mut panel,
            Command::AllOpen(true),
        )
        .unwrap();
        assert!(
            state
                .docs
                .values()
                .all(|d| !d.text.contains("été") && d.dirty())
        );
    }
    #[test]
    fn invalid_regex_never_modifies_document() {
        let (mut state, mut window, mut panel, id) = fixture("été");
        panel.query.mode = Mode::Regex;
        panel.query.needle = "[".into();
        assert!(
            execute(
                &egui::Context::default(),
                &mut state,
                &mut window,
                &mut panel,
                Command::ReplaceAll
            )
            .is_err()
        );
        assert_eq!(state.docs[&id].text, "été");
    }
    #[test]
    fn replacement_can_be_undone_even_without_prior_editor_focus() {
        let (mut state, mut window, mut panel, id) = fixture("été été");
        let ctx = egui::Context::default();
        execute(
            &ctx,
            &mut state,
            &mut window,
            &mut panel,
            Command::ReplaceAll,
        )
        .unwrap();
        assert_eq!(state.docs[&id].text, "fin fin");
        let edit = egui::TextEdit::load_state(&ctx, egui::Id::new(("text", id))).unwrap();
        let current = (
            edit.cursor.char_range().unwrap_or_default(),
            state.docs[&id].text.clone(),
        );
        let mut undoer = edit.undoer();
        assert_eq!(undoer.undo(&current).unwrap().1, "été été");
    }
    #[test]
    fn quick_search_counts_navigates_and_closes_without_changing_text() {
        let (mut state, mut window, _, id) = fixture("été été");
        let ctx = egui::Context::default();
        crate::theme::install(&ctx);
        window.quick.show(None);
        let frame = |window: &mut EditorWindow, events| {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1100.0, 760.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    quick_bar(ctx, &state, window, 0, &mut vec![]);
                },
            );
        };
        frame(&mut window, vec![]);
        frame(&mut window, vec![egui::Event::Text("été".into())]);
        assert_eq!(window.quick.cache.as_ref().unwrap().3.len(), 2);
        assert_eq!(window.jump, Some((id, 0..5)));
        let key = |modifiers, key| egui::Event::Key {
            key,
            physical_key: Some(key),
            pressed: true,
            repeat: false,
            modifiers,
        };
        frame(
            &mut window,
            vec![key(egui::Modifiers::NONE, egui::Key::Enter)],
        );
        assert_eq!(window.jump, Some((id, 6..11)));
        frame(
            &mut window,
            vec![key(egui::Modifiers::SHIFT, egui::Key::Enter)],
        );
        assert_eq!(window.jump, Some((id, 0..5)));
        frame(
            &mut window,
            vec![key(egui::Modifiers::NONE, egui::Key::Escape)],
        );
        assert!(!window.quick.open);
        assert!(window.search.highlight.is_none());
        assert_eq!(state.docs.get_mut(&id).unwrap().text, "été été");
    }
    #[test]
    fn quick_highlights_preserve_every_utf8_byte() {
        let mut quick = Quick::default();
        quick.show(None);
        quick.cache = Some((1, 0, "été".into(), vec![0..5, 6..11]));
        let text = "été été";
        let mut job = egui::text::LayoutJob::simple(
            text.into(),
            egui::FontId::monospace(15.0),
            egui::Color32::BLACK,
            f32::INFINITY,
        );
        quick.highlight(1, 0, &mut job);
        assert_eq!(job.text, text);
        let covered: String = job
            .sections
            .iter()
            .map(|s| &text[s.byte_range.clone()])
            .collect();
        assert_eq!(covered, text);
        assert_eq!(
            job.sections
                .iter()
                .filter(|s| s.format.background != egui::Color32::TRANSPARENT)
                .count(),
            2
        );
    }
}
