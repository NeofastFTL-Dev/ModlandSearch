use std::array;
use std::cmp::Ordering;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::Duration;

use eframe::egui::{
    self, Align, Color32, Layout, RichText, Stroke,
};
use modland_search::catalog::{self, Entry, Origin};
use modland_search::download;
use modland_search::kgm;
use modland_search::paths;
use modland_search::player::Player;
use modland_search::settings::Settings;

const BG: Color32 = Color32::from_rgb(0x12, 0x17, 0x1e);
const SIDEBAR: Color32 = Color32::from_rgb(0x0d, 0x12, 0x18);
const ROW_A: Color32 = Color32::from_rgb(0x16, 0x1d, 0x26);
const ROW_B: Color32 = Color32::from_rgb(0x1a, 0x22, 0x2c);
const TEXT: Color32 = Color32::from_rgb(0xe7, 0xeb, 0xef);
const MUTED: Color32 = Color32::from_rgb(0x8d, 0x99, 0xa8);
const ACCENT: Color32 = Color32::from_rgb(0x3f, 0x7d, 0x5c);
const ACCENT_SOFT: Color32 = Color32::from_rgb(0x2b, 0x4a, 0x3b);
const LINK: Color32 = Color32::from_rgb(0x76, 0xba, 0x94);
const PINK_TEXT: Color32 = Color32::from_rgb(0xd9, 0x8a, 0xa0);
const FLAG_BLUE: Color32 = Color32::from_rgb(0x5b, 0xce, 0xfa);
const FLAG_PINK: Color32 = Color32::from_rgb(0xf5, 0xa9, 0xb8);

const WEBSITE_URL: &str = "https://neofastftldev.nl";
const MODLAND_URL: &str = "https://modland.com";
const KEYGEN_URL: &str = "https://keygen.music";
const BOTB_URL: &str = "https://battleofthebits.com";
const MODARCHIVE_URL: &str = "https://modarchive.org";
const SCENEORG_URL: &str = "https://files.scene.org";
const AMP_URL: &str = "https://amp.dascene.net";

const SOURCE_COUNT: usize = Origin::ALL.len();

#[derive(Clone, Copy, PartialEq, Eq)]
enum Nav {
    Search,
    Library,
    Settings,
    Support,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Sort {
    Catalogue,
    Name,
    Artist,
    Size,
}

impl Sort {
    fn label(self) -> &'static str {
        match self {
            Sort::Catalogue => "Catalogue order",
            Sort::Name => "Name",
            Sort::Artist => "Artist",
            Sort::Size => "Size (largest)",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Format {
    All,
    Mod,
    Xm,
    S3m,
    It,
    Mptm,
}

impl Format {
    fn label(self) -> &'static str {
        match self {
            Format::All => "All formats",
            Format::Mod => "MOD",
            Format::Xm => "XM",
            Format::S3m => "S3M",
            Format::It => "IT",
            Format::Mptm => "MPTM",
        }
    }

    fn matches(self, entry: &Entry) -> bool {
        match self {
            Format::All => true,
            Format::Mod => entry.ext == "mod",
            Format::Xm => entry.ext == "xm",
            Format::S3m => entry.ext == "s3m",
            Format::It => entry.ext == "it",
            Format::Mptm => entry.ext == "mptm",
        }
    }
}

enum Msg {
    Modland(Vec<Entry>, String),
    Keygen(Vec<Entry>, String),
    Live {
        origin: Origin,
        seq: u64,
        result: Result<Vec<Entry>, String>,
    },
    Error(String),
    PreviewReady {
        origin: Origin,
        rel: String,
        bytes: Vec<u8>,
    },
    Downloaded(String),
}

struct App {
    /// One list per archive; the search view merges all of them.
    lists: [Vec<Entry>; SOURCE_COUNT],
    nav: Nav,
    format: Format,
    sort: Sort,
    filtered: Vec<(Origin, usize)>,
    query: String,
    selected: Option<(Origin, usize)>,
    status: String,
    loading_modland: bool,
    loading_keygen: bool,
    settings: Settings,
    download_dir_text: String,
    player: Player,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
    preview_dir: PathBuf,
    active_downloads: usize,
    remote_seq: u64,
    remote_pending: usize,
}

impl App {
    fn new() -> Self {
        let (tx, rx) = channel();
        let settings = Settings::load();
        let mut app = App {
            lists: array::from_fn(|_| Vec::new()),
            nav: Nav::Search,
            format: Format::All,
            sort: Sort::Catalogue,
            filtered: Vec::new(),
            query: String::new(),
            selected: None,
            status: "Loading Modland + Keygen Music...".to_string(),
            loading_modland: true,
            loading_keygen: true,
            download_dir_text: settings.download_dir.display().to_string(),
            settings,
            player: Player::new(),
            tx: tx.clone(),
            rx,
            preview_dir: paths::preview_dir(),
            active_downloads: 0,
            remote_seq: 0,
            remote_pending: 0,
        };
        app.spawn_catalog(false);
        app.spawn_keygen();
        app
    }

