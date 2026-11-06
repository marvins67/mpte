use crate::document::Document;
use regex::{Regex, RegexBuilder};
use std::{
    ops::Range,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Normal,
    Extended,
    Regex,
}
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Query {
    pub needle: String,
    pub replacement: String,
    pub mode: Mode,
    pub case: bool,
    pub word: bool,
    pub dot_newline: bool,
}
pub struct Pattern {
    regex: Regex,
    replacement: String,
    expand: bool,
    word: bool,
}
#[derive(Clone, Debug)]
pub struct Hit {
    pub range: Range<usize>,
    pub replacement: String,
}

pub fn unescape(input: &str) -> Result<String, String> {
    let mut out = String::new();
    let mut chars = input.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next().ok_or("Échappement incomplet")? {
            'n' => out.push('\n'),
            'r' => out.push('\r'),
            't' => out.push('\t'),
            '0' => out.push('\0'),
            '\\' => out.push('\\'),
            kind @ ('x' | 'u') => {
                let count = if kind == 'x' { 2 } else { 4 };
                let digits: String = chars.by_ref().take(count).collect();
                if digits.len() != count || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
                    return Err("Échappement hexadécimal invalide".into());
                }
                let number = u32::from_str_radix(&digits, 16).map_err(|e| e.to_string())?;
                out.push(char::from_u32(number).ok_or("Caractère Unicode invalide")?);
            }
            other => return Err(format!("Échappement inconnu : \\{other}")),
        }
    }
    Ok(out)
}
impl Pattern {
    pub fn new(query: &Query) -> Result<Self, String> {
        if query.needle.is_empty() {
            return Err("Saisissez un texte à rechercher.".into());
        }
        let needle = if query.mode == Mode::Extended {
            unescape(&query.needle)?
        } else {
            query.needle.clone()
        };
        let source = if query.mode == Mode::Regex {
            needle
        } else {
            regex::escape(&needle)
        };
        let regex = RegexBuilder::new(&source)
            .case_insensitive(!query.case)
            .multi_line(true)
            .dot_matches_new_line(query.dot_newline)
            .size_limit(4 * 1024 * 1024)
            .build()
            .map_err(|e| e.to_string())?;
        let replacement = if query.mode == Mode::Extended {
            unescape(&query.replacement)?
        } else {
            query.replacement.clone()
        };
        Ok(Self {
            regex,
            replacement,
            expand: query.mode == Mode::Regex,
            word: query.word,
        })
    }
    pub fn hits(&self, text: &str, scope: Range<usize>) -> Vec<Hit> {
        let word = |c: char| c.is_alphanumeric() || c == '_';
        self.regex
            .captures_iter(text)
            .filter_map(|cap| {
                let m = cap.get(0)?;
                let range = m.range();
                if range.start < scope.start || range.end > scope.end {
                    return None;
                }
                if self.word
                    && (text[..range.start].chars().next_back().is_some_and(word)
                        || text[range.end..].chars().next().is_some_and(word))
                {
                    return None;
                }
                let replacement = if self.expand {
                    let mut s = String::new();
                    cap.expand(&self.replacement, &mut s);
                    s
                } else {
                    self.replacement.clone()
                };
                Some(Hit { range, replacement })
            })
            .collect()
    }
    pub fn replace_all(&self, text: &mut String, scope: Range<usize>) -> usize {
        let hits = self.hits(text, scope);
        let count = hits.len();
        for hit in hits.into_iter().rev() {
            text.replace_range(hit.range, &hit.replacement);
        }
        count
    }
}

