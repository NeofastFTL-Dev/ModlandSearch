use std::path::Path;

use serde::Deserialize;

use crate::catalog::{Entry, Origin};
use crate::download::encode_segment;

/// Keygen Music serves modules under the same path stored in kgm-lib.txt,
/// with the leading `/kgm` prefix removed and every segment percent-encoded.
pub const DOWNLOAD_BASE: &str = "https://keygen.music/music";

#[derive(Deserialize)]
struct RawEntry {
    #[serde(default)]
    st: String,
    #[serde(default)]
    rg: String,
    #[serde(default)]
    sn: String,
    #[serde(default)]
    path: String,
}

/// Parse the cached `kgm-lib.txt`: a JSON array of
/// `{st, rg, sn, n, mdt, path}` records.
pub fn parse(text: &str) -> Result<Vec<Entry>, String> {
    let raw: Vec<RawEntry> =
        serde_json::from_str(text).map_err(|error| format!("invalid kgm-lib.txt: {error}"))?;
    let mut entries = Vec::with_capacity(raw.len());
    for item in raw {
        // Keep the `kgm/` prefix: it marks the archive on every path the app
        // shows, and kgm::url() strips it again when building the download URL.
        let path = item.path.trim_start_matches('/');
        if path.is_empty() {
            continue;
        }

        let file_name = path.rsplit('/').next().unwrap_or(path);
        let ext = file_name
            .rsplit_once('.')
            .map(|(_, ext)| ext.to_ascii_lowercase())
            .unwrap_or_default();
        let name = if item.sn.trim().is_empty() {
            item.st.clone()
        } else {
            item.sn.clone()
        };
        let artist = item.rg.clone();
        let lower = format!("{} {} {} {}", path, item.st, artist, name).to_lowercase();

        entries.push(Entry {
            path: path.to_string(),
            size: 0,
            ext,
            artist,
            name,
            origin: Origin::Keygen,
            id: 0,
            download_url: None,
            lower,
        });
    }
    Ok(entries)
}

pub fn load_cached(path: &Path) -> Option<Vec<Entry>> {
    let text = std::fs::read_to_string(path).ok()?;
    if text.trim().is_empty() {
        return None;
    }
    parse(&text).ok()
}

/// Read the cached library version stamp (milliseconds since the epoch).
pub fn cached_version() -> Option<u64> {
    std::fs::read_to_string(crate::paths::kgm_ver_path())
        .ok()?
        .trim()
        .parse()
        .ok()
}

/// Build the playback/download URL for a library path such as
/// `!Others/0BiT - Fax Spider 2.1 kg.xm`.
pub fn url(path: &str) -> String {
    let path = path.trim_start_matches('/');
    let path = path.strip_prefix("kgm/").unwrap_or(path);
    let encoded: Vec<String> = path.split('/').map(encode_segment).collect();
    format!("{DOWNLOAD_BASE}/{}", encoded.join("/"))
}