    fn loading(&self) -> bool {
        self.loading_modland || self.loading_keygen
    }

    fn list(&self, origin: Origin) -> &[Entry] {
        &self.lists[origin.index()]
    }

    fn entry(&self, origin: Origin, index: usize) -> Option<&Entry> {
        self.list(origin).get(index)
    }

    /// Local catalogues filter instantly; the live archives are asked over the
    /// network each time a search is started, and all hits land in one list.
    fn search_all(&mut self) {
        let term = self
            .query
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_string();
        self.remote_seq += 1;
        let seq = self.remote_seq;
        for origin in Origin::LIVE {
            self.lists[origin.index()].clear();
        }
        self.run_search();
        if term.is_empty() {
            self.remote_pending = 0;
            return;
        }

        let searches = catalog::live_searches();
        self.remote_pending = searches.len();
        self.status = format!(
            "Searching {} live archives for {term} ...",
            searches.len()
        );
        for (origin, search) in searches {
            let tx = self.tx.clone();
            let term = term.clone();
            std::thread::spawn(move || {
                let _ = tx.send(Msg::Live {
                    origin,
                    seq,
                    result: search(&term),
                });
            });
        }
    }

    fn spawn_catalog(&mut self, force: bool) {
        self.loading_modland = true;
        self.status = if force {
            "Updating Modland catalogue from modland.com...".to_string()
        } else {
            "Reading Modland catalogue...".to_string()
        };
        let tx = self.tx.clone();
        let cache = paths::catalog_path();
        let zip = paths::catalog_zip_path();
        std::thread::spawn(move || {
            if !force {
                if let Some(entries) = catalog::load_cached(&cache) {
                    let count = entries.len();
                    let _ = tx.send(Msg::Modland(
                        entries,
                        format!("{count} Modland modules loaded"),
                    ));
                    return;
                }
            }
            match catalog::fetch_catalog(&zip) {
                Ok((text, entries)) => {
                    let _ = std::fs::write(&cache, text);
                    let count = entries.len();
                    let _ = tx.send(Msg::Modland(
                        entries,
                        format!("Modland catalogue updated: {count} modules"),
                    ));
                }
                Err(error) => {
                    let _ = tx.send(Msg::Error(format!("Catalogue download failed: {error}")));
                }
            }
        });
    }

    fn spawn_keygen(&mut self) {
        self.loading_keygen = true;
        self.status = "Reading Keygen Music library...".to_string();
        let tx = self.tx.clone();
        let path = paths::kgm_lib_path();
        std::thread::spawn(move || match std::fs::read_to_string(&path) {
            Ok(text) => match kgm::parse(&text) {
                Ok(entries) => {
                    let count = entries.len();
                    let _ = tx.send(Msg::Keygen(
                        entries,
                        format!("{count} Keygen Music tracks loaded"),
                    ));
                }
                Err(error) => {
                    let _ = tx.send(Msg::Error(format!("Keygen library unreadable: {error}")));
                }
            },
            Err(error) => {
                let _ = tx.send(Msg::Error(format!(
                    "Keygen library not found at {}: {error}",
                    path.display()
                )));
            }
        });
    }

    fn run_search(&mut self) {
        self.selected = None;
        if self.loading() {
            self.filtered.clear();
            return;
        }
        let terms: Vec<String> = self
            .query
            .split_whitespace()
            .map(|term| term.to_lowercase())
            .collect();
        let format = self.format;
        let mut hits: Vec<(Origin, usize)> = Vec::new();
        for origin in Origin::ALL {
            for (index, entry) in self.list(origin).iter().enumerate() {
                if format.matches(entry) && (terms.is_empty() || entry.matches(&terms)) {
                    hits.push((origin, index));
                }
            }
        }
        let entry_of = |hit: &(Origin, usize)| -> &Entry { &self.list(hit.0)[hit.1] };
        match self.sort {
            Sort::Catalogue => {}
            Sort::Name => hits.sort_unstable_by(|a, b| {
                compare_ci(&entry_of(a).name, &entry_of(b).name)
                    .then_with(|| compare_ci(&entry_of(a).artist, &entry_of(b).artist))
            }),
            Sort::Artist => hits.sort_unstable_by(|a, b| {
                compare_ci(&entry_of(a).artist, &entry_of(b).artist)
                    .then_with(|| compare_ci(&entry_of(a).name, &entry_of(b).name))
            }),
            Sort::Size => {
                hits.sort_unstable_by(|a, b| entry_of(b).size.cmp(&entry_of(a).size))
            }
        }
        self.filtered = hits;
    }

