use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub struct Document {
    pub path: Option<PathBuf>,
    pub text: String,
    pub saved: String,
    disk: Option<Vec<u8>>,
    bom: bool,
    crlf: bool,
    pub format_on_save: bool,
    pub revision: u64,
    pub view: crate::outline::View,
    pub column: crate::column::State,
    pub partition: Option<crate::partition::Partition>,
    saved_partition: Option<crate::partition::Data>,
    partition_disk: Option<Vec<u8>>,
    pub partition_warning: Option<String>,
    pub partition_pages: usize,
    pub partition_overflow: usize,
}

impl Document {
    pub fn new() -> Self {
        Self {
            path: None,
            text: String::new(),
            saved: String::new(),
            disk: None,
            bom: false,
            crlf: false,
            format_on_save: false,
            revision: 0,
            view: crate::outline::View::default(),
            column: crate::column::State::default(),
            partition: None,
            saved_partition: None,
            partition_disk: None,
            partition_warning: None,
            partition_pages: 1,
            partition_overflow: 0,
        }
    }
    pub fn open(path: &Path) -> Result<Self, String> {
        let bytes = fs::read(path).map_err(|e| e.to_string())?;
        let bom = bytes.starts_with(&[0xef, 0xbb, 0xbf]);
        let raw = std::str::from_utf8(if bom { &bytes[3..] } else { &bytes }).map_err(|_| {
            "Ce fichier n’est pas en UTF-8. Convertissez son encodage avant de l’ouvrir.".to_owned()
        })?;
        if raw.contains('\0') {
            return Err("Ce fichier semble être binaire.".into());
        }
        let crlf = raw.contains("\r\n");
        let text = raw.replace("\r\n", "\n");
        let partition_disk = match fs::read(partition_path(path)) {
            Ok(bytes) => Some(bytes),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(format!("Lecture du fichier compagnon Partition : {e}")),
        };
        let mut partition_warning = None;
        let partition = partition_disk.as_ref().and_then(|bytes| {
            match serde_json::from_slice::<crate::partition::Data>(bytes)
                .map_err(|e| format!("Fichier compagnon Partition illisible : {e}"))
                .and_then(|data| crate::partition::Partition::from_data(data, &text))
            {
                Ok(partition) => Some(partition),
                Err(e) => {
                    partition_warning = Some(e);
                    None
                }
            }
        });
        let saved_partition = partition.as_ref().map(|p| p.data());
        Ok(Self {
            path: Some(path.to_owned()),
            saved: text.clone(),
            text,
            disk: Some(bytes),
            bom,
            crlf,
            format_on_save: false,
            revision: 0,
            view: crate::outline::View::default(),
            column: crate::column::State::default(),
            partition,
            saved_partition,
            partition_disk,
            partition_warning,
            partition_pages: 1,
            partition_overflow: 0,
        })
    }
    pub fn dirty(&self) -> bool {
        self.text != self.saved || self.partition.as_ref().map(|p| p.data()) != self.saved_partition
    }
    pub fn partition_active(&self) -> bool {
        self.partition.as_ref().is_some_and(|p| p.active)
    }
    pub fn toggle_partition(&mut self) {
        self.column.clear();
        self.view.collapsed.clear();
        if let Some(partition) = &mut self.partition {
            partition.sync(&self.text, 0..0);
            partition.active = !partition.active;
        } else {
            self.partition = Some(crate::partition::Partition::new(&self.text));
        }
    }
    pub fn title(&self) -> String {
        self.path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Sans titre".into())
    }
    pub fn extension(&self) -> String {
        self.path
            .as_ref()
            .and_then(|p| p.extension())
            .and_then(|s| s.to_str())
            .unwrap_or("txt")
            .to_ascii_lowercase()
    }
    pub fn save(&mut self, path: &Path) -> Result<(), String> {
        if let Some(partition) = &mut self.partition {
            partition.sync(&self.text, 0..0);
        }
        let partition_data = self.partition.as_ref().map(|p| p.data());
        let companion = partition_path(path);
        let mut partition_temp = if let Some(data) = &partition_data {
            let current = match fs::read(&companion) {
                Ok(bytes) => Some(bytes),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                Err(e) => return Err(format!("Vérification du fichier compagnon Partition : {e}")),
            };
            let expected = if self.path.as_deref() == Some(path) {
                self.partition_disk.as_ref()
            } else {
                None
            };
            if current.as_ref() != expected {
                return Err("Le fichier compagnon Partition a changé sur disque ou existe déjà. Utilisez Enregistrer sous pour conserver les deux versions.".into());
            }
            let bytes = serde_json::to_vec(data).map_err(|e| e.to_string())?;
            let parent = companion
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(Path::new("."));
            let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
            if let Ok(meta) = fs::metadata(&companion) {
                temp.as_file()
                    .set_permissions(meta.permissions())
                    .map_err(|e| e.to_string())?;
            }
            temp.write_all(&bytes)
                .and_then(|_| temp.as_file().sync_all())
                .map_err(|e| e.to_string())?;
            Some((temp, bytes))
        } else {
            None
        };
        if self.path.as_deref() == Some(path) && self.disk.is_some() {
            let current = fs::read(path).map_err(|e| {
                format!("Vérification du fichier : {e}. Utilisez Enregistrer sous.")
            })?;
            if Some(&current) != self.disk.as_ref() {
                return Err("Le fichier a changé sur disque. Utilisez Enregistrer sous pour conserver les deux versions.".into());
            }
        }
        let mut bytes = if self.bom {
            vec![0xef, 0xbb, 0xbf]
        } else {
            Vec::new()
        };
        let text = if self.crlf {
            self.text.replace('\n', "\r\n")
        } else {
            self.text.clone()
        };
        bytes.extend(text.as_bytes());
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
        if let Ok(meta) = fs::metadata(path) {
            temp.as_file()
                .set_permissions(meta.permissions())
                .map_err(|e| e.to_string())?;
        }
        temp.write_all(&bytes)
            .and_then(|_| temp.as_file().sync_all())
            .map_err(|e| e.to_string())?;
        temp.persist(path).map_err(|e| e.to_string())?;
        if self.path.as_deref() != Some(path) {
            self.partition_disk = None;
        }
        self.path = Some(path.to_owned());
        self.disk = Some(bytes);
        if let Some((temp, bytes)) = partition_temp.take() {
            temp.persist(&companion).map_err(|e| format!("Le texte est enregistré, mais pas ses tailles Partition : {e}. Réessayez l’enregistrement."))?;
            self.partition_disk = Some(bytes);
        }
        self.saved.clone_from(&self.text);
        self.saved_partition = partition_data;
        Ok(())
    }
}

