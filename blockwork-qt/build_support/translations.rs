use serde::Deserialize;
use std::collections::BTreeSet;
use std::path::Path;

#[derive(Default, Deserialize)]
struct Catalog {
    #[serde(rename = "context", default)]
    contexts: Vec<Context>,
}

#[derive(Deserialize)]
struct Context {
    name: String,
    #[serde(rename = "message", default)]
    messages: Vec<Message>,
}

#[derive(Deserialize)]
struct Message {
    source: String,
    #[serde(default)]
    comment: String,
    translation: Option<Translation>,
}

#[derive(Deserialize)]
struct Translation {
    #[serde(rename = "@type", default)]
    kind: String,
    #[serde(rename = "$text", default)]
    text: String,
    #[serde(rename = "numerusform", default)]
    forms: Vec<String>,
}

fn messages(xml: &str) -> (BTreeSet<(String, String, String)>, usize) {
    let catalog: Catalog = quick_xml::de::from_str(xml).expect("valid Qt translation catalog");
    let mut keys = BTreeSet::new();
    let mut finished = 0;
    for context in catalog.contexts {
        for message in context.messages {
            let translation = message.translation;
            if translation
                .as_ref()
                .is_some_and(|t| matches!(t.kind.as_str(), "obsolete" | "vanished"))
            {
                continue;
            }
            if !keys.insert((context.name.clone(), message.source, message.comment)) {
                continue;
            }
            if let Some(t) = translation {
                if t.kind.is_empty()
                    && if t.forms.is_empty() {
                        !t.text.trim().is_empty()
                    } else {
                        t.forms.iter().all(|form| !form.trim().is_empty())
                    }
                {
                    finished += 1;
                }
            }
        }
    }
    (keys, finished)
}

pub fn metadata(directory: &Path) -> serde_json::Value {
    let mut catalogs = Vec::new();
    let mut all_keys = BTreeSet::new();
    for entry in std::fs::read_dir(directory).expect("translation directory") {
        let path = entry.unwrap().path();
        let name = path.file_stem().unwrap().to_string_lossy();
        let Some(code) = name.strip_prefix("qml_") else {
            continue;
        };
        if path.extension().is_none_or(|ext| ext != "ts") || !path.with_extension("qm").exists() {
            continue;
        }
        let (keys, translated) =
            messages(&std::fs::read_to_string(&path).expect("read translation catalog"));
        all_keys.extend(keys);
        catalogs.push((code.to_owned(), translated));
    }
    catalogs.sort();
    serde_json::Value::Array(
        catalogs
            .into_iter()
            .map(|(code, translated)| {
                // English is the source language; its catalog only contains plurals.
                let translated = if code == "en" {
                    all_keys.len()
                } else {
                    translated
                };
                serde_json::json!({ "id": code, "translated": translated, "total": all_keys.len() })
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_only_finished_active_messages_and_complete_plurals() {
        let xml = r#"<TS><context><name>Test</name>
            <message><source>Done</source><translation>Fertig</translation></message>
            <message><source>Draft</source><translation type="unfinished">Entwurf</translation></message>
            <message><source>Blank</source><translation>  </translation></message>
            <message><source>Missing</source></message>
            <message><source>Old</source><translation type="vanished">Alt</translation></message>
            <message><source>Plural</source><translation><numerusform>One</numerusform><numerusform>Many</numerusform></translation></message>
            <message><source>Partial plural</source><translation><numerusform>One</numerusform><numerusform></numerusform></translation></message>
            <message><source>Done</source><translation>Fertig</translation></message>
        </context></TS>"#;
        let (keys, finished) = messages(xml);
        assert_eq!(keys.len(), 6);
        assert_eq!(finished, 2);
    }

    #[test]
    fn source_english_is_complete_and_all_bundled_catalogs_are_listed() {
        let data = metadata(Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/i18n")));
        let entries = data.as_array().unwrap();
        assert_eq!(entries.len(), 31);
        let english = entries.iter().find(|e| e["id"] == "en").unwrap();
        assert_eq!(english["translated"], english["total"]);
        assert!(english["total"].as_u64().unwrap() > 500);
        assert!(
            entries
                .iter()
                .all(|e| e["translated"].as_u64() <= e["total"].as_u64())
        );
    }
}