    fn cache_path(&self, rel: &str) -> PathBuf {
        let mut path = self.preview_dir.clone();
        for segment in rel.split('/').filter(|segment| !segment.is_empty()) {
            path.push(download::sanitize_segment(segment));
        }
        path
    }

    fn start_preview(&mut self, origin: Origin, index: usize) {
        let Some(entry) = self.entry(origin, index) else {
            return;
        };
        let rel = entry.path.clone();
        let name = entry.name.clone();
        let title = format!("{} - {}", entry.artist, entry.name);
        let url = entry.url();
        let cached = self.cache_path(&rel);

        if cached.exists() {
            match std::fs::read(&cached) {
                Ok(bytes) => match self.player.play(bytes, title.clone(), self.settings.volume) {
                    Ok(()) => self.status = format!("Playing {title}"),
                    Err(error) => self.status = format!("Playback failed: {error}"),
                },
                Err(error) => self.status = format!("Could not read preview cache: {error}"),
            }
            return;
        }

        let tx = self.tx.clone();
        self.status = format!("Fetching {name} ...");
        std::thread::spawn(move || match download::fetch(&url) {
            Ok(bytes) => {
                if let Some(parent) = cached.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                let target = cached.clone();
                let temporary = target.with_file_name(format!(
                    "{}.part",
                    target
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("module")
                ));
                if std::fs::write(&temporary, &bytes).is_ok() {
                    let _ = std::fs::rename(&temporary, &target);
                }
                let _ = tx.send(Msg::PreviewReady { origin, rel, bytes });
            }
            Err(error) => {
                let _ = tx.send(Msg::Error(format!("Download failed: {error}")));
            }
        });
    }

    fn start_download(&mut self, origin: Origin, index: usize) {
        let Some(entry) = self.entry(origin, index) else {
            return;
        };
        let rel = entry.path.clone();
        let file_name = rel.rsplit('/').next().unwrap_or("module").to_string();
        let url = entry.url();
        if self.settings.download_dir.join(&file_name).exists() {
            self.status = format!("Already in your library: {file_name}");
            return;
        }

        let tx = self.tx.clone();
        let directory = self.settings.download_dir.clone();
        let part_name = format!("{file_name}.part");
        self.active_downloads += 1;
        self.status = format!("Downloading {file_name} ...");
        std::thread::spawn(move || {
            let result = (|| -> Result<PathBuf, String> {
                let bytes = download::fetch(&url)?;
                std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
                let destination = directory.join(&file_name);
                let temporary = directory.join(&part_name);
                std::fs::write(&temporary, &bytes).map_err(|error| error.to_string())?;
                std::fs::rename(&temporary, &destination).map_err(|error| error.to_string())?;
                Ok(destination)
            })();
            match result {
                Ok(path) => {
                    let _ = tx.send(Msg::Downloaded(format!("Saved {}", path.display())));
                }
                Err(error) => {
                    let _ = tx.send(Msg::Error(format!("Download failed: {error}")));
                }
            }
        });
    }

    fn handle_messages(&mut self) {
        while let Ok(message) = self.rx.try_recv() {
            match message {
                Msg::Modland(entries, status) => {
                    self.lists[Origin::Modland.index()] = entries;
                    self.loading_modland = false;
                    self.status = status;
                    self.run_search();
                }
                Msg::Keygen(entries, status) => {
                    self.lists[Origin::Keygen.index()] = entries;
                    self.loading_keygen = false;
                    self.status = status;
                    self.run_search();
                }
                Msg::Live { origin, seq, result } => {
                    if seq == self.remote_seq {
                        self.remote_pending = self.remote_pending.saturating_sub(1);
                        match result {
                            Ok(entries) => {
                                self.lists[origin.index()] = entries;
                                self.run_search();
                                self.status =
                                    format!("{}: {} entries", origin.label(), thousands(self.list(origin).len()));
                                if self.remote_pending == 0 {
                                    self.status = format!(
                                        "{} results across {} archives",
                                        thousands(self.filtered.len()),
                                        SOURCE_COUNT
                                    );
                                }
                            }
                            Err(error) => {
                                self.status =
                                    format!("{} search failed: {error}", origin.label());
                            }
                        }
                    }
                }
                Msg::Error(error) => {
                    self.loading_modland = false;
                    self.loading_keygen = false;
                    self.active_downloads = self.active_downloads.saturating_sub(1);
                    self.status = error;
                }
                Msg::PreviewReady { origin, rel, bytes } => {
                    let title = self
                        .list(origin)
                        .iter()
                        .find(|entry| entry.path == rel)
                        .map(|entry| format!("{} - {}", entry.artist, entry.name))
                        .unwrap_or_else(|| rel.clone());
                    match self.player.play(bytes, title.clone(), self.settings.volume) {
                        Ok(()) => self.status = format!("Playing {title}"),
                        Err(error) => self.status = format!("Playback failed: {error}"),
                    }
                }
                Msg::Downloaded(message) => {
                    self.active_downloads = self.active_downloads.saturating_sub(1);
                    self.status = message;
                }
            }
        }
    }

