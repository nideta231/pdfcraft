# Attribution — Android port

- **PdfCraft**: <https://github.com/storytold/pdfcraft> (MIT OR Apache-2.0), unmodified except for the Android patch on the `android` branch.
- **Android shell** (`apps/pdfcraft-android/`, the vendored-crate notes and the save-dialog/clipboard patches) was started
  from [@kanaaka/pdfcraft-android](https://github.com/kanaaka/pdfcraft-android) (Apache-2.0 OR MIT, October 2026) and
  forward-ported to upstream v0.8.0 by [@nideta231](https://github.com/nideta231), who added the Android system bridge
  (`src/android_bridge.rs`), package identity, size-optimised build profile and release CI.
- Unofficial: not produced or endorsed by storytold / the ArtCraft team.
