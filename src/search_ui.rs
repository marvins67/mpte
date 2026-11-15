mod compact;
use super::{EditorWindow, Id, State};
use crate::editor::record_undo;
use crate::search::{self, FileHit, FileOptions, Mode, Pattern, Query, Report};
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
    message_error: bool,
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
    total: usize,
    current: usize,
}
impl Quick {
    pub fn show(&mut self, selected: Option<&str>) {
        self.open = true;
        self.focus = true;
        if let Some(text) = selected.filter(|s| !s.is_empty() && !s.contains('\n')) {
            self.query = text.into();
        }
    }
    pub fn highlight(&self, id: Id, revision: u64, job: &mut Arc<egui::text::LayoutJob>) {
        if !self.open {
            return;
        }
        let Some((did, rev, _, ranges)) = &self.cache else {
            return;
        };
        if *did != id || *rev != revision {
            return;
        }
        if ranges.is_empty() {
            return;
        }
        let job = Arc::make_mut(job);
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
    let count = quick.total;
    let current = quick.current;
    egui::TopBottomPanel::top("quick-search")
        .show_separator_line(false)
        .exact_height(56.0)
        .frame(theme::panel_frame(theme::BACKGROUND))
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                let width = (ui.available_width() - 255.0).clamp(150.0, 480.0);
                egui::Frame::NONE
                    .fill(egui::Color32::WHITE)
                    .stroke(egui::Stroke::new(1.0, theme::INPUT_BORDER))
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
            let mut hits = Vec::new();
            quick.total = 0;
            quick.current = 0;
            if !quick.query.is_empty() {
                if let Ok(pattern) = Pattern::new(&Query {
                    needle: quick.query.clone(),
                    ..Default::default()
                }) {
                    for range in pattern.ranges(&doc.text, 0..doc.text.len()) {
                        quick.total += 1;
                        if hits.len() < search::MAX_RESULTS {
                            hits.push(range);
                        }
                    }
                }
            }
            quick.cache = Some((id, doc.revision, quick.query.clone(), hits));
            ctx.request_repaint();
        }
        if changed || direction.is_some() {
            let caret = if changed || direction == Some(-1) {
                window.selection.start
            } else {
                window.selection.end
            };
            let pattern = Pattern::new(&Query {
                needle: quick.query.clone(),
                ..Default::default()
            });
            let hit = pattern.ok().and_then(|pattern| {
                choose(
                    pattern.ranges(&doc.text, 0..doc.text.len()),
                    &(caret..caret),
                    direction == Some(-1),
                    true,
                    None,
                )
                .0
            });
            quick.current = hit.as_ref().map_or(0, |(_, index)| *index);
            window.search.highlight = hit
                .as_ref()
                .map(|(range, _)| (id, doc.revision, range.clone()));
            if let Some((range, _)) = hit {
                window.selection = range.clone();
                window.jump = Some((id, range));
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
            message_error: false,
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
                    JobResult::Scanned(Ok(mut report)) => {
                        panel.message_error = false;
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
                        panel.results = std::mem::take(&mut report.hits);
                        panel.errors = std::mem::take(&mut report.errors);
                        if !report.plan.is_empty() {
                            panel.report = Some(report);
                        }
                    }
                    JobResult::Scanned(Err(error)) => {
                        panel.message = error;
                        panel.message_error = true;
                    }
                    JobResult::Applied(count, errors) => {
                        panel.message_error = !errors.is_empty();
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
                panel.message_error = true;
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
    // Consume Escape before a popup or field can also close the whole dialog.
    let had_popup = ctx.memory(|m| m.any_popup_open());
    let escape = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
    if escape && had_popup {
        ctx.memory_mut(|m| m.close_popup());
    } else if escape {
        panel.open = false;
    }
    let enter = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
    let mut open = panel.open;
    let mut command = None;
    let mut clicked = None;
    let busy = panel.job.is_some();
    compact::show(ctx, &mut panel, wid, &mut open, &mut command, &mut clicked);
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
        panel.message_error = false;
    }
    panel.open &= open;
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
        panel.message_error = false;
        if let Err(error) = execute(ctx, state, window, &mut panel, command) {
            panel.message = error;
            panel.message_error = true;
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
    let mut result_bytes = 0;
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
            let mut found = 0;
            if replace {
                if pattern
                    .ranges(&doc.text, 0..doc.text.len())
                    .next()
                    .is_some()
                {
                    record_undo(ctx, *id, &doc.text);
                    let len = doc.text.len();
                    found = pattern.replace_all(&mut doc.text, 0..len);
                    doc.revision += 1;
                }
            } else {
                for range in pattern.ranges(&doc.text, 0..doc.text.len()) {
                    found += 1;
                    push_buffer_result(&mut panel.results, &mut result_bytes, *id, doc, range);
                }
            }
            files += usize::from(found > 0);
            count += found;
        }
        panel.message = format!(
            "{count} occurrence(s) dans {files} document(s) ouvert(s){}. Les documents modifiés restent à enregistrer.",
            if replace { " remplacée(s)" } else { "" }
        );
        state.repaint_windows(ctx);
        return Ok(());
    }
    let id = *window
        .tabs
        .get(window.active)
        .ok_or("Aucun document actif.")?;
    let scope = panel.scope(state, id)?;
    let doc = state.docs.get_mut(&id).unwrap();
    match command {
        Command::Current => {
            panel.results.clear();
            let mut count = 0;
            for range in pattern.ranges(&doc.text, scope) {
                count += 1;
                push_buffer_result(&mut panel.results, &mut result_bytes, id, doc, range);
            }
            panel.message =
                format!("{count} occurrence(s), affichage limité à 5 000 résultats et 8 Mio.");
        }
        Command::Count => {
            panel.message = format!(
                "{} occurrence(s).",
                pattern.ranges(&doc.text, scope).count()
            );
        }
        Command::ReplaceAll => {
            if pattern.ranges(&doc.text, scope.clone()).next().is_some() {
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
                let replacement = pattern
                    .iter(&doc.text, scope.clone())
                    .find(|h| h.range == selected);
                if let Some(hit) = replacement {
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
            let skip_zero = panel
                .highlight
                .as_ref()
                .filter(|(did, rev, _)| *did == id && *rev == doc.revision)
                .map(|(_, _, r)| r.clone());
            let (next, total) = choose(
                pattern.ranges(&doc.text, scope),
                &selected,
                panel.backward,
                panel.wrap,
                skip_zero.as_ref(),
            );
            if let Some((range, index)) = next {
                window.jump = Some((id, range.clone()));
                window.selection = range.clone();
                panel.highlight = Some((id, doc.revision, range));
                panel.message = format!("Occurrence {index} sur {total}.");
                ctx.request_repaint();
            } else {
                panel.message = "Aucune occurrence suivante dans le périmètre choisi.".into();
            }
        }
        _ => {}
    }
    Ok(())
}
fn choose(
    hits: impl Iterator<Item = Range<usize>>,
    selection: &Range<usize>,
    backward: bool,
    wrap: bool,
    previous: Option<&Range<usize>>,
) -> (Option<(Range<usize>, usize)>, usize) {
    let mut first = None;
    let mut last = None;
    let mut next = None;
    let mut count = 0;
    for range in hits {
        count += 1;
        if first.is_none() {
            first = Some((range.clone(), count));
        }
        let eligible = !(range.is_empty() && previous == Some(&range));
        if eligible
            && ((backward && range.end <= selection.start)
                || (!backward && range.start >= selection.end && next.is_none()))
        {
            next = Some((range.clone(), count));
        }
        last = Some((range, count));
    }
    if next.is_none() && wrap {
        next = if backward { last } else { first };
    }
    (next, count)
}

fn push_buffer_result(
    results: &mut Vec<FileHit>,
    used: &mut usize,
    id: Id,
    doc: &crate::document::Document,
    range: Range<usize>,
) {
    if results.len() >= search::MAX_RESULTS {
        return;
    }
    // A bound in bytes also covers a regex matching an entire very large document.
    const BUDGET: usize = 8 * 1024 * 1024;
    let path_bytes = doc.path.as_ref().map_or(32, |path| path.as_os_str().len());
    if used
        .saturating_add(range.len())
        .saturating_add(path_bytes + 800 + std::mem::size_of::<FileHit>())
        > BUDGET
    {
        return;
    }
    let hit = buffer_hit(id, doc, range);
    *used += hit.matched.len()
        + hit.preview.len()
        + hit.path.as_os_str().len()
        + std::mem::size_of::<FileHit>();
    results.push(hit);
}

fn buffer_hit(id: Id, doc: &crate::document::Document, range: Range<usize>) -> FileHit {
    let line = doc.line_at_byte(range.start) + 1;
    let bounds = doc.line_bounds(range.start);
    let (start, end) = (bounds.start, bounds.end);
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
    fn streaming_navigation_reaches_matches_beyond_the_display_limit() {
        let pattern = Pattern::new(&Query {
            needle: "é".into(),
            ..Default::default()
        })
        .unwrap();
        let text = "é ".repeat(6000);
        let (next, total) = choose(
            pattern.ranges(&text, 0..text.len()),
            &(15_000..15_000),
            false,
            true,
            None,
        );
        assert_eq!(total, 6000);
        assert_eq!(next, Some((15_000..15_002, 5001)));
        let (previous, _) = choose(
            pattern.ranges(&text, 0..text.len()),
            &(15_000..15_000),
            true,
            true,
            None,
        );
        assert_eq!(previous, Some((14_997..14_999, 5000)));
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
        let mut job = Arc::new(egui::text::LayoutJob::simple(
            text.into(),
            egui::FontId::monospace(15.0),
            egui::Color32::BLACK,
            f32::INFINITY,
        ));
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
    fn dialog_frame(
        ctx: &egui::Context,
        state: &mut State,
        window: &mut EditorWindow,
        events: Vec<egui::Event>,
    ) {
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
                draw(ctx, state, window, 0);
            },
        );
    }
    fn key_event(key: egui::Key) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: Some(key),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }
    }
    #[test]
    fn compact_dialog_keeps_enter_navigation_and_escape_closure() {
        let (mut state, mut window, panel, id) = fixture("été été");
        window.search = panel;
        window.search.show_find(Some(id), &state, &(0..0));
        let ctx = egui::Context::default();
        crate::theme::install(&ctx);
        dialog_frame(&ctx, &mut state, &mut window, vec![]);
        dialog_frame(
            &ctx,
            &mut state,
            &mut window,
            vec![key_event(egui::Key::Enter)],
        );
        assert_eq!(window.jump, Some((id, 0..5)));
        // A popup consumes the first Escape; the second closes only the dialog.
        ctx.memory_mut(|m| m.open_popup(egui::Id::new("history-test")));
        dialog_frame(
            &ctx,
            &mut state,
            &mut window,
            vec![key_event(egui::Key::Escape)],
        );
        assert!(window.search.open);
        assert!(!ctx.memory(|m| m.any_popup_open()));
        dialog_frame(
            &ctx,
            &mut state,
            &mut window,
            vec![key_event(egui::Key::Escape)],
        );
        assert!(!window.search.open);
        assert_eq!(state.docs[&id].text, "été été");
    }
    #[test]
    fn compact_dialog_reports_invalid_regex_without_touching_document() {
        let (mut state, mut window, mut panel, id) = fixture("été");
        panel.open = true;
        panel.query.mode = Mode::Regex;
        panel.query.needle = "[".into();
        window.search = panel;
        let ctx = egui::Context::default();
        crate::theme::install(&ctx);
        dialog_frame(&ctx, &mut state, &mut window, vec![]);
        dialog_frame(
            &ctx,
            &mut state,
            &mut window,
            vec![key_event(egui::Key::Enter)],
        );
        assert!(window.search.message_error);
        assert!(!window.search.message.is_empty());
        assert_eq!(state.docs[&id].text, "été");
    }
}