#[derive(Clone)]
pub struct FileOptions {
    pub root: PathBuf,
    pub filters: String,
    pub recursive: bool,
    pub hidden: bool,
}
#[derive(Clone)]
pub struct FileHit {
    pub document_id: Option<u64>,
    pub matched: String,
    pub path: PathBuf,
    pub range: Range<usize>,
    pub line: usize,
    pub preview: String,
}
pub struct PlannedFile {
    pub document: Document,
    pub count: usize,
}
#[derive(Default)]
pub struct Report {
    pub hits: Vec<FileHit>,
    pub plan: Vec<PlannedFile>,
    pub count: usize,
    pub scanned: usize,
    pub skipped: usize,
    pub errors: Vec<String>,
    pub limited: bool,
}
pub fn scan(
    pattern: &Pattern,
    options: &FileOptions,
    replace: bool,
    cancel: &AtomicBool,
) -> Result<Report, String> {
    let root = options
        .root
        .canonicalize()
        .map_err(|e| format!("Dossier inaccessible : {e}"))?;
    if !root.is_dir() {
        return Err("Choisissez un dossier.".into());
    }
    let mut filters = globset::GlobSetBuilder::new();
    for filter in options
        .filters
        .split([';', ','])
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        filters.add(globset::Glob::new(filter).map_err(|e| e.to_string())?);
    }
    let filters = filters.build().map_err(|e| e.to_string())?;
    let mut report = Report::default();
    let mut planned_bytes = 0;
    let walker = walkdir::WalkDir::new(&root)
        .follow_links(false)
        .max_depth(if options.recursive { usize::MAX } else { 1 });
    for entry in walker.into_iter().filter_entry(|e| {
        options.hidden || e.depth() == 0 || !e.file_name().to_string_lossy().starts_with('.')
    }) {
        if cancel.load(Ordering::Relaxed) {
            return Err("Recherche annulée.".into());
        }
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                report.errors.push(e.to_string());
                continue;
            }
        };
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if !filters.is_empty()
            && !filters.is_match(entry.file_name())
            && !filters.is_match(path.strip_prefix(&root).unwrap_or(path))
        {
            continue;
        }
        if entry
            .metadata()
            .map(|m| m.len() > 16 * 1024 * 1024)
            .unwrap_or(true)
        {
            report.skipped += 1;
            continue;
        }
        let mut doc = match Document::open(path) {
            Ok(d) => d,
            Err(_) => {
                report.skipped += 1;
                continue;
            }
        };
        report.scanned += 1;
        let hits = pattern.hits(&doc.text, 0..doc.text.len());
        report.count += hits.len();
        for hit in &hits {
            if report.hits.len() >= 5000 {
                report.limited = true;
                break;
            }
            let line = doc.text[..hit.range.start]
                .bytes()
                .filter(|b| *b == b'\n')
                .count()
                + 1;
            let start = doc.text[..hit.range.start].rfind('\n').map_or(0, |i| i + 1);
            let end = doc.text[hit.range.start..]
                .find('\n')
                .map_or(doc.text.len(), |i| hit.range.start + i);
            report.hits.push(FileHit {
                document_id: None,
                matched: doc.text[hit.range.clone()].to_owned(),
                path: path.to_owned(),
                range: hit.range.clone(),
                line,
                preview: doc.text[start..end].chars().take(200).collect(),
            });
        }
        if replace && !hits.is_empty() {
            planned_bytes += doc.text.len();
            if planned_bytes > 100 * 1024 * 1024 {
                return Err(
                    "Le remplacement dépasse 100 Mio. Réduisez le dossier ou les filtres.".into(),
                );
            }
            for hit in hits.iter().rev() {
                doc.text.replace_range(hit.range.clone(), &hit.replacement);
            }
            report.plan.push(PlannedFile {
                document: doc,
                count: hits.len(),
            });
        }
    }
    Ok(report)
}
pub fn apply_plan(
    plan: Vec<PlannedFile>,
    excluded: &[PathBuf],
    cancel: &AtomicBool,
) -> (usize, Vec<String>) {
    let mut count = 0;
    let mut errors = vec![];
    for mut file in plan {
        if cancel.load(Ordering::Relaxed) {
            errors.push(
                "Remplacement interrompu : les fichiers déjà enregistrés restent modifiés.".into(),
            );
            break;
        }
        let path = file.document.path.clone().unwrap();
        if excluded.iter().any(|p| same_path(p, &path)) {
            errors.push(format!(
                "{} : fichier ouvert, ignoré. Utilisez le remplacement dans les documents ouverts.",
                path.display()
            ));
            continue;
        }
        match file.document.save(&path) {
            Ok(()) => count += file.count,
            Err(e) => errors.push(format!("{} : {e}", path.display())),
        }
    }
    (count, errors)
}
fn same_path(a: &Path, b: &Path) -> bool {
    a.canonicalize().unwrap_or_else(|_| a.to_owned())
        == b.canonicalize().unwrap_or_else(|_| b.to_owned())
}
pub fn cancel_token() -> Arc<AtomicBool> {
    Arc::new(AtomicBool::new(false))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unicode_case_words_and_selection() {
        let q = Query {
            needle: "été".into(),
            word: true,
            ..Default::default()
        };
        let p = Pattern::new(&q).unwrap();
        let s = "ÉTÉ été_long été";
        assert_eq!(p.hits(s, 0..s.len()).len(), 2);
        assert_eq!(p.hits(s, 5..s.len()).len(), 1);
    }
    #[test]
    fn extended_and_regex_replacement() {
        let p = Pattern::new(&Query {
            needle: r"\n\t".into(),
            replacement: r"\u00e9".into(),
            mode: Mode::Extended,
            ..Default::default()
        })
        .unwrap();
        let mut s = "a\n\tb".to_owned();
        assert_eq!(p.replace_all(&mut s, 0..4), 1);
        assert_eq!(s, "aéb");
        let p = Pattern::new(&Query {
            needle: r"(\w+)=(\d+)".into(),
            replacement: "$2:${1}".into(),
            mode: Mode::Regex,
            ..Default::default()
        })
        .unwrap();
        let mut s = "été=42".to_owned();
        let len = s.len();
        p.replace_all(&mut s, 0..len);
        assert_eq!(s, "42:été");
    }
    #[test]
    fn zero_width_and_invalid_patterns() {
        let p = Pattern::new(&Query {
            needle: "^".into(),
            replacement: ">".into(),
            mode: Mode::Regex,
            ..Default::default()
        })
        .unwrap();
        let mut s = "a\nb".to_owned();
        assert_eq!(p.replace_all(&mut s, 0..3), 2);
        assert_eq!(s, ">a\n>b");
        assert!(Pattern::new(&Query::default()).is_err());
        assert!(
            Pattern::new(&Query {
                needle: "[".into(),
                mode: Mode::Regex,
                ..Default::default()
            })
            .is_err()
        );
        assert!(unescape(r"\xZZ").is_err());
    }
    #[test]
    fn folder_filters_hidden_and_safe_replacement() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path();
        std::fs::create_dir(root.join("sub")).unwrap();
        std::fs::create_dir(root.join(".hidden")).unwrap();
        for name in ["a.json", "sub/b.json", ".hidden/c.json", "d.java"] {
            std::fs::write(root.join(name), "foo").unwrap();
        }
        let p = Pattern::new(&Query {
            needle: "foo".into(),
            replacement: "bar".into(),
            ..Default::default()
        })
        .unwrap();
        let o = FileOptions {
            root: root.into(),
            filters: "*.json".into(),
            recursive: true,
            hidden: false,
        };
        let report = scan(&p, &o, true, &AtomicBool::new(false)).unwrap();
        assert_eq!(report.count, 2);
        std::fs::write(root.join("a.json"), "external").unwrap();
        let (count, errors) = apply_plan(report.plan, &[], &AtomicBool::new(false));
        assert_eq!(count, 1);
        assert_eq!(errors.len(), 1);
        assert_eq!(
            std::fs::read_to_string(root.join("a.json")).unwrap(),
            "external"
        );
        assert_eq!(
            std::fs::read_to_string(root.join("sub/b.json")).unwrap(),
            "bar"
        );
        assert_eq!(
            std::fs::read_to_string(root.join(".hidden/c.json")).unwrap(),
            "foo"
        );
    }
    #[test]
    fn open_files_and_cancelled_operations_are_not_written() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.txt");
        std::fs::write(&path, "foo").unwrap();
        let p = Pattern::new(&Query {
            needle: "foo".into(),
            replacement: "bar".into(),
            ..Default::default()
        })
        .unwrap();
        let options = FileOptions {
            root: dir.path().into(),
            filters: "*.txt".into(),
            recursive: false,
            hidden: false,
        };
        let report = scan(&p, &options, true, &AtomicBool::new(false)).unwrap();
        let (count, errors) = apply_plan(
            report.plan,
            std::slice::from_ref(&path),
            &AtomicBool::new(false),
        );
        assert_eq!(count, 0);
        assert_eq!(errors.len(), 1);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "foo");
        assert!(scan(&p, &options, false, &AtomicBool::new(true)).is_err());
    }
}
