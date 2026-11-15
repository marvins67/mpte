use std::{
    io::{Read, Write},
    path::PathBuf,
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
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
    let mut command = Command::new(node);
    command.arg(tools.join("format.mjs")).arg(extension);
    run_formatter(
        command,
        text.as_bytes().to_vec(),
        Duration::from_secs(60),
        64 * 1024 * 1024,
    )
}

fn read_limited(
    mut reader: impl Read,
    limit: usize,
    exceeded: Arc<AtomicBool>,
) -> Result<Vec<u8>, String> {
    let mut data = Vec::new();
    let mut buffer = [0; 8192];
    loop {
        let count = reader.read(&mut buffer).map_err(|e| e.to_string())?;
        if count == 0 {
            return Ok(data);
        }
        if data.len().saturating_add(count) > limit {
            exceeded.store(true, Ordering::Relaxed);
            return Err("La sortie du formateur dépasse la limite mémoire.".into());
        }
        data.extend_from_slice(&buffer[..count]);
    }
}

fn run_formatter(
    mut command: Command,
    input: Vec<u8>,
    timeout: Duration,
    output_limit: usize,
) -> Result<String, String> {
    let mut child = command
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
    let stdout = child
        .stdout
        .take()
        .ok_or("Sortie du formateur indisponible")?;
    let stderr = child
        .stderr
        .take()
        .ok_or("Erreur du formateur indisponible")?;
    let exceeded = Arc::new(AtomicBool::new(false));
    let stdout_exceeded = exceeded.clone();
    let stderr_exceeded = exceeded.clone();
    let writer = std::thread::spawn(move || stdin.write_all(&input));
    let output_reader =
        std::thread::spawn(move || read_limited(stdout, output_limit, stdout_exceeded));
    let error_reader =
        std::thread::spawn(move || read_limited(stderr, 1024 * 1024, stderr_exceeded));
    let started = Instant::now();
    let status = loop {
        if exceeded.load(Ordering::Relaxed) || started.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            break Err(if exceeded.load(Ordering::Relaxed) {
                "La sortie du formateur dépasse la limite mémoire.".to_owned()
            } else {
                "Le formateur a dépassé le délai maximal de 60 secondes.".to_owned()
            });
        }
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) => std::thread::sleep(
                Duration::from_millis(100).min(timeout.saturating_sub(started.elapsed())),
            ),
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                break Err(error.to_string());
            }
        }
    };
    let written = writer.join().map_err(|_| "Échec du formateur")?;
    let output = output_reader
        .join()
        .map_err(|_| "Échec de lecture du formateur")?;
    let errors = error_reader
        .join()
        .map_err(|_| "Échec de lecture du formateur")?;
    let status = status?;
    let output = output?;
    let errors = errors?;
    if !status.success() {
        return Err(String::from_utf8_lossy(&errors).trim().to_owned());
    }
    written.map_err(|e| e.to_string())?;
    String::from_utf8(output).map_err(|e| e.to_string())
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
    #[cfg(unix)]
    #[test]
    fn external_formatter_timeout_and_output_limit_terminate_the_child() {
        let mut slow = Command::new("sh");
        slow.args(["-c", "exec sleep 5"]);
        let started = Instant::now();
        assert!(
            run_formatter(slow, vec![], Duration::from_millis(50), 1024)
                .unwrap_err()
                .contains("délai")
        );
        assert!(started.elapsed() < Duration::from_secs(3));
        let mut noisy = Command::new("sh");
        noisy.args(["-c", "printf 1234567890"]);
        assert!(
            run_formatter(noisy, vec![], Duration::from_secs(2), 4)
                .unwrap_err()
                .contains("mémoire")
        );
    }

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
