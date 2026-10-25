# Developing xpaint

xpaint is written in Rust with [egui/eframe](https://github.com/emilk/egui).
For using xpaint, see the [README](README.md).

## Building from source

You need a Rust toolchain ([rustup.rs](https://rustup.rs)).

```sh
git clone https://github.com/matz0r/xpaint.git
cd xpaint
cargo run -- [image files...]
```

Files given on the command line open as tabs.

Release builds use LTO; dependencies are built with `opt-level = 3` even in dev
builds so image decoding stays fast.

## Installing a local build

`./install.sh` builds a release binary when it finds `Cargo.toml` next to it and
installs it for the current user:

- the program at `~/.local/bin/xpaint`
- the icons and a desktop entry under `~/.local/share`

`./install.sh uninstall` removes it again. In a release archive (no `Cargo.toml`),
`install.sh` uses the bundled binary instead.

The prebuilt release binary needs glibc 2.39 or newer.

## Tests and linting

```sh
cargo test                     # all tests
cargo test undo_and_redo_crop  # a single test
cargo clippy
cargo fmt
```

A brush frame-time benchmark is ignored by default:

```sh
cargo test --release -- --ignored --nocapture brush_frame_time
```

## Code layout

- `src/app.rs` — the whole UI and interaction state (toolbar, tabs, canvas,
  status bar, shortcuts)
- `src/document.rs` — one open image with its zoom, selection and undo/redo
  history
- `src/shapes.rs` — shape geometry, the live preview and rasterization with
  tiny-skia
- `src/text.rs` — text layout and rasterization with the embedded DejaVu fonts
- `src/clipboard.rs` — reading images from the system clipboard (X11 and Wayland)

UI icons are SVGs in `assets/icons/` (24×24 viewBox, white 2px round strokes, no
fill). They are tinted at runtime, so new icons should follow the same style.

## Dev aids

- `XPAINT_SCREENSHOT=out.png cargo run -- some.png` saves the app's own frame
  after about a second and exits.
- `python3 assets/make_icon.py` regenerates the app icon (needs Pillow).

## Releasing

1. Bump `version` in `Cargo.toml` and commit.
2. Tag and push: `git tag vX.Y.Z && git push origin main vX.Y.Z`

`.github/workflows/release.yml` builds on the `v*` tag and publishes a GitHub
release with `xpaint-vX.Y.Z-x86_64-linux.tar.gz` (binary, `install.sh`, assets).
