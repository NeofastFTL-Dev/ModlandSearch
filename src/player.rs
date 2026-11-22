//! Audio playback for tracker modules through libopenmpt + rodio.
//!
//! libopenmpt.dll is loaded at runtime, so the app runs without it (playback
//! simply reports unavailable) and the installer just ships the DLL.

use std::ffi::{c_void, CStr};
use std::path::PathBuf;
use std::time::Duration;

use libloading::Library;
use rodio::{OutputStream, OutputStreamHandle, Sink, Source};

type ModuleHandle = c_void;

type CreateFn = unsafe extern "C" fn(
    filedata: *const c_void,
    filesize: usize,
    logfunc: *const c_void,
    loguser: *const c_void,
    errfunc: *const c_void,
    erruser: *const c_void,
    error: *mut i32,
    error_message: *mut *const i8,
) -> *mut ModuleHandle;
type ReadFn = unsafe extern "C" fn(*mut ModuleHandle, i32, usize, *mut f32, *mut f32) -> usize;
type DestroyFn = unsafe extern "C" fn(*mut ModuleHandle);
type SetRepeatFn = unsafe extern "C" fn(*mut ModuleHandle, i32);

const CHUNK: usize = 1024;
const SAMPLE_RATE: u32 = 48_000;

#[derive(Clone, Copy)]
struct Api {
    create: CreateFn,
    read: ReadFn,
    destroy: DestroyFn,
    set_repeat: SetRepeatFn,
}

impl Api {
    unsafe fn load(library: &Library) -> Result<Api, String> {
        let create = *library
            .get::<CreateFn>(b"openmpt_module_create_from_memory\0")
            .map_err(|error| format!("libopenmpt entry point: {error}"))?;
        let read = *library
            .get::<ReadFn>(b"openmpt_module_read_float_stereo\0")
            .map_err(|error| format!("libopenmpt entry point: {error}"))?;
        let destroy = *library
            .get::<DestroyFn>(b"openmpt_module_destroy\0")
            .map_err(|error| format!("libopenmpt entry point: {error}"))?;
        let set_repeat = *library
            .get::<SetRepeatFn>(b"openmpt_module_set_repeat_count\0")
            .map_err(|error| format!("libopenmpt entry point: {error}"))?;
        Ok(Api {
            create,
            read,
            destroy,
            set_repeat,
        })
    }
}

pub struct Player {
    _library: Option<Library>,
    _dependencies: Vec<Library>,
    api: Option<Api>,
    _stream: Option<OutputStream>,
    handle: Option<OutputStreamHandle>,
    sink: Option<Sink>,
    pub now_playing: Option<String>,
    pub error: Option<String>,
}

impl Player {
    pub fn new() -> Self {
        let mut player = Player {
            _library: None,
            _dependencies: Vec::new(),
            api: None,
            _stream: None,
            handle: None,
            sink: None,
            now_playing: None,
            error: None,
        };

        match find_library() {
            Some(path) => match load_library(&path) {
                Ok((library, dependencies)) => match unsafe { Api::load(&library) } {
                    Ok(api) => {
                        player.api = Some(api);
                        player._library = Some(library);
                        player._dependencies = dependencies;
                    }
                    Err(error) => player.error = Some(error),
                },
                Err(error) => player.error = Some(format!("Could not load libopenmpt.dll: {error}")),
            },
            None => player.error = Some("libopenmpt.dll was not found".to_string()),
        }

        match OutputStream::try_default() {
            Ok((stream, handle)) => {
                player._stream = Some(stream);
                player.handle = Some(handle);
            }
            Err(error) => {
                if player.error.is_none() {
                    player.error = Some(format!("No audio output device: {error}"));
                }
            }
        }

        player
    }

    pub fn available(&self) -> bool {
        self.api.is_some() && self.handle.is_some()
    }