fn partition_path(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(".mpte-partition.json");
    PathBuf::from(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn partition_sizes_roundtrip_without_changing_plain_text_or_line_endings() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("chant.txt");
        let original = "\u{feff}Café 🦀\r\nAm      G\r\n";
        fs::write(&path, original).unwrap();
        let mut doc = Document::open(&path).unwrap();
        doc.toggle_partition();
        doc.partition.as_mut().unwrap().resize(0..5, 6., false);
        assert!(doc.dirty());
        doc.save(&path).unwrap();
        assert!(!doc.dirty());
        assert_eq!(fs::read_to_string(&path).unwrap(), original);
        let mut reopened = Document::open(&path).unwrap();
        assert!(reopened.partition_active());
        assert_eq!(
            reopened.partition.as_ref().unwrap().selection_size(0..5),
            "20 pt"
        );
        assert!(!reopened.dirty());
        reopened.toggle_partition();
        reopened.save(&path).unwrap();
        let mut normal = Document::open(&path).unwrap();
        assert!(!normal.partition_active());
        normal.toggle_partition();
        assert_eq!(
            normal.partition.as_ref().unwrap().selection_size(0..5),
            "20 pt"
        );
        let copy = dir.path().join("copie.txt");
        normal.save(&copy).unwrap();
        assert!(partition_path(&copy).exists());
        assert_eq!(
            Document::open(&copy)
                .unwrap()
                .partition
                .as_ref()
                .unwrap()
                .selection_size(0..5),
            "20 pt"
        );
    }

    #[test]
    fn partition_companion_conflicts_are_reported_before_writing_text() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("chant.txt");
        let mut doc = Document::new();
        doc.text = "Am".into();
        doc.toggle_partition();
        doc.save(&path).unwrap();
        fs::write(partition_path(&path), "external").unwrap();
        doc.text = "G".into();
        assert!(doc.save(&path).unwrap_err().contains("compagnon"));
        assert_eq!(fs::read_to_string(&path).unwrap(), "Am");
        assert_eq!(
            fs::read_to_string(partition_path(&path)).unwrap(),
            "external"
        );
        let reopened = Document::open(&path).unwrap();
        assert_eq!(reopened.text, "Am");
        assert!(reopened.partition_warning.is_some());
        assert!(!reopened.partition_active());
    }

    #[test]
    fn stale_companion_does_not_apply_sizes_to_externally_edited_text() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("chant.txt");
        let mut doc = Document::new();
        doc.text = "Am".into();
        doc.toggle_partition();
        doc.save(&path).unwrap();
        fs::write(&path, "Café").unwrap();
        let reopened = Document::open(&path).unwrap();
        assert_eq!(reopened.text, "Café");
        assert!(reopened.partition.is_none());
        assert!(reopened.partition_warning.is_some());
    }

    #[test]
    fn save_roundtrip_preserves_bom_crlf_and_unicode() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.txt");
        fs::write(&path, b"\xef\xbb\xbfhello\r\n").unwrap();
        let mut doc = Document::open(&path).unwrap();
        doc.text.push_str("été 🦀\n");
        assert!(doc.dirty());
        doc.save(&path).unwrap();
        assert!(!doc.dirty());
        assert_eq!(
            fs::read(&path).unwrap(),
            "\u{feff}hello\r\nété 🦀\r\n".as_bytes()
        );
    }
    #[test]
    fn external_changes_are_not_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.json");
        fs::write(&path, "original").unwrap();
        let mut doc = Document::open(&path).unwrap();
        doc.text = "local".into();
        fs::write(&path, "external").unwrap();
        assert!(doc.save(&path).is_err());
        assert_eq!(fs::read_to_string(path).unwrap(), "external");
        assert!(doc.dirty());
    }
    #[test]
    fn refuses_binary_and_invalid_utf8() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("binary");
        for bytes in [&[0xff][..], &[0, 1][..]] {
            fs::write(&path, bytes).unwrap();
            assert!(Document::open(&path).is_err());
        }
    }
}
