use std::fs;
use std::path::PathBuf;

use crate::paths;

#[derive(Clone, Debug)]
pub struct Settings {
    pub download_dir: PathBuf,
    pub volume: f32,
}

impl Default for Settings {
    fn default() -> Self {
        let home = std::env::var_os("USERPROFILE")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        Settings {
            download_dir: home.join("Music").join("Modland"),
            volume: 0.68,
        }
    }
}

impl Settings {
    /// Load settings.ini, keeping defaults for anything missing or invalid.
    pub fn load() -> Self {
        let mut settings = Settings::default();
        if let Ok(text) = fs::read_to_string(paths::settings_path()) {
            for line in text.lines() {
                let Some((key, value)) = line.split_once('=') else {
                    continue;
                };
                match key.trim() {
                    "download_dir" => {
                        let value = value.trim();
                        if !value.is_empty() {
                            settings.download_dir = PathBuf::from(value);
                        }
                    }
                    "volume" => {
                        if let Ok(volume) = value.trim().parse::<f32>() {
                            settings.volume = volume.clamp(0.0, 1.0);
                        }
                    }
                    _ => {}
                }
            }
        }
        settings
    }

    pub fn save(&self) {
        let _ = fs::create_dir_all(paths::appdata_dir());
        let text = format!(
            "download_dir={}\nvolume={:.2}\n",
            self.download_dir.display(),
            self.volume
        );
        let _ = fs::write(paths::settings_path(), text);
    }
}