    fn download_folder_editor(&mut self, ui: &mut egui::Ui) {
        let mut changed = false;
        ui.horizontal(|ui| {
            let response = ui.add_sized(
                [420.0, 26.0],
                egui::TextEdit::singleline(&mut self.download_dir_text),
            );
            if response.changed() {
                self.settings.download_dir =
                    PathBuf::from(self.download_dir_text.trim().to_string());
                changed = true;
            }
            if ui.button("Browse...").clicked() {
                if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                    self.download_dir_text = folder.display().to_string();
                    self.settings.download_dir = folder;
                    changed = true;
                }
            }
            if ui.button("Open").clicked() {
                open_folder(&self.settings.download_dir);
            }
        });
        if changed {
            self.settings.save();
        }
    }

    fn search_view(&mut self, ui: &mut egui::Ui) {
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            let find_width = 104.0;
            let gap = ui.spacing().item_spacing.x;
            let field_width = (ui.available_width() - find_width - gap).max(160.0);
            let response = ui.add_sized(
                [field_width, 38.0],
                egui::TextEdit::singleline(&mut self.query)
                    .hint_text("Search every archive, e.g. Elwood deadlock"),
            );
            let submitted =
                response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
            let find = ui.add_sized(
                [find_width, 38.0],
                egui::Button::new(
                    RichText::new("Find")
                        .strong()
                        .color(Color32::from_rgb(18, 23, 30)),
                )
                .fill(Color32::from_rgb(0xd8, 0xdf, 0xe6)),
            );
            if submitted || find.clicked() {
                self.search_all();
            }
        });
        ui.add_space(16.0);

        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("{} results", thousands(self.filtered.len())))
                    .size(15.0)
                    .color(MUTED),
            );
            if self.remote_pending > 0 {
                ui.spinner();
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    RichText::new("One list - Modland, Keygen, BotB, Mod Archive, scene.org, AMP")
                        .small()
                        .color(MUTED),
                );
            });
        });
        ui.add_space(6.0);

        if self.loading() {
            ui.vertical_centered(|ui| {
                ui.add_space(60.0);
                ui.spinner();
            });
            return;
        }

        if Origin::ALL.iter().all(|origin| self.list(*origin).is_empty()) {
            ui.vertical_centered(|ui| {
                ui.add_space(60.0);
                ui.label(
                    RichText::new(
                        "No catalogue loaded yet. Update the Modland catalogue in Settings, add kgm-lib.txt for Keygen Music, or search a term to query the live archives.",
                    )
                    .color(MUTED),
                );
            });
            return;
        }
        if self.filtered.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(60.0);
                ui.label(RichText::new("No modules match this search").color(MUTED));
            });
            return;
        }

        let widths = column_widths(ui.available_width());
        table_header(ui, &widths);
        ui.add_space(2.0);

        let row_height = 30.0;
        let mut play_request: Option<(Origin, usize)> = None;
        let mut download_request: Option<(Origin, usize)> = None;
        let mut open_request: Option<(Origin, usize)> = None;
        let mut select_request: Option<(Origin, usize)> = None;
        let rows = self.filtered.len();
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show_rows(ui, row_height, rows, |ui, range| {
                ui.spacing_mut().item_spacing.y = 0.0;
                for row in range {
                    let (origin, index) = self.filtered[row];
                    let entry = &self.list(origin)[index];
                    let top = ui.cursor().top();
                    let left = ui.cursor().left();
                    let row_rect = egui::Rect::from_min_size(
                        egui::pos2(left, top),
                        egui::vec2(ui.available_width(), row_height),
                    );
                    let fill = if self.selected == Some((origin, index)) {
                        ACCENT_SOFT
                    } else if row % 2 == 0 {
                        ROW_A
                    } else {
                        ROW_B
                    };
                    ui.painter().rect_filled(row_rect, 0.0, fill);
                    let downloadable = entry.can_download();
                    let has_page = entry.page_url().is_some();
                    let inner = ui.allocate_ui_with_layout(
                        egui::vec2(row_rect.width(), row_height),
                        Layout::left_to_right(Align::Center),
                        |ui| {
                            ui.add_sized(
                                [widths.name, row_height],
                                egui::Label::new(
                                    RichText::new(ellipsize(&entry.name, 64)).color(TEXT).strong(),
                                ),
                            );
                            ui.add_sized(
                                [widths.artist, row_height],
                                egui::Label::new(
                                    RichText::new(ellipsize(&entry.artist, 30)).color(MUTED),
                                ),
                            );
                            ui.add_sized(
                                [widths.source, row_height],
                                egui::Label::new(
                                    RichText::new(origin.short_label())
                                        .small()
                                        .color(LINK),
                                ),
                            );
                            ui.add_sized(
                                [widths.format, row_height],
                                egui::Label::new(
                                    RichText::new(entry.ext.to_uppercase()).small().color(MUTED),
                                ),
                            );
                            ui.add_sized(
                                [widths.size, row_height],
                                egui::Label::new(
                                    RichText::new(if entry.size > 0 {
                                        download::human_size(entry.size)
                                    } else {
                                        "-".to_string()
                                    })
                                    .small()
                                    .color(MUTED),
                                ),
                            );
                            ui.allocate_ui_with_layout(
                                egui::vec2(widths.actions, row_height),
                                Layout::right_to_left(Align::Center),
                                |ui| {
                                    if has_page && ui.small_button("Open").clicked() {
                                        open_request = Some((origin, index));
                                    }
                                    if downloadable {
                                        if ui.small_button("Save").clicked() {
                                            download_request = Some((origin, index));
                                        }
                                        if ui.small_button("Play").clicked() {
                                            play_request = Some((origin, index));
                                        }
                                    }
                                },
                            );
                        },
                    );
                    let response = inner.response.interact(egui::Sense::click());
                    if response.clicked() {
                        select_request = Some((origin, index));
                    }
                    if response.double_clicked() {
                        if downloadable {
                            play_request = Some((origin, index));
                        } else {
                            open_request = Some((origin, index));
                        }
                    }
                }
            });

        if let Some(hit) = select_request {
            self.selected = Some(hit);
        }
        if let Some((origin, index)) = open_request {
            if let Some(url) = self.entry(origin, index).and_then(Entry::page_url) {
                ui.ctx().open_url(egui::OpenUrl {
                    url,
                    new_tab: true,
                });
                self.status = "Opened the page in your browser".to_string();
            }
        }
        if let Some((origin, index)) = play_request {
            self.start_preview(origin, index);
        }
        if let Some((origin, index)) = download_request {
            self.start_download(origin, index);
        }
    }

    fn library_view(&mut self, ui: &mut egui::Ui) {
        ui.add_space(6.0);
        ui.heading("Library");
        ui.add_space(4.0);
        ui.label(
            RichText::new("Modules you save land in this folder. Preview streaming uses a separate cache.")
                .color(MUTED),
        );
        ui.add_space(14.0);
        self.download_folder_editor(ui);
        ui.add_space(10.0);
        if ui.button("Open download folder").clicked() {
            open_folder(&self.settings.download_dir);
        }
        ui.add_space(16.0);
        ui.separator();
        ui.add_space(10.0);
        let count = std::fs::read_dir(&self.settings.download_dir)
            .map(|entries| entries.filter_map(|entry| entry.ok()).count())
            .unwrap_or(0);
        ui.label(
            RichText::new(format!("{} file(s) in the library folder", thousands(count)))
                .color(MUTED),
        );
        ui.add_space(6.0);
        ui.label(
            RichText::new(format!("{}", self.settings.download_dir.display()))
                .small()
                .color(TEXT),
        );
    }

    fn settings_view(&mut self, ui: &mut egui::Ui) {
        ui.add_space(6.0);
        ui.heading("Settings");
        ui.add_space(4.0);
        ui.label(RichText::new("Download folder").color(MUTED));
        ui.add_space(6.0);
        self.download_folder_editor(ui);
        ui.add_space(16.0);
        ui.separator();
        ui.add_space(10.0);
        ui.label(RichText::new("Data").color(MUTED));
        ui.add_space(4.0);
        ui.label(
            RichText::new(format!("Modland catalogue: {}", paths::catalog_path().display()))
                .small(),
        );
        ui.label(
            RichText::new(format!("Keygen index: {}", paths::kgm_lib_path().display())).small(),
        );
        ui.label(
            RichText::new(format!("Preview cache: {}", self.preview_dir.display())).small(),
        );
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            if ui.button("Update Modland catalogue").clicked() {
                self.spawn_catalog(true);
            }
            if ui.button("Reload Keygen index").clicked() {
                self.spawn_keygen();
            }
            if ui.button("Clear preview cache").clicked() {
                let _ = std::fs::remove_dir_all(&self.preview_dir);
                let _ = std::fs::create_dir_all(&self.preview_dir);
                self.status = "Preview cache cleared".to_string();
            }
        });
        ui.add_space(16.0);
        ui.separator();
        ui.add_space(10.0);
        ui.label(RichText::new("Volume").color(MUTED));
        let mut volume = self.settings.volume;
        if ui
            .add_sized(
                [240.0, 20.0],
                egui::Slider::new(&mut volume, 0.0..=1.0).show_value(true),
            )
            .changed()
        {
            self.settings.volume = volume;
            self.player.set_volume(volume);
            self.settings.save();
        }
        ui.add_space(16.0);
        ui.label(
            RichText::new("Double-click a result to preview it. BotB, The Mod Archive, scene.org and Amiga Music Preservation are searched live on every Find and their hits join the same list as Modland and Keygen Music. Settings live in %LOCALAPPDATA%\\ModlandSearch\\settings.ini.")
                .small()
                .color(MUTED),
        );
    }

    fn support_view(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        flag_strip(ui);
        ui.add_space(20.0);
        ui.scope(|ui| {
            ui.set_max_width(820.0);
            ui.label(RichText::new("hey, I'm NeofastFTL ♡").size(26.0).strong());
            ui.add_space(2.0);
            ui.label(
                RichText::new("ModlandSearch · made for tracker music lovers")
                    .size(14.0)
                    .color(LINK),
            );
            ui.add_space(18.0);
            ui.label(
                RichText::new("Thank you for using this little tracker-music browser. I hope it saves you a few tabs and a bit of chaos."),
            );
            ui.add_space(10.0);
            ui.label(
                RichText::new("If ModlandSearch helped you, sharing it with a fellow tracker fan is the sweetest thank-you."),
            );
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                link_button(ui, "Open website", WEBSITE_URL);
                link_button(ui, "Modland archive", MODLAND_URL);
                link_button(ui, "Keygen Music", KEYGEN_URL);
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                link_button(ui, "Battle of the Bits", BOTB_URL);
                link_button(ui, "The Mod Archive", MODARCHIVE_URL);
                link_button(ui, "scene.org", SCENEORG_URL);
                link_button(ui, "Amiga Music Preservation", AMP_URL);
            });

            ui.add_space(26.0);
            ui.label(RichText::new("Support").size(22.0).color(PINK_TEXT));
            ui.add_space(6.0);
            ui.label(
                RichText::new("Modland, Keygen Music, Battle of the Bits, The Mod Archive, scene.org and Amiga Music Preservation keep these archives alive. Kind words, bug reports and a little patience keep this project going. No pressure, just vibes."),
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new(format!(
                    "You're on version {} - updates arrive with a new installer.",
                    env!("CARGO_PKG_VERSION")
                ))
                .small()
                .color(MUTED),
            );

            ui.add_space(24.0);
            ui.label(RichText::new("ARR license").size(22.0).color(PINK_TEXT));
            ui.add_space(6.0);
            ui.label(RichText::new("Copyright 2026 NeofastFTL - All Rights Reserved").strong());
            ui.add_space(8.0);
            ui.label("The ModlandSearch name, icon, and this app's presentation are mine. Please don't rebrand it as your own.");
            ui.add_space(4.0);
            ui.label("You're warmly welcome to use it, love it, and share it with friends.");
            ui.add_space(8.0);
            ui.label(
                RichText::new(
                    "Made with too much tracker music, a little sky blue, and a lot of stubborn girl energy.",
                )
                .italics()
                .color(LINK),
            );
            ui.add_space(20.0);
            ui.label(RichText::new("thank you for being here ♡").color(PINK_TEXT));
            ui.add_space(20.0);
        });
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_messages();

        egui::SidePanel::left("sidebar")
            .exact_width(224.0)
            .resizable(false)
            .frame(egui::Frame::default().fill(SIDEBAR))
            .show(ctx, |ui| {
                ui.add_space(18.0);
                ui.horizontal(|ui| {
                    ui.add_space(14.0);
                    ui.vertical(|ui| {
                        ui.label(RichText::new("ModlandSearch").size(22.0).strong());
                        ui.label(
                            RichText::new("6 archives - one list")
                                .small()
                                .color(MUTED),
                        );
                    });
                });
                ui.add_space(18.0);
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    ui.vertical(|ui| {
                        let width = 196.0;
                        if nav_button(ui, "Search", self.nav == Nav::Search, width) {
                            self.nav = Nav::Search;
                        }
                        if nav_button(ui, "Library", self.nav == Nav::Library, width) {
                            self.nav = Nav::Library;
                        }
                        if nav_button(ui, "Settings", self.nav == Nav::Settings, width) {
                            self.nav = Nav::Settings;
                        }
                        ui.add_space(6.0);
                        if nav_button(ui, "Support ♡", self.nav == Nav::Support, width) {
                            self.nav = Nav::Support;
                        }

                        ui.add_space(16.0);
                        ui.label(RichText::new("SOURCES").small().color(MUTED));
                        ui.add_space(4.0);
                        for origin in Origin::ALL {
                            ui.label(
                                RichText::new(format!(
                                    "{} {}",
                                    thousands(self.list(origin).len()),
                                    origin.label()
                                ))
                                .small()
                                .color(TEXT),
                            );
                        }
                        ui.add_space(2.0);
                        ui.label(
                            RichText::new("Last four are searched live on every Find")
                                .small()
                                .color(MUTED),
                        );

                        ui.add_space(10.0);
                        ui.label(RichText::new("FORMAT").small().color(MUTED));
                        ui.add_space(4.0);
                        let mut format = self.format;
                        egui::ComboBox::from_id_salt("format_combo")
                            .width(width)
                            .selected_text(format.label())
                            .show_ui(ui, |ui| {
                                for option in [
                                    Format::All,
                                    Format::Mod,
                                    Format::Xm,
                                    Format::S3m,
                                    Format::It,
                                    Format::Mptm,
                                ] {
                                    ui.selectable_value(&mut format, option, option.label());
                                }
                            });
                        if format != self.format {
                            self.format = format;
                            self.run_search();
                        }

                        ui.add_space(10.0);
                        ui.label(RichText::new("SORT").small().color(MUTED));
                        ui.add_space(4.0);
                        let mut sort = self.sort;
                        egui::ComboBox::from_id_salt("sort_combo")
                            .width(width)
                            .selected_text(sort.label())
                            .show_ui(ui, |ui| {
                                for option in
                                    [Sort::Catalogue, Sort::Name, Sort::Artist, Sort::Size]
                                {
                                    ui.selectable_value(&mut sort, option, option.label());
                                }
                            });
                        if sort != self.sort {
                            self.sort = sort;
                            self.run_search();
                        }

                        ui.add_space(12.0);
                        if ui
                            .add_sized([width, 26.0], egui::Button::new("Reset filters"))
                            .clicked()
                        {
                            self.query.clear();
                            self.format = Format::All;
                            self.sort = Sort::Catalogue;
                            self.run_search();
                        }

                        ui.add_space(18.0);
                        ui.separator();
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new(
                                "Double-click a row to play\nSave writes to your folder\nOpen shows the song page",
                            )
                            .small()
                            .color(MUTED),
                        );
                    });
                });
            });

        egui::TopBottomPanel::bottom("statusbar")
            .frame(egui::Frame::default().fill(SIDEBAR))
            .show(ctx, |ui| {
                ui.add_space(5.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(ellipsize(&self.status, 90)).small().color(MUTED));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let mut volume = self.settings.volume;
                        if ui
                            .add_sized(
                                [120.0, 16.0],
                                egui::Slider::new(&mut volume, 0.0..=1.0).show_value(false),
                            )
                            .changed()
                        {
                            self.settings.volume = volume;
                            self.player.set_volume(volume);
                            self.settings.save();
                        }
                        ui.label(RichText::new("Volume").small().color(MUTED));
                        if let Some(title) = self.player.now_playing.clone() {
                            if ui.small_button("Stop").clicked() {
                                self.player.stop();
                                self.status = "Stopped".to_string();
                            }
                            ui.label(
                                RichText::new(ellipsize(&title, 42)).small().color(TEXT),
                            );
                        }
                    });
                });
                ui.add_space(5.0);
            });

        egui::CentralPanel::default()
            .frame(
                egui::Frame::default()
                    .fill(BG)
                    .inner_margin(egui::Margin::symmetric(18, 14)),
            )
            .show(ctx, |ui| match self.nav {
                Nav::Search => self.search_view(ui),
                Nav::Library => self.library_view(ui),
                Nav::Settings => self.settings_view(ui),
                Nav::Support => self.support_view(ui),
            });

        if self.loading() || self.active_downloads > 0 || self.remote_pending > 0 {
            ctx.request_repaint_after(Duration::from_millis(200));
        }
    }
}

