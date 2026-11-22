use std::time::Duration;

use crate::catalog::{is_module_extension, Entry, Origin};
use crate::download::{decode_entities, encode_segment, fetch_text, sanitize_segment};

/// Amiga Music Preservation, rebuilt in 2026: `/search` redirects to the
/// `/modules` listing, where every hit links to `/module/<id>`.
pub const SITE: &str = "https://amp.dascene.net";

const MAX_RESULTS: usize = 40;

/// Search the module database and scrape the result rows.
pub fn search(query: &str) -> Result<Vec<Entry>, String> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }
    let url = format!("{SITE}/search?type=modules&q={}", encode_segment(query));
    let html = fetch_text(&url, Duration::from_secs(25))?;
    Ok(parse(&html))
}

/// Each row carries the title, the composer link, a format badge and a
/// `/module/<id>` download link.
fn parse(html: &str) -> Vec<Entry> {
    let mut entries: Vec<Entry> = Vec::new();
    let mut rest = html;
    while let Some(found) = rest.find("fg-mod-entry") {
        rest = &rest[found + "fg-mod-entry".len()..];
        let Some(link_at) = rest.find("/module/") else {
            continue;
        };
        let block = &rest[..link_at];
        let tail = &rest[link_at + "/module/".len()..];
        let digits: String = tail.chars().take_while(|ch| ch.is_ascii_digit()).collect();
        let Ok(id) = digits.parse::<u32>() else {
            continue;
        };
        if id == 0 || entries.iter().any(|entry| entry.id == id) {
            continue;
        }

        let name = text_after(block, "fg-mod-title").unwrap_or_default();
        let artist = text_after(block, "fg-link fg-caption").unwrap_or_default();
        let ext = text_after(block, "q-badge")
            .unwrap_or_default()
            .to_ascii_lowercase();
        if name.is_empty() || !is_module_extension(&ext) {
            continue;
        }

        let file_name = format!("{}.{ext}", sanitize_segment(&name));
        let lower = format!("{name} {artist} {file_name} amiga music preservation amp").to_lowercase();
        entries.push(Entry {
            path: format!("amp/{id}/{file_name}"),
            size: 0,
            ext,
            artist,
            name,
            origin: Origin::Amiga,
            id,
            download_url: Some(format!("{SITE}/module/{id}?download=1")),
            lower,
        });
        if entries.len() >= MAX_RESULTS {
            break;
        }
    }
    entries
}

/// Read the text of the element whose class or attribute contains `marker`.
fn text_after(block: &str, marker: &str) -> Option<String> {
    let at = block.find(marker)?;
    let tail = &block[at..];
    let open_end = tail.find('>')?;
    let after = &tail[open_end + 1..];
    let close = after.find('<')?;
    let text = after[..close].trim();
    if text.is_empty() {
        None
    } else {
        Some(decode_entities(text))
    }
}
