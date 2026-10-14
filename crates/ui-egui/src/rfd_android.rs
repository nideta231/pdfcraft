//! `rfd` on Android: the same calls, answered by the host's own picker.
//!
//! # What this is
//!
//! A drop-in replacement for the small part of `rfd` this crate uses — `AsyncFileDialog` with its
//! filters and its four `pick_*`/`save_file` futures, and `FileHandle`. The calls are unchanged;
//! what answers them is not a native panel but the Android shell, which draws a picker with egui
//! (it has no other way to show one) and is the only party that can.
//!
//! # How the answer gets back
//!
//! `pickers.rs` creates the future on the UI thread and blocks a worker thread on it
//! (`pollster::block_on`), because on macOS the panel has to be created on the UI thread while
//! the waiting must not be. This keeps that shape exactly:
//!
//! 1. `pick_file()`/`save_file()`/… **register the request with the host immediately** — it is
//!    called from `PdfCraftApp::ask` during `ui`, which is the only moment that has an egui frame
//!    to draw in — and return a future that waits on a channel.
//! 2. The shell draws the picker, and answers on that channel when the user chooses or cancels
//!    (an empty answer means cancelled; `pickers.rs` already reads that as one).
//! 3. The worker wakes, queues the pick, and wakes the window; the app uses it on a later frame.
//!
//! The future blocks the thread that polls it. That is not a slip: it is a dedicated worker, which
//! is what rfd's own Linux backend does, and the alternative — polling the channel from `poll` —
//! would need a waker the picker has no way to reach.
//!
//! # When no picker is installed
//!
//! A test, or a host that forgot to install one, gets "cancelled" at once with an error in the
//! log. Waiting for a dialog that can never be drawn would look like a hang.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::mpsc::{Receiver, channel};

/// Which panel was asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// One existing file.
    File,
    /// One or more existing files.
    Files,
    /// A folder.
    Folder,
    /// A path to save to.
    Save,
}

/// What the picker is being asked to show.
///
/// `extensions` is every extension the filters named, flattened: the picker lists files, and a
/// tablet has no room for filter group labels. An empty list means every file is listed.
#[derive(Clone, Debug)]
pub struct Request {
    pub kind: Kind,
    /// The dialog's title, as the caller set it (`set_title`).
    pub title: Option<String>,
    /// Extensions the listing is filtered to.
    pub extensions: Vec<String>,
    /// A suggested file name (`set_file_name`), for a Save panel.
    pub suggested_name: Option<String>,
}

/// The host's picker: show `request`, and answer on the receiver it returns.
///
/// An empty answer means the user cancelled — never a dropped sender, which `pickers.rs` would
/// also read as cancelled but which would hide a picker that died.
pub type Picker = Box<dyn Fn(Request) -> Receiver<Vec<PathBuf>> + Send + Sync>;

/// The one picker the host installed.
static PICKER: OnceLock<Picker> = OnceLock::new();

/// Give the dialogs their implementation. Called once by the Android shell, before the first
/// frame; a second call is logged and ignored.
pub fn install_picker(picker: Picker) {
    if PICKER.set(picker).is_err() {
        log::warn!("a file picker is already installed; this one is ignored");
    }
}

/// Ask the host to show `request`.
fn start(request: Request) -> Receiver<Vec<PathBuf>> {
    match PICKER.get() {
        Some(picker) => picker(request),
        None => {
            log::error!("no file picker installed: {:?} is cancelled", request.kind);
            let (answer, picks) = channel();
            let _ = answer.send(Vec::new());
            picks
        }
    }
}

/// A file the user chose.
///
/// rfd's `FileHandle` in one field: the path is the whole of it, because a path is what this
/// platform can hand back — there is no `File` object to hold on to, as there is on the web.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileHandle(PathBuf);

impl FileHandle {
    /// The path the user chose.
    pub fn path(&self) -> &Path {
        &self.0
    }

    /// rfd's name for the same thing, kept so call sites do not care which platform they are on.
    pub fn inner(&self) -> &Path {
        &self.0
    }

    /// The last component of the path, as rfd's does.
    pub fn file_name(&self) -> String {
        self.0.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
    }

    /// The file's bytes, read now. An unreadable file logs and reads as empty, which is what rfd's
    /// native `read` does with an I/O error.
    pub async fn read(&self) -> Vec<u8> {
        match std::fs::read(&self.0) {
            Ok(bytes) => bytes,
            Err(e) => {
                log::error!("couldn't read {}: {e}", self.0.display());
                Vec::new()
            }
        }
    }
}

/// rfd's `AsyncFileDialog`, as far as this crate uses it: a builder, and the four futures.
///
/// Building one has no effect — only the `pick_*`/`save_file` call does — so a dialog built and
/// then not used (as `files_picker(purpose)` is, on a path that returns early) shows nothing.
#[derive(Clone, Debug, Default)]
pub struct AsyncFileDialog {
    title: Option<String>,
    extensions: Vec<String>,
    file_name: Option<String>,
}

impl AsyncFileDialog {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a filter. The label is dropped: the picker filters the listing by extension and has no
    /// room for group headings — but it must be accepted, because every call site passes one.
    pub fn add_filter(mut self, _label: impl Into<String>, extensions: &[impl ToString]) -> Self {
        self.extensions.extend(extensions.iter().map(|e| e.to_string()));
        self
    }

    pub fn set_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn set_file_name(mut self, file_name: impl Into<String>) -> Self {
        self.file_name = Some(file_name.into());
        self
    }

    /// Ignored, deliberately: the directory a desktop caller suggests is a desktop path, and the
    /// picker starts in the app's own folder instead, which is the only one it knows is reachable.
    pub fn set_directory<P: AsRef<Path>>(self, _directory: P) -> Self {
        self
    }

    fn ask(&self, kind: Kind) -> Receiver<Vec<PathBuf>> {
        start(Request {
            kind,
            title: self.title.clone(),
            extensions: self.extensions.clone(),
            suggested_name: self.file_name.clone(),
        })
    }

    /// One file, or `None` when the user cancelled.
    pub fn pick_file(self) -> impl Future<Output = Option<FileHandle>> {
        let picks = self.ask(Kind::File);
        async move { picks.recv().ok().and_then(|paths| paths.into_iter().next()).map(FileHandle) }
    }

    /// Several files, or `None` when the user cancelled. rfd returns `Some(vec![])` for an empty
    /// multi-selection; `pickers.rs` reads both as "nothing to do", so `None` is the honest one.
    pub fn pick_files(self) -> impl Future<Output = Option<Vec<FileHandle>>> {
        let picks = self.ask(Kind::Files);
        async move {
            let paths = picks.recv().ok().unwrap_or_default();
            (!paths.is_empty()).then(|| paths.into_iter().map(FileHandle).collect())
        }
    }

    /// One folder, or `None` when the user cancelled.
    pub fn pick_folder(self) -> impl Future<Output = Option<FileHandle>> {
        let picks = self.ask(Kind::Folder);
        async move { picks.recv().ok().and_then(|paths| paths.into_iter().next()).map(FileHandle) }
    }

    /// A path to write to, or `None` when the user cancelled.
    pub fn save_file(self) -> impl Future<Output = Option<FileHandle>> {
        let picks = self.ask(Kind::Save);
        async move { picks.recv().ok().and_then(|paths| paths.into_iter().next()).map(FileHandle) }
    }
}
