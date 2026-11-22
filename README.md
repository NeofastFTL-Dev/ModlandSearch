# ModlandSearch

Browse, preview and download **tracker modules** from the big module archives in one place.
Windows desktop app (Rust + egui) with an optional command-line twin.

Search across six sources, filter by format, listen to a preview before downloading, and keep a
local library of what you grabbed — without opening six websites.

## Sources it searches

| Source | Address | Notes |
|---|---|---|
| Modland | <https://modland.com> | the main catalogue |
| Keygen Music | <https://keygen.music> | KGM packs |
| Battle of the Bots | <https://battleofthebits.com> | BotB entries |
| The Mod Archive | <https://modarchive.org> | public module archive |
| scene.org | <https://files.scene.org> | demoscene releases |
| Amiga Music Preservation | <https://amp.dascene.net> | Amiga-era modules |

Each site keeps its own terms and rate limits; this client only performs the queries a normal
browser would, and you are responsible for respecting those sites.

## What it does

* Search all sources at once, or one at a time, with sorting by catalogue order, name, artist and
  size.
* Format filters (MOD and friends) and size/artist columns in a virtualised list.
* **Preview playback** through libopenmpt — modules, KGM and Ogg/MP3 inside archives.
* Downloads into a library folder, with progress in the status bar.
* Settings and the preview cache live in `%LOCALAPPDATA%\ModlandSearch`, never next to the exe.
* A second binary, `modland-cli`, exposes the same catalogue from the command line
  (`modland-cli --help`).

## Repository layout

| Path | What |
|---|---|
| `src/main.rs` | GUI (`ModlandZoeker.exe`), egui/eframe |
| `src/bin/modland_cli.rs` | CLI (`modland-cli.exe`) |
| `src/*.rs` | one module per source (`modland` lives in `catalog.rs`, plus `kgm`, `botb`, `modarchive`, `sceneorg`, `amp`), `download`, `player`, `settings`, `paths` |
| `third_party/libopenmpt/` | libopenmpt licence texts and changelog; the headers, import libs and `amd64` DLLs are **not** tracked - `scripts/fetch-libopenmpt.sh` fetches them |
| `app.rc`, `assets/icon.ico`, `build.rs` | Windows resources (icon + version info) embedded at build time |
| `installer.iss` | Inno Setup 6 script |
| `build-installer.ps1` | one-step release build: `cargo build --release` + ISCC |

## Requirements

* **Windows x64** (the app embeds Win32 resources and loads `libopenmpt.dll`, so this is the
  supported target).
* Rust with the **MSVC** toolchain (`rustup default stable-msvc`), for `embed-resource`/`windres`-free
  resource embedding.
* **Inno Setup 6** only if you want the installer.
* No external libopenmpt install: the DLLs are vendored in `third_party/libopenmpt/bin/amd64/`.

## Building

`./build.sh` wraps the build and the DLL copying described below, so a fresh clone needs nothing
from outside the repository (it does need `cargo`; Rust is installed here via rustup in `~/.cargo`).
The libopenmpt DLLs are not stored in git: on the first build the script downloads the pinned
libopenmpt 0.8.10 Windows dev package (SHA-256 checked) into `target/libopenmpt-cache` and unpacks
`bin/` into `third_party/libopenmpt/`. Later builds reuse that copy and work offline.

```bash
./build.sh                                 # host build
TARGET=x86_64-pc-windows-gnu ./build.sh    # cross-compile from Linux
```

By hand:

```powershell
cargo build --release
```

That produces:

```
target\release\ModlandZoeker.exe      # GUI
target\release\modland-cli.exe        # CLI
```

`ModlandZoeker.exe` needs these files **next to it** (they are not statically linked):

```
libopenmpt.dll  openmpt-mpg123.dll  openmpt-ogg.dll  openmpt-vorbis.dll  openmpt-zlib.dll
```

Copy them from `third_party\libopenmpt\bin\amd64\` — that folder is filled in by
`scripts/fetch-libopenmpt.sh` (run by `build.sh`, or by hand when building with `cargo` directly).
The installer does this for you.

### Installer

```powershell
powershell -ExecutionPolicy Bypass -File build-installer.ps1
# or, if ISCC is on PATH:
ISCC.exe installer.iss
```

Output: `dist\ModlandSearchSetup-1.1.0.exe` (per-user install, no admin rights, ships the GUI, the
CLI, the libopenmpt DLLs, their licence and this README).

### Building on Linux

Cross-compiling works on this machine; only the installer step still needs Windows (Inno Setup).

```bash
rustup target add x86_64-pc-windows-gnu   # once; mingw-w64-gcc provides the linker and dlltool
TARGET=x86_64-pc-windows-gnu ./build.sh
```

Output lands in `target/x86_64-pc-windows-gnu/release/` (GUI, CLI and the five libopenmpt DLLs).
Verified here with rustup 1.100.0 and mingw-w64-gcc 16.2.0.

## Configuration

* Settings and preview cache: `%LOCALAPPDATA%\ModlandSearch`.
* Nothing is written to the install folder, so upgrades and uninstalls never touch your library.

## Screenshots

Screenshots belong in `docs/screenshots/`; none are committed yet. Three shots tell the whole
story:

| File to add | What to capture |
|---|---|
| `docs/screenshots/search.png` | Search results with a few sources selected and the format filter visible |
| `docs/screenshots/player.png` | A module previewing in the player bar |
| `docs/screenshots/library.png` | The library page with a handful of downloaded modules |

```html
<!-- Uncomment once the files exist:
![Search results](docs/screenshots/search.png)
![Player](docs/screenshots/player.png)
![Library](docs/screenshots/library.png)
-->
```

## Credits

* [libopenmpt](https://lib.openmpt.org/) — BSD-3-Clause, tracker module decoding and audio output
  through its bundled MPG123/Ogg/Vorbis/zlib helpers. The vendored copy keeps its own
  `third_party/libopenmpt/LICENSE.txt`.
* [egui / eframe](https://github.com/emilk/egui) — MIT / Apache-2.0, the whole UI.
* [rodio](https://github.com/RustAudio/rodio) — MIT, audio output.
* [ureq](https://github.com/algesten/ureq) — MIT / Apache-2.0, HTTPS client.
* [serde](https://serde.rs/) / [serde_json](https://github.com/serde-rs/json) — MIT / Apache-2.0.
* [zip](https://github.com/zip-rs/zip2) — MIT, reading archived modules.
* [rfd](https://github.com/PolyMeilex/rfd) — MIT, file dialogs.
* [libloading](https://github.com/nagisa/rust_libloading) — ISC, loading `libopenmpt.dll`.
* [embed-resource](https://github.com/nabijaczleweli/rust-embed-resource) — MIT, icon/version info.
* Catalogue data belongs to Modland, Keygen Music, Battle of the Bots, The Mod Archive, scene.org
  and Amiga Music Preservation — this app only queries their public endpoints.

## License

MIT, as already declared in `Cargo.toml` - see [LICENSE](LICENSE).

The libopenmpt DLLs that `build.sh` fetches are BSD-3-Clause; their licence texts ship in
`third_party/libopenmpt/` next to the code that loads them.
