use std::time::Duration;

use crate::catalog::{Entry, Origin};
use crate::download::{decode_entities, encode_segment, fetch_text};

/// The Mod Archive's guest search page; results are plain HTML table rows.
pub const SEARCH_URL: &str =
    "https://modarchive.org/index.php?request=search&search_type=filename_or_songtitle";

const MAX_RESULTS: usize = 40;

/// Search file names and titles, then scrape the result rows.
pub fn search(query: &str) -> Result<Vec<Entry>, String> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }
    let url = format!("{SEARCH_URL}&query=*{}*&submit=Find", encode_segment(query));
    let html = fetch_text(&url, Duration::from_secs(25))?;
    Ok(parse(&html))
}

/// Pull `moduleid`, title and file name out of each result link:
/// `<a class="standard-link" href="index.php?request=view_by_moduleid&amp;query=42"
/// title="Song">artist_-_song.xm</a>`.
fn parse(html: &str) -> Vec<Entry> {
    let mut entries: Vec<Entry> = Vec::new();
    let mut cursor = 0usize;
    while let Some(found) = html[cursor..].find("view_by_moduleid") {
        let link = cursor + found;
        cursor = link + "view_by_moduleid".len();
        let tail = &html[cursor..];

        let Some(query_offset) = tail.find("query=") else {
            continue;
        };
        if query_offset > 16 {
            continue;
        }
        let after_query = &tail[query_offset + "query=".len()..];
        let digits: String = after_query
            .chars()
            .take_while(|ch| ch.is_ascii_digit())
            .collect();
        let Ok(id) = digits.parse::<u32>() else {
            continue;
        };
        if entries.iter().any(|entry| entry.id == id) {
            continue;
        }

        let Some(title_offset) = after_query.find("title=\"") else {
            continue;
        };
        if title_offset > 240 {
            continue;
        }
        let title_rest = &after_query[title_offset + "title=\"".len()..];
        let Some(title_end) = title_rest.find("\">") else {
            continue;
        };
        let title = decode_entities(title_rest[..title_end].trim());
        let file_rest = &title_rest[title_end + 2..];
        let Some(file_end) = file_rest.find("</a>") else {
            continue;
        };
        let file = decode_entities(file_rest[..file_end].trim());
        if file.is_empty() {
            continue;
        }
        let ext = file
            .rsplit_once('.')
            .map(|(_, ext)| ext.to_ascii_lowercase())
            .unwrap_or_default();
        if ext.is_empty() {
            continue;
        }

        let name = if title.is_empty() {
            file.clone()
        } else {
            title
        };
        let lower = format!("{name} {file} the mod archive modarchive").to_lowercase();
        entries.push(Entry {
            path: format!("modarchive/{id}/{file}"),
            size: 0,
            ext,
            artist: String::new(),
            name,
            origin: Origin::ModArchive,
            id,
            download_url: Some(format!(
                "https://api.modarchive.org/downloads.php?moduleid={id}"
            )),
            lower,
        });
        if entries.len() >= MAX_RESULTS {
            break;
        }
    }
    entries
}

