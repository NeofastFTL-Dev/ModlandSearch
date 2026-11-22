use std::path::PathBuf;

/// Base directory for the app's own data: %LOCALAPPDATA%\ModlandSearch.
pub fn appdata_dir() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    base.join("ModlandSearch")
}

/// Create the data directories and return the app data root.
pub fn ensure_appdata() -> PathBuf {
    let dir = appdata_dir();
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::create_dir_all(dir.join("preview"));
    dir
}

pub fn settings_path() -> PathBuf {
    appdata_dir().join("settings.ini")
}

pub fn catalog_path() -> PathBuf {
    appdata_dir().join("allmods.txt")
}

pub fn catalog_zip_path() -> PathBuf {
    appdata_dir().join("allmods.zip")
}

pub fn preview_dir() -> PathBuf {
    appdata_dir().join("preview")
}

pub fn keygen_preview_dir() -> PathBuf {
    preview_dir().join("kgm")
}

pub fn kgm_lib_path() -> PathBuf {
    appdata_dir().join("kgm-lib.txt")
}

pub fn kgm_ver_path() -> PathBuf {
    appdata_dir().join("kgm-ver.txt")
}
