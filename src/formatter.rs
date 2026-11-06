use std::{
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
};

pub fn format(text: &str, extension: &str) -> Result<String, String> {
    if extension == "json" {
        // Validate duplicate keys before serde_json::Value can collapse them.
        let mut deserializer = serde_json::Deserializer::from_str(text);
        <UniqueKeys as serde::Deserialize>::deserialize(&mut deserializer)
            .map_err(|e| e.to_string())?;
        let value: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
        return serde_json::to_string_pretty(&value)
            .map(|s| s + "\n")
            .map_err(|e| e.to_string());
    }
    if !["xml", "html", "htm", "java"].contains(&extension) {
        return Err("Formatage disponible pour JSON, XML, HTML et Java.".into());
    }
    let tools = std::env::var_os("MPTE_FORMATTER_DIR")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::current_exe()
                .ok()
                .and_then(|p| p.parent()?.parent().map(|p| p.join("Resources/formatters")))
                .filter(|p| p.join("format.mjs").exists())
        })
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tools"));
    let bundled_node = tools.join("node/bin/node");
    let node = std::env::var_os("MPTE_NODE_BINARY").unwrap_or_else(|| {
        if bundled_node.exists() {
            bundled_node.into_os_string()
        } else {
            "node".into()
        }
    });
    let mut child = Command::new(node)
        .arg(tools.join("format.mjs"))
        .arg(extension)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            format!("Formateur indisponible : {e}. Installez Node.js et lancez npm ci dans tools/.")
        })?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or("Entrée du formateur indisponible")?;
    let input = text.as_bytes().to_vec();
    // Write in parallel with output collection to avoid pipe-capacity deadlocks on large files.
    let writer = std::thread::spawn(move || stdin.write_all(&input));
    let output = child.wait_with_output().map_err(|e| e.to_string())?;
    let written = writer.join().map_err(|_| "Échec du formateur")?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
    }
    written.map_err(|e| e.to_string())?;
    String::from_utf8(output.stdout).map_err(|e| e.to_string())
}

struct UniqueKeys;
impl<'de> serde::Deserialize<'de> for UniqueKeys {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = UniqueKeys;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("une valeur JSON")
            }
            fn visit_bool<E: serde::de::Error>(self, _: bool) -> Result<UniqueKeys, E> {
                Ok(UniqueKeys)
            }
            fn visit_i64<E: serde::de::Error>(self, _: i64) -> Result<UniqueKeys, E> {
                Ok(UniqueKeys)
            }
            fn visit_u64<E: serde::de::Error>(self, _: u64) -> Result<UniqueKeys, E> {
                Ok(UniqueKeys)
            }
            fn visit_f64<E: serde::de::Error>(self, _: f64) -> Result<UniqueKeys, E> {
                Ok(UniqueKeys)
            }
            fn visit_str<E: serde::de::Error>(self, _: &str) -> Result<UniqueKeys, E> {
                Ok(UniqueKeys)
            }
            fn visit_unit<E: serde::de::Error>(self) -> Result<UniqueKeys, E> {
                Ok(UniqueKeys)
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> Result<UniqueKeys, A::Error> {
                while seq.next_element::<UniqueKeys>()?.is_some() {}
                Ok(UniqueKeys)
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<UniqueKeys, A::Error> {
                let mut keys = std::collections::HashSet::new();
                while let Some(key) = map.next_key::<String>()? {
                    if !keys.insert(key.clone()) {
                        return Err(serde::de::Error::custom(format!(
                            "Clé JSON dupliquée : {key}. Le formatage est annulé pour préserver les données."
                        )));
                    }
                    map.next_value::<UniqueKeys>()?;
                }
                Ok(UniqueKeys)
            }
        }
        d.deserialize_any(Visitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn json_preserves_key_order_and_is_idempotent() {
        let result = format(r#"{"z":1,"a":[true,"été"]}"#, "json").unwrap();
        assert!(result.find('z').unwrap() < result.find('a').unwrap());
        assert!(result.contains("été"));
        assert_eq!(format(&result, "json").unwrap(), result);
    }
    #[test]
    fn invalid_json_is_an_error() {
        assert!(format("{", "json").is_err());
    }
    #[test]
    fn duplicate_json_keys_are_rejected_and_large_numbers_preserved() {
        assert!(format(r#"{"nested":[{"a":1,"a":2}]}"#, "json").is_err());
        let number = "123456789012345678901234567890";
        assert_eq!(format(number, "json").unwrap().trim(), number);
    }
    #[test]
    fn java_formatter_runs_from_rust() {
        let result = format("class Example{int value=1;}", "java").unwrap();
        assert!(result.contains("int value = 1;"));
    }
}