fn nav_button(ui: &mut egui::Ui, text: &str, active: bool, width: f32) -> bool {
    let fill = if active { ACCENT } else { Color32::TRANSPARENT };
    let mut button = egui::Button::new(RichText::new(text).size(14.0).color(if active {
        Color32::WHITE
    } else {
        TEXT
    }))
    .min_size(egui::vec2(width, 30.0))
    .fill(fill)
    .stroke(Stroke::NONE);
    if !active {
        button = button.frame(false);
    }
    ui.add(button).clicked()
}

fn column_widths(available: f32) -> ColumnWidths {
    let actions = 158.0;
    let artist = 170.0;
    let source = 95.0;
    let format = 60.0;
    let size = 75.0;
    let gaps = 5.0 * 8.0;
    let name = (available - actions - artist - source - format - size - gaps).max(150.0);
    ColumnWidths {
        name,
        artist,
        source,
        format,
        size,
        actions,
    }
}

struct ColumnWidths {
    name: f32,
    artist: f32,
    source: f32,
    format: f32,
    size: f32,
    actions: f32,
}

fn table_header(ui: &mut egui::Ui, widths: &ColumnWidths) {
    ui.horizontal(|ui| {
        for (label, width) in [
            ("NAME", widths.name),
            ("ARTIST", widths.artist),
            ("SOURCE", widths.source),
            ("FORMAT", widths.format),
            ("SIZE", widths.size),
            ("", widths.actions),
        ] {
            ui.add_sized(
                [width, 20.0],
                egui::Label::new(RichText::new(label).small().color(MUTED)),
            );
        }
    });
    ui.separator();
}