    /// Decode module bytes and start playback, replacing anything already playing.
    pub fn play(&mut self, bytes: Vec<u8>, title: String, volume: f32) -> Result<(), String> {
        let api = self
            .api
            .ok_or_else(|| self.error.clone().unwrap_or_else(|| "Playback is unavailable".into()))?;
        let handle = self
            .handle
            .as_ref()
            .ok_or_else(|| self.error.clone().unwrap_or_else(|| "No audio device".into()))?;

        let mut error = 0i32;
        let mut message: *const i8 = std::ptr::null();
        let module = unsafe {
            (api.create)(
                bytes.as_ptr() as *const c_void,
                bytes.len(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                &mut error,
                &mut message,
            )
        };
        if module.is_null() {
            let reason = if message.is_null() {
                "libopenmpt could not open this module".to_string()
            } else {
                unsafe { CStr::from_ptr(message).to_string_lossy().into_owned() }
            };
            return Err(reason);
        }
        unsafe { (api.set_repeat)(module, 0) };

        if let Some(previous) = self.sink.take() {
            previous.stop();
        }
        let sink = Sink::try_new(handle).map_err(|error| error.to_string())?;
        sink.set_volume(volume.clamp(0.0, 1.0));
        sink.append(ModuleSource {
            module,
            api,
            sample_rate: SAMPLE_RATE,
            left: vec![0.0; CHUNK],
            right: vec![0.0; CHUNK],
            interleaved: vec![0.0; CHUNK * 2],
            filled: 0,
            index: 0,
            done: false,
        });
        sink.play();
        self.sink = Some(sink);
        self.now_playing = Some(title);
        Ok(())
    }

    pub fn set_volume(&self, volume: f32) {
        if let Some(sink) = &self.sink {
            sink.set_volume(volume.clamp(0.0, 1.0));
        }
    }

    pub fn stop(&mut self) {
        if let Some(sink) = self.sink.take() {
            sink.stop();
        }
        self.now_playing = None;
    }
}

impl Default for Player {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        self.stop();
    }
}

struct ModuleSource {
    module: *mut ModuleHandle,
    api: Api,
    sample_rate: u32,
    left: Vec<f32>,
    right: Vec<f32>,
    interleaved: Vec<f32>,
    filled: usize,
    index: usize,
    done: bool,
}

/// Only the audio output thread touches the module handle.
unsafe impl Send for ModuleSource {}

impl Iterator for ModuleSource {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        loop {
            if self.index < self.filled {
                let sample = self.interleaved[self.index];
                self.index += 1;
                return Some(sample);
            }
            if self.done {
                return None;
            }
            let read = unsafe {
                (self.api.read)(
                    self.module,
                    self.sample_rate as i32,
                    CHUNK,
                    self.left.as_mut_ptr(),
                    self.right.as_mut_ptr(),
                )
            };
            if read == 0 {
                self.done = true;
                return None;
            }
            for frame in 0..read {
                self.interleaved[frame * 2] = self.left[frame];
                self.interleaved[frame * 2 + 1] = self.right[frame];
            }
            self.filled = read * 2;
            self.index = 0;
        }
    }
}

impl Source for ModuleSource {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> u16 {
        2
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

impl Drop for ModuleSource {
    fn drop(&mut self) {
        unsafe { (self.api.destroy)(self.module) };
    }
}

/// Load libopenmpt, preloading its codec DLLs so their own imports resolve too.
#[cfg(windows)]
fn load_library(path: &std::path::Path) -> Result<(Library, Vec<Library>), String> {
    use libloading::os::windows::{Library as WindowsLibrary, LOAD_WITH_ALTERED_SEARCH_PATH};
    let directory = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    let mut dependencies = Vec::new();
    for name in [
        "openmpt-zlib.dll",
        "openmpt-ogg.dll",
        "openmpt-vorbis.dll",
        "openmpt-mpg123.dll",
    ] {
        let candidate = directory.join(name);
        if !candidate.exists() {
            continue;
        }
        let library = unsafe {
            WindowsLibrary::load_with_flags(candidate.as_os_str(), LOAD_WITH_ALTERED_SEARCH_PATH)
        }
        .map_err(|error| format!("Could not load {name}: {error}"))?;
        dependencies.push(Library::from(library));
    }
    let library = unsafe {
        WindowsLibrary::load_with_flags(path.as_os_str(), LOAD_WITH_ALTERED_SEARCH_PATH)
    }
    .map_err(|error| error.to_string())?;
    Ok((Library::from(library), dependencies))
}

#[cfg(not(windows))]
fn load_library(path: &std::path::Path) -> Result<(Library, Vec<Library>), String> {
    let library = unsafe { Library::new(path) }.map_err(|error| error.to_string())?;
    Ok((library, Vec::new()))
}

fn find_library() -> Option<PathBuf> {
    let relative = PathBuf::from("third_party/libopenmpt/bin/amd64/libopenmpt.dll");
    let mut candidates = Vec::new();
    if let Ok(executable) = std::env::current_exe() {
        if let Some(directory) = executable.parent() {
            candidates.push(directory.join("libopenmpt.dll"));
        }
    }
    if let Ok(current) = std::env::current_dir() {
        candidates.push(current.join(&relative));
    }
    candidates.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(&relative));
    candidates.into_iter().find(|path| path.exists())
}
