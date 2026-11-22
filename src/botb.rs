use std::time::Duration;

use serde_json::Value;

use crate::catalog::{is_module_extension, Entry, Origin};
use crate::download::{encode_segment, fetch_text};

/// Public BotB API v1: `/{object_type}/{command}`.
pub const SEARCH_URL: &str = "https://battleofthebits.com/api/v1/entry/search";

/// Search BotB entry titles and return the tracker modules among the hits.
pub fn search(query: &str) -> Result<Vec<Entry>, String> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }
    let url = format!("{SEARCH_URL}/{}/0/50", encode_segment(query));
    let body = fetch_text(&url, Duration::from_secs(25))?;
    let value: Value = serde_json::from_str(&body)
        .map_err(|error| format!("BotB returned invalid JSON: {error}"))?;
    if let Some(error) = value.get("error").and_then(Value::as_str) {
        return Err(format!("BotB error: {error}"));
    }
    let list = value
        .as_array()
        .ok_or_else(|| "BotB returned an unexpected response".to_string())?;

    let mut entries: Vec<Entry> = Vec::new();
    for item in list {
        let Some(id) = field_u32(item, "id") else {
            continue;
        };
        if entries.iter().any(|entry| entry.id == id) {
            continue;
        }

        let title = text_field(item, "title");
        let artist = text_field(item, "authors_display");
        let file = item
            .get("view_url")
            .and_then(Value::as_str)
            .unwrap_or("")
            .rsplit('/')
            .next()
            .unwrap_or("")
            .trim()
            .to_string();
        let ext = file
            .rsplit_once('.')
            .map(|(_, ext)| ext.to_ascii_lowercase())
            .unwrap_or_default();
        if !is_module_extension(&ext) {
            continue;
        }

        let name = if title.is_empty() {
            file.clone()
        } else {
            title
        };
        let lower = format!("{name} {artist} {file} battle of the bits botb").to_lowercase();
        entries.push(Entry {
            path: format!("botb/{id}/{file}"),
            size: 0,
            ext,
            artist,
            name,
            origin: Origin::Botb,
            id,
            download_url: Some(format!(
                "https://battleofthebits.com/player/EntryDonload/{id}/"
            )),
            lower,
        });
    }
    Ok(entries)
}

fn text_field(item: &Value, key: &str) -> String {
    item.get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string()
}

/// BotB hands ids over as strings; accept numbers as well.
fn field_u32(item: &Value, key: &str) -> Option<u32> {
    let value = item.get(key)?;
    if let Some(number) = value.as_u64() {
        return Some(number as u32);
    }
    value.as_str()?.trim().parse().ok()
}