fn compare_ci(a: &str, b: &str) -> Ordering {
    let mut left = a.chars().flat_map(char::to_lowercase);
    let mut right = b.chars().flat_map(char::to_lowercase);
    loop {
        match (left.next(), right.next()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) => match x.cmp(&y) {
                Ordering::Equal => {}
                other => return other,
            },
        }
    }
}

fn ellipsize(text: &str, max_chars: usize) -> String {
    let count = text.chars().count();
    if count <= max_chars {
        return text.to_string();
    }
    let mut shortened: String = text.chars().take(max_chars.saturating_sub(1)).collect();
    shortened.push('\u{2026}');
    shortened
}

fn thousands(value: usize) -> String {
    let digits = value.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, ch) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index) % 3 == 0 {
            out.push(' ');
        }
        out.push(ch);
    }
    out
}

fn open_folder(path: &std::path::Path) {
    let _ = std::fs::create_dir_all(path);
    let _ = std::process::Command::new("explorer").arg(path).spawn();
}

fn link_button(ui: &mut egui::Ui, label: &str, url: &str) {
    let button = egui::Button::new(RichText::new(label).color(TEXT))
        .fill(Color32::from_rgb(0x2a, 0x33, 0x3f));
    if ui.add(button).clicked() {
        ui.ctx().open_url(egui::OpenUrl {
            url: url.to_string(),
            new_tab: true,
        });
    }
}

