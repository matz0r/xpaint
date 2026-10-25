# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

xpaint is a small paint.net-style image editor for Linux written in Rust with egui/eframe: tabs of open images, selection + crop, brush/arrow/rectangle/ellipse/text tools, undo/redo, paste-from-clipboard.

## Commands

- Build: `cargo build` / `cargo build --release` (release uses LTO; dependencies are built with `opt-level = 3` even in dev so image decoding stays fast)
- Run: `cargo run -- [image files...]` — files given on the command line open as tabs
- Test all: `cargo test`
- Single test: `cargo test <name>`, e.g. `cargo test undo_and_redo_crop`
- Perf benchmark (ignored by default): `cargo test --release -- --ignored --nocapture brush_frame_time`
- Lint: `cargo clippy`, format: `cargo fmt`
- Install for the current user (binary, icons, desktop entry into `~/.local`): `./install.sh`; remove with `./install.sh uninstall`
- Release: bump `version` in `Cargo.toml`, commit, then `git tag vX.Y.Z && git push origin main vX.Y.Z`. `.github/workflows/release.yml` builds on the `v*` tag and publishes a GitHub release with `xpaint-vX.Y.Z-x86_64-linux.tar.gz` (binary, `install.sh`, assets). `install.sh` uses the bundled binary when there is no `Cargo.toml` next to it.
- Screenshot the running app (dev aid): `XPAINT_SCREENSHOT=out.png cargo run -- some.png` saves the app's own frame after ~1 s and exits
- Regenerate the icon (needs Pillow): `python3 assets/make_icon.py`

## Architecture

- `src/app.rs` — `XPaintApp`, the whole UI and interaction state (toolbar, tabs, canvas, status bar, shortcuts). Most of the code lives here.
- `src/document.rs` — `Document`: one open image (`RgbaImage`), its path, zoom, selection, undo/redo, and cached egui texture. Every edit goes through `record(label)` which snapshots the full image; history is trimmed to a ~1 GiB budget (always keeping the latest step). After changing `image`, set `texture = None` so it is re-uploaded.
- `src/shapes.rs` — shape geometry. A `Shape` is decomposed into `Primitive`s; the canvas paints those with egui as the live preview and `rasterize` (tiny-skia) draws the same primitives into pixels on commit, so preview and result match. Also hit-testing/bounds for moving shapes.
- `src/text.rs` — text layout and rasterization with ab_glyph using the embedded DejaVu fonts (`assets/fonts`).
- `src/clipboard.rs` — reads an image from the system clipboard via arboard (Wayland supported).

Key interaction model: finished shapes (`PendingShape`) and text (`PendingText`) **float** above the image rather than being written immediately — they can still be moved (drag / arrow-key nudge), restyled, or deleted. They are composited into the document (`Document::draw_overlay`, one undoable step) when committed, e.g. on switching tools, Enter, or starting something new.

UI icons are SVGs in `assets/icons/` (24×24 viewBox, white 2px round strokes, no fill), embedded with `egui::include_image!` and rendered by `egui_extras`' SVG loader. Being white, they are tinted to the widget's state color when painted (`icon_button`, `paint_icon`, `menu_icon` in `app.rs`). New icons should follow the same style. Icon and button sizes are multiplied by `icon_scale(ui)` (the Settings menu's icon size, saved via eframe `persistence` under the `icon_size` key); size new icon buttons through these helpers so they follow the setting.

Coordinates: document state is in image pixels; the canvas converts via `image_rect.min + pos * zoom`.

Keyboard shortcuts mirror paint.net where possible (e.g. Ctrl+Shift+X crop, Ctrl+Alt+V paste into new image, Ctrl+B fit). When matching shortcuts, check the Shift variant first — egui ignores extra Shift when matching Ctrl+S / Ctrl+Z.

## Testing

Unit tests sit in each module's `mod tests`. App-level tests in `app.rs` drive a headless `egui::Context` with synthetic `RawInput` events (helpers `frame`, `canvas_frame`, `drag`, `button`, `key`) and construct the app via `XPaintApp::with_context`; they must clear `textures_delta` since there is no renderer.
