use std::path::PathBuf;

use modland_search::catalog::{self, Entry, Origin};
use modland_search::download;
use modland_search::kgm;
use modland_search::paths;
use modland_search::settings::Settings;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(command) = args.first().map(String::as_str) else {
        usage();
        return;
    };
    match command {
        "search" => search(&args[1..]),
        "download" => download_one(&args[1..]),
        "update" => {
            if ensure_catalog(true).is_none() {
                std::process::exit(1);
            }
        }
        "info" => info(),
        _ => usage(),
    }
}

fn usage() {
    eprintln!("ModlandSearch command line - every archive in one list");
    eprintln!("  modland-cli search <terms...> [--limit N]");
    eprintln!("  modland-cli download <path> [--out DIR]");
    eprintln!("  modland-cli update");
    eprintln!("  modland-cli info");
}

fn ensure_catalog(force: bool) -> Option<Vec<Entry>> {
    paths::ensure_appdata();
    let cache = paths::catalog_path();
    if !force {
        if let Some(entries) = catalog::load_cached(&cache) {
            return Some(entries);
        }
    }
    eprintln!("Downloading Modland catalogue ...");
    match catalog::fetch_catalog(&paths::catalog_zip_path()) {
        Ok((text, entries)) => {
            let _ = std::fs::write(&cache, text);
            eprintln!("Catalogue: {} modules", entries.len());
            Some(entries)
        }
        Err(error) => {
            eprintln!("error: {error}");
            None
        }
    }
}

fn load_keygen() -> Option<Vec<Entry>> {
    paths::ensure_appdata();
    match kgm::load_cached(&paths::kgm_lib_path()) {
        Some(entries) => Some(entries),
        None => {
            eprintln!(
                "note: no readable Keygen index at {}",
                paths::kgm_lib_path().display()
            );
            None
        }
    }
}

/// Ask every live archive at once and collect the hits.
fn search_live(term: &str) -> Vec<Entry> {
    let (tx, rx) = std::sync::mpsc::channel();
    let searches = catalog::live_searches();
    eprintln!(
        "Searching {} live archives for {term} ...",
        searches.len()
    );
    for (origin, search) in searches {
        let tx = tx.clone();
        let term = term.to_string();
        std::thread::spawn(move || {
            let _ = tx.send((origin, search(&term)));
        });
    }
    drop(tx);

    let mut entries = Vec::new();
    for (origin, result) in rx {
        match result {
            Ok(more) => entries.extend(more),
            Err(error) => eprintln!("note: {} skipped: {error}", origin.label()),
        }
    }
    entries
}

fn search(args: &[String]) {
    let mut limit = 50usize;
    let mut terms = Vec::new();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--limit" => {
                index += 1;
                if let Some(value) = args.get(index) {
                    limit = value.parse().unwrap_or(limit);
                }
            }
            other => terms.push(other.to_lowercase()),
        }
        index += 1;
    }
    if terms.is_empty() {
        eprintln!("error: give at least one search term");
        return;
    }

    let mut entries: Vec<Entry> = Vec::new();
    if let Some(local) = ensure_catalog(false) {
        entries.extend(local);
    }
    if let Some(keygen) = load_keygen() {
        entries.extend(keygen);
    }
    entries.extend(search_live(&terms[0]));

    let mut shown = 0usize;
    for entry in entries.iter().filter(|entry| entry.matches(&terms)) {
        let size = if entry.size > 0 {
            download::human_size(entry.size)
        } else {
            "-".to_string()
        };
        println!(
            "{:>9}  {:<10}  {}",
            size,
            entry.origin.short_label(),
            entry.path
        );
        shown += 1;
        if shown >= limit {
            break;
        }
    }
    eprintln!("{shown} result(s) shown");
}

fn download_one(args: &[String]) {
    let mut out: Option<PathBuf> = None;
    let mut path: Option<String> = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--out" => {
                index += 1;
                out = args.get(index).map(PathBuf::from);
            }
            other if path.is_none() => path = Some(other.to_string()),
            _ => {}
        }
        index += 1;
    }
    let Some(path) = path else {
        eprintln!("error: give a path from a search result");
        return;
    };
    let Some(entry) = catalog::entry_from_path(&path) else {
        eprintln!("error: {path} does not look like a module path");
        return;
    };
    let directory = out.unwrap_or_else(|| Settings::load().download_dir);
    let file_name = entry
        .path
        .rsplit('/')
        .next()
        .unwrap_or("module")
        .to_string();
    match download::fetch(&entry.url()) {
        Ok(bytes) => {
            if let Err(error) = std::fs::create_dir_all(&directory) {
                eprintln!("error: {error}");
                std::process::exit(1);
            }
            let destination = directory.join(&file_name);
            match std::fs::write(&destination, bytes) {
                Ok(()) => println!("{}", destination.display()),
                Err(error) => {
                    eprintln!("error: {error}");
                    std::process::exit(1);
                }
            }
        }
        Err(error) => {
            eprintln!("error: {error}");
            std::process::exit(1);
        }
    }
}

fn info() {
    paths::ensure_appdata();
    if let Some(entries) = catalog::load_cached(&paths::catalog_path()) {
        println!("Modland catalogue entries: {}", entries.len());
    } else {
        println!("Modland catalogue entries: none (run `modland-cli update`)");
    }
    match kgm::load_cached(&paths::kgm_lib_path()) {
        Some(entries) => println!("Keygen Music entries: {}", entries.len()),
        None => println!(
            "Keygen Music entries: none ({})",
            paths::kgm_lib_path().display()
        ),
    }
    println!(
        "Live archives: {}",
        Origin::LIVE
            .iter()
            .map(|origin| origin.label())
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!("Modland catalogue: {}", paths::catalog_path().display());
    println!("Keygen index: {}", paths::kgm_lib_path().display());
    println!("Preview cache: {}", paths::preview_dir().display());
    println!("Download folder: {}", Settings::load().download_dir.display());
    println!("Settings: {}", paths::settings_path().display());
}
