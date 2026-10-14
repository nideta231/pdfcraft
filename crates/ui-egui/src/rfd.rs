//! The native file dialogs this crate asks for, resolved under one name on every platform:
//! `crate::rfd`.
//!
//! # Why the name is resolved here and not in `Cargo.toml`
//!
//! Everything above `PdfCraftApp`'s own drawing wants a path the user chose, and the desktop gets
//! one from `crate::rfd::AsyncFileDialog`: the future is created on the UI thread (macOS shows the panel
//! there), a worker thread waits for it, and the app uses the choice on a later frame. That is the
//! whole of `pickers.rs`, and 42 call sites build a dialog with it.
//!
//! On Android `rfd` has no backend at all. Its `backend.rs` compiles no module for a target that
//! is neither Linux/BSD, macOS, Windows nor wasm32: the `FilePickerDialogImpl` family is declared
//! and nothing implements it, so the build fails at the first call with `the trait bound
//! FileDialog: FilePickerDialogImpl is not satisfied`. Android's own picker is not a substitute —
//! the Storage Access Framework needs `startActivityForResult`, which `android-activity` 0.6 does
//! not expose.
//!
//! The shell cannot supply another crate *called* `rfd` either: Cargo refuses a dependency whose
//! source path differs per target ("Dependency 'rfd' has different source paths depending on the
//! build target. Each dependency must have a single canonical source path irrespective of build
//! target."). So the name is resolved here instead. Every call site says `crate::rfd::…`, and this
//! module is the real `rfd` wherever it exists and a host-backed stand-in on Android.
//!
//! Nothing about the desktop changes: `pub use ::rfd::*` is the same crate, the same types and the
//! same behaviour it always had, reached through one extra hop.

#[cfg(not(target_os = "android"))]
pub use ::rfd::*;

/// The Android stand-in, in its own file so the re-export above stays one line.
#[cfg(target_os = "android")]
#[path = "rfd_android.rs"]
mod android;
#[cfg(target_os = "android")]
pub use android::{AsyncFileDialog, FileHandle, Kind, Request, install_picker};
