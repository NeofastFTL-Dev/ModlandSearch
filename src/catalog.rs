use std::fs;
use std::io::Read;
use std::path::Path;

use crate::download::fetch;

pub const ALLMODS_URL: &str = "https://modland.com/allmods.zip";

/// Every format libopenmpt can decode. The live archives also host graphics,
/// mp3s and zip packages, so their hits are filtered against this list.
pub const MODULE_EXTENSIONS: &[&str] = &[
    "669", "amf", "dbm", "digi", "dmf", "dsm", "dsym", "far", "gdm", "imf", "it", "j2b", "m15",
    "mdl", "med", "mod", "mo3", "mt2", "mtm", "mptm", "okt", "okta", "psm", "pt36", "ptm", "s3m",
    "sfx", "sfx2", "st26", "stm", "stp", "stx", "ult", "umx", "wow", "xm", "xmf",
];

pub fn is_module_extension(ext: &str) -> bool {
    MODULE_EXTENSIONS.contains(&ext)
}

/// Where an entry comes from. Every origin has its own download URL and cache folder.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Origin {
    Modland,
    Keygen,
    Botb,
    ModArchive,
    SceneOrg,
    Amiga,
}

impl Origin {
    /// Every archive the app searches, in the order results are gathered.
    pub const ALL: [Origin; 6] = [
        Origin::Modland,
        Origin::Keygen,
        Origin::Botb,
        Origin::ModArchive,
        Origin::SceneOrg,
        Origin::Amiga,
    ];

    /// Archives that have to be asked over the network on every search.
    pub const LIVE: [Origin; 4] = [
        Origin::Botb,
        Origin::ModArchive,
        Origin::SceneOrg,
        Origin::Amiga,
    ];

    /// Position in the per-origin result tables.
    pub fn index(self) -> usize {
        match self {
            Origin::Modland => 0,
            Origin::Keygen => 1,
            Origin::Botb => 2,
            Origin::ModArchive => 3,
            Origin::SceneOrg => 4,
            Origin::Amiga => 5,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Origin::Modland => "Modland",
            Origin::Keygen => "Keygen Music",
            Origin::Botb => "Battle of the Bits",
            Origin::ModArchive => "The Mod Archive",
            Origin::SceneOrg => "scene.org",
            Origin::Amiga => "Amiga Music Preservation",
        }
    }

    /// Short name for the narrow SOURCE column.
    pub fn short_label(self) -> &'static str {
        match self {
            Origin::Modland => "Modland",
            Origin::Keygen => "Keygen",
            Origin::Botb => "BotB",
            Origin::ModArchive => "ModArchive",
            Origin::SceneOrg => "scene.org",
            Origin::Amiga => "AMP",
        }
    }

    /// Prefix every synthetic path starts with, so a copied path can be traced
    /// back to the archive it came from.
    pub fn slug(self) -> &'static str {
        match self {
            Origin::Modland => "modland",
            Origin::Keygen => "kgm",
            Origin::Botb => "botb",
            Origin::ModArchive => "modarchive",
            Origin::SceneOrg => "sceneorg",
            Origin::Amiga => "amp",
        }
    }

    /// True for the archives that answer a live search instead of a local index.
    pub fn is_live(self) -> bool {
        !matches!(self, Origin::Modland | Origin::Keygen)
    }
}

/// The live archives and the search function for each one, so callers can walk
/// every remote source in one loop.
pub fn live_searches() -> [(Origin, fn(&str) -> Result<Vec<Entry>, String>); 4] {
    [
        (Origin::Botb, crate::botb::search),
        (Origin::ModArchive, crate::modarchive::search),
        (Origin::SceneOrg, crate::sceneorg::search),
        (Origin::Amiga, crate::amp::search),
    ]
}

#[derive(Clone, Debug)]
pub struct Entry {
    /// Catalogue path, e.g. `Fasttracker 2/Elwood/deadlock.xm`,
    /// or `botb/<id>/<file>` for a live search hit.
    pub path: String,
    pub size: u64,
    pub ext: String,
    pub artist: String,
    pub name: String,
    pub origin: Origin,
    /// Site-side id for live hits (BotB entry id, Mod Archive module id,
    /// AMP module id).
    pub id: u32,
    /// Direct file link when the archive supplies one; local catalogues build
    /// their link from the path instead.
    pub download_url: Option<String>,
    pub(crate) lower: String,
}

impl Entry {
    /// Every search term must appear somewhere in the path.
    pub fn matches(&self, terms: &[String]) -> bool {
        terms.iter().all(|term| self.lower.contains(term.as_str()))
    }

    pub fn url(&self) -> String {
        if let Some(url) = &self.download_url {
            return url.clone();
        }
        match self.origin {
            Origin::Modland => crate::download::module_url(&self.path),
            Origin::Keygen => crate::kgm::url(&self.path),
            _ => String::new(),
        }
    }