fn flag_strip(ui: &mut egui::Ui) {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 6.0), egui::Sense::hover());
    let colors = [
        FLAG_BLUE,
        FLAG_PINK,
        Color32::WHITE,
        FLAG_PINK,
        FLAG_BLUE,
    ];
    let fractions = [0.18, 0.18, 0.28, 0.18, 0.18];
    let mut x = rect.left();
    for (color, fraction) in colors.iter().zip(fractions.iter()) {
        let segment = rect.width() * fraction;
        ui.painter().rect_filled(
            egui::Rect::from_min_size(
                egui::pos2(x, rect.top()),
                egui::vec2(segment, rect.height()),
            ),
            0.0,
            *color,
        );
        x += segment;
    }
}

fn apply_theme(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = BG;
    visuals.window_fill = BG;
    visuals.faint_bg_color = ROW_A;
    visuals.extreme_bg_color = Color32::from_rgb(0x0b, 0x0f, 0x14);
    visuals.override_text_color = Some(TEXT);
    visuals.selection.bg_fill = ACCENT;
    visuals.selection.stroke = Stroke::new(1.0_f32, Color32::WHITE);
    visuals.hyperlink_color = LINK;
    visuals.widgets.inactive.bg_fill = ROW_B;
    visuals.widgets.inactive.weak_bg_fill = ROW_B;
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(0x2a, 0x35, 0x43);
    visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(0x2a, 0x35, 0x43);
    visuals.widgets.active.bg_fill = ACCENT;
    visuals.widgets.active.weak_bg_fill = ACCENT_SOFT;
    style.visuals = visuals;
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(10.0, 5.0);
    ctx.set_style(style);
}

fn main() -> eframe::Result<()> {
    paths::ensure_appdata();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("ModlandSearch")
            .with_inner_size([1180.0, 760.0])
            .with_min_inner_size([900.0, 560.0]),
        ..Default::default()
    };
    eframe::run_native(
        "ModlandSearch",
        options,
        Box::new(|cc| {
            apply_theme(&cc.egui_ctx);
            Ok(Box::new(App::new()))
        }),
    )
}
