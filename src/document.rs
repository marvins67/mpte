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
        })
    }
    pub fn dirty(&self) -> bool {
        self.text != self.saved
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
        self.path = Some(path.to_owned());
        self.saved.clone_from(&self.text);
        self.disk = Some(bytes);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