    /// Website page for origins that also have one to open in a browser.
    pub fn page_url(&self) -> Option<String> {
        match self.origin {
            Origin::Botb => Some(format!(
                "https://battleofthebits.com/player/Entry/{}/",
                self.id
            )),
            Origin::ModArchive => Some(format!(
                "https://modarchive.org/index.php?request=view_by_moduleid&query={}",
                self.id
            )),
            Origin::SceneOrg => Some(format!(
                "https://files.scene.org/view{}",
                self.path.strip_prefix("sceneorg").unwrap_or("")
            )),
            Origin::Amiga => Some(format!("https://amp.dascene.net/module/{}", self.id)),
            _ => None,
        }
    }

    /// True when the archive lets guests fetch the module file itself.
    pub fn can_download(&self) -> bool {
        self.origin != Origin::Botb
    }
}

/// Rebuild an entry from a path copied out of the UI or the command line.
/// The archive is inferred from the path prefix, so no source has to be chosen.
pub fn entry_from_path(path: &str) -> Option<Entry> {
    let path = path.trim();
    if path.is_empty() {
        return None;
    }
    let (origin, rest) = if let Some(rest) = path.strip_prefix("kgm/") {
        (Origin::Keygen, rest)
    } else if let Some(rest) = path.strip_prefix("botb/") {
        (Origin::Botb, rest)
    } else if let Some(rest) = path.strip_prefix("modarchive/") {
        (Origin::ModArchive, rest)
    } else if let Some(rest) = path.strip_prefix("sceneorg/") {
        (Origin::SceneOrg, rest)
    } else if let Some(rest) = path.strip_prefix("amp/") {
        (Origin::Amiga, rest)
    } else {
        (Origin::Modland, path)
    };

    let mut id = 0u32;
    let mut download_url = match origin {
        Origin::Modland => crate::download::module_url(path),
        Origin::Keygen => crate::kgm::url(path),
        Origin::SceneOrg => format!("https://files.scene.org/get/{rest}"),
        _ => String::new(),
    };
    if matches!(origin, Origin::Botb | Origin::ModArchive | Origin::Amiga) {
        id = rest.split('/').next()?.parse().ok()?;
        download_url = match origin {
            Origin::Botb => format!("https://battleofthebits.com/player/EntryDonload/{id}/"),
            Origin::ModArchive => format!("https://api.modarchive.org/downloads.php?moduleid={id}"),
            _ => format!("https://amp.dascene.net/module/{id}?download=1"),
        };
    }

    let file_name = path.rsplit('/').next().unwrap_or(path);
    let (stem, ext) = match file_name.rsplit_once('.') {
        Some((stem, ext)) => (stem, ext.to_ascii_lowercase()),
        None => (file_name, String::new()),
    };
    let parent = path
        .rsplit_once('/')
        .map(|(parent, _)| parent)
        .unwrap_or("");
    let artist = parent.rsplit('/').next().unwrap_or(parent).to_string();

    Some(Entry {
        path: path.to_string(),
        size: 0,
        ext,
        artist,
        name: stem.to_string(),
        origin,
        id,
        download_url: Some(download_url),
        lower: path.to_lowercase(),
    })
}

/// Parse the `allmods.txt` list: one `size<TAB>path` record per line.
pub fn parse_allmods(text: &str) -> Vec<Entry> {
    let mut entries = Vec::with_capacity(540_000);
    for line in text.lines() {
        let line = line.trim_end_matches('\r');
        let Some((size_text, path)) = line.split_once('\t') else {
            continue;
        };
        let Ok(size) = size_text.trim().parse::<u64>() else {
            continue;
        };
        let path = path.trim();
        if path.is_empty() {
            continue;
        }

        let file_name = path.rsplit('/').next().unwrap_or(path);
        let (stem, ext) = match file_name.rsplit_once('.') {
            Some((stem, ext)) => (stem, ext.to_ascii_lowercase()),
            None => (file_name, String::new()),
        };
        let parent = path
            .rsplit_once('/')
            .map(|(parent, _)| parent)
            .unwrap_or("");
        let artist = parent.rsplit('/').next().unwrap_or(parent).to_string();

        entries.push(Entry {
            path: path.to_string(),
            size,
            ext,
            artist,
            name: stem.to_string(),
            origin: Origin::Modland,
            id: 0,
            download_url: None,
            lower: path.to_lowercase(),
        });
    }
    entries
}

pub fn load_cached(path: &Path) -> Option<Vec<Entry>> {
    let text = fs::read_to_string(path).ok()?;
    if text.is_empty() {
        return None;
    }
    Some(parse_allmods(&text))
}

/// Download allmods.zip, extract allmods.txt and parse it.
pub fn fetch_catalog(zip_path: &Path) -> Result<(String, Vec<Entry>), String> {
    let bytes = fetch(ALLMODS_URL)?;
    let text = extract_allmods(&bytes)?;
    if let Some(parent) = zip_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(zip_path, &bytes);
    let entries = parse_allmods(&text);
    Ok((text, entries))
}

fn extract_allmods(bytes: &[u8]) -> Result<String, String> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes))
        .map_err(|error| format!("invalid allmods.zip: {error}"))?;
    let mut file = archive
        .by_name("allmods.txt")
        .map_err(|error| format!("allmods.txt not found in archive: {error}"))?;
    let mut text = String::new();
    file.read_to_string(&mut text)
        .map_err(|error| error.to_string())?;
    Ok(text)
}
