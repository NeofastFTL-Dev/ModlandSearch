use std::time::Duration;

use serde_json::Value;

use crate::catalog::{is_module_extension, Entry, Origin};
use crate::download::{encode_segment, fetch_text};

/// scene.org's public file archive API; documented at https://files.scene.org/api
pub const SEARCH_URL: &str = "https://files.scene.org/api/search/";

const MAX_RESULTS: usize = 40;

/// Search the whole archive and keep the tracker modules among the hits.
pub fn search(query: &str) -> Result<Vec<Entry>, String> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }
    let url = format!("{SEARCH_URL}?q={}", encode_segment(query));
    let body = fetch_text(&url, Duration::from_secs(25))?;
    let value: Value = serde_json::from_str(&body)
        .map_err(|error| format!("scene.org returned invalid JSON: {error}"))?;
    if value.get("success").and_then(Value::as_bool) != Some(true) {
        return Err("scene.org refused the search".to_string());
    }
    let files = value
        .get("files")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let mut entries: Vec<Entry> = Vec::new();
    for file in files {
        let Some(file_name) = file.get("filename").and_then(Value::as_str) else {
            continue;
        };
        let Some(full_path) = file.get("fullPath").and_then(Value::as_str) else {
            continue;
        };
        let Some(get_url) = file.get("getURL").and_then(Value::as_str) else {
            continue;
        };
        let (stem, ext) = match file_name.rsplit_once('.') {
            Some((stem, ext)) => (stem, ext.to_ascii_lowercase()),
            None => continue,
        };
        if !is_module_extension(&ext) {
            continue;
        }

        let path = format!("sceneorg{}", full_path.trim_end_matches('/'));
        if entries.iter().any(|entry| entry.path == path) {
            continue;
        }
        let parent = full_path
            .trim_end_matches('/')
            .rsplit_once('/')
            .map(|(parent, _)| parent)
            .unwrap_or("");
        let artist = parent.rsplit('/').next().unwrap_or(parent).to_string();
        let size = file.get("size").and_then(Value::as_u64).unwrap_or(0);
        let lower = format!("{stem} {artist} {file_name} scene.org sceneorg").to_lowercase();

        entries.push(Entry {
            path,
            size,
            ext,
            artist,
            name: stem.to_string(),
            origin: Origin::SceneOrg,
            id: 0,
            download_url: Some(get_url.to_string()),
            lower,
        });
        if entries.len() >= MAX_RESULTS {
            break;
        }
    }
    Ok(entries)
}
