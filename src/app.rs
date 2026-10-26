use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use egui::{
    Align, Align2, Color32, ColorImage, CornerRadius, CursorIcon, Event, Frame, ImageSource, Key,
    KeyboardShortcut, Layout, Margin, Modifiers, Pos2, Rect, RichText, Sense, Shape, Stroke,
    StrokeKind, TextureHandle, TextureId, TextureOptions, Ui, UiBuilder, Vec2, pos2, vec2,
};

use crate::clipboard;
use crate::document::Document;
use crate::install;
use crate::shapes::{self, Primitive, ShapeStyle};
use crate::text::{self, RenderedText, TextStyle};
#[cfg(test)]
use image::RgbaImage;

pub const ICON_PNG: &[u8] = include_bytes!("../assets/icon-256.png");

// UI icons: white line drawings, tinted to the widget's text color when painted.
const OPEN_ICON: ImageSource = egui::include_image!("../assets/icons/open.svg");
const COPY_ICON: ImageSource = egui::include_image!("../assets/icons/copy.svg");
const PASTE_ICON: ImageSource = egui::include_image!("../assets/icons/paste.svg");
const SAVE_ICON: ImageSource = egui::include_image!("../assets/icons/save.svg");
const UNDO_ICON: ImageSource = egui::include_image!("../assets/icons/undo.svg");
const REDO_ICON: ImageSource = egui::include_image!("../assets/icons/redo.svg");
const CROP_ICON: ImageSource = egui::include_image!("../assets/icons/crop.svg");
const ABOUT_ICON: ImageSource = egui::include_image!("../assets/icons/about.svg");
const FIT_ICON: ImageSource = egui::include_image!("../assets/icons/fit.svg");
const ACTUAL_SIZE_ICON: ImageSource = egui::include_image!("../assets/icons/actual-size.svg");
const ZOOM_ICON: ImageSource = egui::include_image!("../assets/icons/zoom.svg");
const CLOSE_ICON: ImageSource = egui::include_image!("../assets/icons/close.svg");
const THEME_SYSTEM_ICON: ImageSource = egui::include_image!("../assets/icons/theme-system.svg");
const THEME_DARK_ICON: ImageSource = egui::include_image!("../assets/icons/theme-dark.svg");
const THEME_LIGHT_ICON: ImageSource = egui::include_image!("../assets/icons/theme-light.svg");
const SETTINGS_ICON: ImageSource = egui::include_image!("../assets/icons/settings.svg");
const INSTALL_ICON: ImageSource = egui::include_image!("../assets/icons/install.svg");
const UPDATE_ICON: ImageSource = egui::include_image!("../assets/icons/update.svg");
const UNINSTALL_ICON: ImageSource = egui::include_image!("../assets/icons/uninstall.svg");

const OPEN_SHORTCUT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::O);
const SAVE_SHORTCUT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::S);
// Same shortcut paint.net uses for "Save As".
const SAVE_AS_SHORTCUT: KeyboardShortcut =
    KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::S);
// Same shortcut paint.net uses for "Paste into New Image".
const PASTE_NEW_SHORTCUT: KeyboardShortcut =
    KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::ALT), Key::V);
// Same shortcut paint.net uses for "Zoom to Window".
const COPY_SHORTCUT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::C);
const FIT_SHORTCUT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::B);
// Same shortcut paint.net uses for "Actual Size".
const ACTUAL_SIZE_SHORTCUT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Num0);
// Same shortcuts paint.net uses for "Crop to Selection" and "Deselect All".
const CROP_SHORTCUT: KeyboardShortcut =
    KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::X);
const DESELECT_SHORTCUT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::D);
const UNDO_SHORTCUT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Z);
const REDO_SHORTCUT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Y);
// Common alternative for redo on Linux.
const REDO_ALT_SHORTCUT: KeyboardShortcut =
    KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::Z);

const IMAGE_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "bmp", "gif", "tga", "tif", "tiff", "webp", "ico", "qoi",
];

/// Formats offered by "Save As"; the first one is the default.
const SAVE_FORMATS: &[(&str, &[&str])] = &[
    ("PNG", &["png"]),
    ("JPEG", &["jpg", "jpeg"]),
    ("WebP", &["webp"]),
    ("BMP", &["bmp"]),
    ("GIF", &["gif"]),
    ("TIFF", &["tif", "tiff"]),
    ("TGA", &["tga"]),
    ("QOI", &["qoi"]),
];

// Sizes at the Normal icon size; the icon size setting scales them (see `icon_scale`).
const TOOLBAR_HEIGHT: f32 = 42.0;
const ICON_BUTTON: Vec2 = vec2(32.0, 28.0);
const TOOL_BUTTON: Vec2 = vec2(36.0, 32.0);

/// Storage key of the icon size setting.
const ICON_SIZE_KEY: &str = "icon_size";

/// How large the toolbar and tool icons are drawn, together with their buttons.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum IconSize {
    #[default]
    Normal,
    Large,
    Larger,
}

impl IconSize {
    const ALL: [IconSize; 3] = [IconSize::Normal, IconSize::Large, IconSize::Larger];

    fn name(self) -> &'static str {
        match self {
            IconSize::Normal => "Normal",
            IconSize::Large => "Large",
            IconSize::Larger => "Larger",
        }
    }

    fn scale(self) -> f32 {
        match self {
            IconSize::Normal => 1.0,
            IconSize::Large => 1.25,
            IconSize::Larger => 1.5,
        }
    }

    fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|size| size.name() == name)
    }
}
const MIN_ZOOM: f32 = 0.05;
const MAX_ZOOM: f32 = 32.0;
const CHECKER_SIZE: f32 = 8.0;
/// Accent for selected tabs, tools and toggles: the blue of the icon's rainbow, dark
/// enough for white text (contrast about 5.3:1) in both the light and the dark theme.
const ACCENT: Color32 = Color32::from_rgb(0x2F, 0x6D, 0xB5);
/// A brighter blue for marks drawn over the image, like the text cursor, which must
/// show on dark and light pictures alike.
const CANVAS_ACCENT: Color32 = Color32::from_rgb(0x3D, 0x8B, 0xD9);
/// Minimum distance between the points of a brush stroke, in screen pixels.
const BRUSH_POINT_SPACING: f32 = 2.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tool {
    Select,
    Brush,
    Arrow,
    Rectangle,
    Ellipse,
    Text,
}

impl Tool {
    const ALL: [Tool; 6] = [
        Tool::Select,
        Tool::Brush,
        Tool::Arrow,
        Tool::Rectangle,
        Tool::Ellipse,
        Tool::Text,
    ];

    fn icon(self) -> ImageSource<'static> {
        match self {
            Tool::Select => egui::include_image!("../assets/icons/select.svg"),
            Tool::Brush => egui::include_image!("../assets/icons/brush.svg"),
            Tool::Arrow => egui::include_image!("../assets/icons/arrow.svg"),
            Tool::Rectangle => egui::include_image!("../assets/icons/rectangle.svg"),
            Tool::Ellipse => egui::include_image!("../assets/icons/ellipse.svg"),
            Tool::Text => egui::include_image!("../assets/icons/text.svg"),
        }
    }

    /// Also the name of the undo step for tools that draw.
    fn name(self) -> &'static str {
        match self {
            Tool::Select => "Rectangle Select",
            Tool::Brush => "Brush",
            Tool::Arrow => "Arrow",
            Tool::Rectangle => "Rectangle",
            Tool::Ellipse => "Ellipse",
            Tool::Text => "Text",
        }
    }

    /// Single-key shortcut; Select, Brush and Text match paint.net.
    fn key(self) -> Key {
        match self {
            Tool::Select => Key::S,
            Tool::Brush => Key::B,
            Tool::Arrow => Key::A,
            Tool::Rectangle => Key::R,
            Tool::Ellipse => Key::E,
            Tool::Text => Key::T,
        }
    }

    fn tooltip(self) -> String {
        let tip = format!("{} ({})", self.name(), self.key().name());
        match self {
            Tool::Rectangle => tip + "\nHold Shift for a square",
            Tool::Ellipse => tip + "\nHold Shift for a circle",
            _ => tip,
        }
    }
}

/// A shape being drawn by dragging; it becomes part of the image on release.
struct Drawing {
    /// Image pixel coordinates: the brush's path, or start and end of the drag.
    points: Vec<shapes::Point>,
    /// Shift is held: rectangles become squares and ellipses circles.
    constrain: bool,
}

impl Drawing {
    fn shape(&self, tool: Tool) -> Option<shapes::Shape> {
        let start = *self.points.first()?;
        let end = *self.points.last()?;
        let corner = if self.constrain {
            shapes::constrain_square(start, end)
        } else {
            end
        };
        match tool {
            Tool::Brush => Some(shapes::Shape::Brush(self.points.clone())),
            Tool::Arrow => Some(shapes::Shape::Arrow(start, end)),
            Tool::Rectangle => Some(shapes::Shape::Rectangle(start, corner)),
            Tool::Ellipse => Some(shapes::Shape::Ellipse(start, corner)),
            Tool::Select | Tool::Text => None,
        }
    }
}

/// A finished shape. Like [`PendingText`], it floats above the image until it is
/// applied, so it can still be moved and restyled.
struct PendingShape {
    tool: Tool,
    drawing: Drawing,
    /// How far it was moved since it was drawn, in image pixels.
    offset: Vec2,
    /// While dragging: the grabbed point minus `offset`, in image pixels.
    grab: Option<Vec2>,
}

impl PendingShape {
    fn primitives(&self, width: f32) -> Vec<Primitive> {
        let mut primitives = self
            .drawing
            .shape(self.tool)
            .map(|shape| shape.primitives(width))
            .unwrap_or_default();
        shapes::translate(&mut primitives, self.offset.x, self.offset.y);
        primitives
    }

    /// The dashed box around the shape on screen, for an image drawn at `image`.
    fn screen_box(&self, image: Rect, zoom: f32, width: f32) -> Option<Rect> {
        let (x0, y0, x1, y1) = shapes::bounds(&self.primitives(width), width, 0.0)?;
        let to_screen = |x, y| image.min + vec2(x, y) * zoom;
        Some(Rect::from_min_max(to_screen(x0, y0), to_screen(x1, y1)).expand(4.0))
    }
}

/// Text placed with the text tool. Until it is committed it floats above the image,
/// so it can still be typed into, moved and restyled.
struct PendingText {
    /// Top left of the first line, in image pixels.
    anchor: (i32, i32),
    text: String,
    /// Keys go into the text; otherwise it is only selected (movable, restylable).
    typing: bool,
    /// While dragging: the grabbed point, relative to the anchor, in image pixels.
    grab: Option<Vec2>,
    preview: Option<TextPreview>,
}

impl PendingText {
    fn new(anchor: (i32, i32)) -> Self {
        Self {
            anchor,
            text: String::new(),
            typing: true,
            grab: None,
            preview: None,
        }
    }
}

/// The last rendering of a [`PendingText`], kept until its text or style changes.
struct TextPreview {
    text: String,
    style: TextStyle,
    rendered: RenderedText,
    /// `None` while the text has no visible pixels.
    texture: Option<TextureHandle>,
}

impl PendingText {
    fn preview(&mut self, ctx: &egui::Context, style: &TextStyle) -> &TextPreview {
        let stale = self
            .preview
            .as_ref()
            .is_none_or(|p| p.text != self.text || p.style != *style);
        if stale {
            let rendered = text::render(&self.text, style);
            let (w, h) = rendered.image.dimensions();
            let texture = (w > 0 && h > 0).then(|| {
                let image = ColorImage::from_rgba_unmultiplied(
                    [w as usize, h as usize],
                    rendered.image.as_raw(),
                );
                ctx.load_texture("text-preview", image, TextureOptions::NEAREST)
            });
            self.preview = Some(TextPreview {
                text: self.text.clone(),
                style: *style,
                rendered,
                texture,
            });
        }
        self.preview.as_ref().expect("preview was just rendered")
    }
}

/// What the canvas needs to draw a [`PendingText`], copied out so the document can be
/// borrowed at the same time.
struct TextOverlay {
    anchor: Vec2,
    texture: Option<TextureId>,
    origin: Vec2,
    image_size: Vec2,
    box_size: Vec2,
    caret: Vec2,
    caret_height: f32,
    typing: bool,
}

impl TextOverlay {
    /// The text's layout box on screen, for an image drawn at `image` with `zoom`.
    fn screen_box(&self, image: Rect, zoom: f32) -> Rect {
        Rect::from_min_size(
            image.min + self.anchor * zoom,
            self.box_size.max(vec2(0.0, self.caret_height)) * zoom,
        )
    }
}

/// Message shown on the left of the status bar.
#[derive(Default)]
struct Status {
    text: String,
    is_error: bool,
}

pub struct XPaintApp {
    logo: TextureHandle,
    /// A 2×2 checker, repeated across the image to show transparency.
    checkerboard: TextureHandle,
    documents: Vec<Document>,
    active: usize,
    status: Status,
    paste_counter: usize,
    clipboard: clipboard::Writer,
    /// Zoom the active image to fit the canvas on the next frame. Applied while
    /// drawing the canvas, since that is where the viewport size is known.
    fit_requested: bool,
    /// Where the current selection drag started, in image coordinates.
    drag_anchor: Option<(f32, f32)>,
    tool: Tool,
    text_style: TextStyle,
    pending_text: Option<PendingText>,
    shape_style: ShapeStyle,
    drawing: Option<Drawing>,
    pending_shape: Option<PendingShape>,
    show_about: bool,
    show_install_success: bool,
    icon_size: IconSize,
    /// Dev aid: `XPAINT_SCREENSHOT=out.png` saves the app's own frame and quits.
    screenshot_to: Option<PathBuf>,
}

impl XPaintApp {
    pub fn new(cc: &eframe::CreationContext<'_>, files: Vec<PathBuf>) -> Self {
        let mut app = Self::with_context(&cc.egui_ctx, files);
        if let Some(size) = cc
            .storage
            .and_then(|s| s.get_string(ICON_SIZE_KEY))
            .and_then(|name| IconSize::from_name(&name))
        {
            app.icon_size = size;
        }
        app
    }

    fn with_context(ctx: &egui::Context, files: Vec<PathBuf>) -> Self {
        setup_style(ctx);
        egui_extras::install_image_loaders(ctx);

        let logo = image::load_from_memory(ICON_PNG)
            .expect("embedded icon is a valid PNG")
            .to_rgba8();
        let logo = ColorImage::from_rgba_unmultiplied(
            [logo.width() as usize, logo.height() as usize],
            logo.as_raw(),
        );
        let logo = ctx.load_texture(
            "logo",
            logo,
            TextureOptions {
                mipmap_mode: Some(egui::TextureFilter::Linear),
                ..TextureOptions::LINEAR
            },
        );

        let checker = ColorImage::from_rgba_unmultiplied(
            [2, 2],
            &[
                255, 255, 255, 255, 204, 204, 204, 255, 204, 204, 204, 255, 255, 255, 255, 255,
            ],
        );
        let checkerboard = ctx.load_texture(
            "checkerboard",
            checker,
            TextureOptions {
                wrap_mode: egui::TextureWrapMode::Repeat,
                ..TextureOptions::NEAREST
            },
        );

        let mut app = Self {
            logo,
            checkerboard,
            documents: Vec::new(),
            active: 0,
            status: Status::default(),
            paste_counter: 0,
            clipboard: clipboard::Writer::default(),
            fit_requested: false,
            drag_anchor: None,
            tool: Tool::Select,
            text_style: TextStyle::default(),
            pending_text: None,
            shape_style: ShapeStyle::default(),
            drawing: None,
            pending_shape: None,
            show_about: false,
            show_install_success: false,
            icon_size: IconSize::default(),
            screenshot_to: std::env::var_os("XPAINT_SCREENSHOT").map(PathBuf::from),
        };
        for path in files {
            app.open_path(path);
        }
        app
    }

    fn info(&mut self, text: String) {
        self.status = Status {
            text,
            is_error: false,
        };
    }

    fn error(&mut self, text: String) {
        self.status = Status {
            text,
            is_error: true,
        };
    }

    fn add_document(&mut self, doc: Document) {
        self.commit_floating();
        self.info(format!("Opened {}", doc.name));
        self.documents.push(doc);
        self.active = self.documents.len() - 1;
    }

    fn open_path(&mut self, path: PathBuf) {
        match Document::open(&path) {
            Ok(doc) => self.add_document(doc),
            Err(e) => self.error(e),
        }
    }

    fn open_file(&mut self) {
        let path = rfd::FileDialog::new()
            .set_title("Open Image")
            .add_filter("Images", IMAGE_EXTENSIONS)
            .add_filter("All files", &["*"])
            .pick_file();
        if let Some(path) = path {
            self.open_path(path);
        }
    }

    /// Overwrites the image's file. An image that has none yet (e.g. pasted from
    /// the clipboard) asks for a file name, like "Save As".
    fn save(&mut self) {
        self.commit_floating();
        let Some(doc) = self.documents.get_mut(self.active) else {
            return;
        };
        let Some(path) = doc.path.clone() else {
            self.save_as();
            return;
        };
        match doc.save_as(&path) {
            Ok(path) => self.info(format!("Saved {}", path.display())),
            Err(e) => self.error(e),
        }
    }

    fn save_as(&mut self) {
        self.commit_floating();
        let Some(doc) = self.documents.get_mut(self.active) else {
            return;
        };
        let mut dialog = rfd::FileDialog::new()
            .set_title("Save Image As")
            .set_file_name(doc.suggested_file_name());
        if let Some(dir) = doc.path.as_deref().and_then(|p| p.parent()) {
            dialog = dialog.set_directory(dir);
        }
        for (name, extensions) in SAVE_FORMATS {
            dialog = dialog.add_filter(*name, extensions);
        }
        let Some(path) = dialog.save_file() else {
            return;
        };

        match doc.save_as(&path) {
            Ok(path) => self.info(format!("Saved {}", path.display())),
            Err(e) => self.error(e),
        }
    }

    fn new_from_clipboard(&mut self) {
        match clipboard::read_image() {
            Ok(image) => {
                self.paste_counter += 1;
                let name = format!("Pasted {}", self.paste_counter);
                self.add_document(Document::new(name, image));
            }
            Err(e) => self.error(e),
        }
    }

    /// Copies the selection (or the whole image) to the clipboard, including any
    /// floating shape or text, which is applied first.
    fn copy(&mut self) {
        self.commit_floating();
        let Some(doc) = self.documents.get(self.active) else {
            return;
        };
        let image = doc.selected_image();
        let what = if doc.selection.is_some() {
            "selection"
        } else {
            "image"
        };
        match self.clipboard.write_image(&image) {
            Ok(()) => self.info(format!(
                "Copied {what} ({} × {} px) to the clipboard",
                image.width(),
                image.height()
            )),
            Err(e) => self.error(e),
        }
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        self.handle_text_input(ctx);
        self.handle_floating_keys(ctx);
        if !self.is_typing() && !ctx.egui_wants_keyboard_input() {
            for tool in Tool::ALL {
                if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, tool.key())) {
                    self.set_tool(tool);
                }
            }
        }

        if ctx.input_mut(|i| i.consume_shortcut(&OPEN_SHORTCUT)) {
            self.open_file();
        }
        // Ctrl+Shift+S first: egui ignores the extra Shift when matching Ctrl+S.
        if ctx.input_mut(|i| i.consume_shortcut(&SAVE_AS_SHORTCUT)) {
            self.save_as();
        }
        if ctx.input_mut(|i| i.consume_shortcut(&SAVE_SHORTCUT)) {
            self.save();
        }
        if ctx.input_mut(|i| i.consume_shortcut(&PASTE_NEW_SHORTCUT)) {
            self.new_from_clipboard();
        }
        // eframe turns Ctrl+C into `Event::Copy` rather than a key press.
        let copy = !ctx.egui_wants_keyboard_input()
            && ctx.input_mut(|i| {
                let had_event = i.events.iter().any(|e| matches!(e, Event::Copy));
                i.events.retain(|e| !matches!(e, Event::Copy));
                i.consume_shortcut(&COPY_SHORTCUT) || had_event
            });
        if copy {
            self.copy();
        }
        if ctx.input_mut(|i| i.consume_shortcut(&FIT_SHORTCUT)) {
            self.fit_requested = true;
        }
        if ctx.input_mut(|i| i.consume_shortcut(&ACTUAL_SIZE_SHORTCUT))
            && let Some(doc) = self.documents.get_mut(self.active)
        {
            doc.zoom = 1.0;
        }
        if ctx.input_mut(|i| i.consume_shortcut(&CROP_SHORTCUT)) {
            self.crop();
        }
        // Ctrl+Shift+Z first: egui ignores the extra Shift when matching Ctrl+Z.
        let redo = ctx.input_mut(|i| {
            i.consume_shortcut(&REDO_ALT_SHORTCUT) || i.consume_shortcut(&REDO_SHORTCUT)
        });
        if redo {
            self.redo();
        }
        if ctx.input_mut(|i| i.consume_shortcut(&UNDO_SHORTCUT)) {
            self.undo();
        }
        if self.drawing.is_some() && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape))
        {
            self.drawing = None; // cancel the shape being dragged
        }
        let deselect = ctx.input_mut(|i| {
            i.consume_shortcut(&DESELECT_SHORTCUT) || i.consume_key(Modifiers::NONE, Key::Escape)
        });
        if deselect && let Some(doc) = self.documents.get_mut(self.active) {
            doc.selection = None;
        }
    }

    fn has_selection(&self) -> bool {
        self.documents
            .get(self.active)
            .is_some_and(|doc| doc.selection.is_some())
    }

    fn crop(&mut self) {
        self.commit_floating();
        let Some(doc) = self.documents.get_mut(self.active) else {
            return;
        };
        if doc.selection.is_none() {
            return;
        }
        doc.crop_to_selection();
        let text = format!(
            "Cropped {} to {} × {} px",
            doc.name,
            doc.width(),
            doc.height()
        );
        self.info(text);
    }

    fn undo(&mut self) {
        // Undoing right after typing removes the text that was just typed.
        self.commit_floating();
        let Some(doc) = self.documents.get_mut(self.active) else {
            return;
        };
        if let Some(label) = doc.undo() {
            self.drag_anchor = None;
            self.info(format!("Undid {label}"));
        }
    }

    fn redo(&mut self) {
        self.commit_floating();
        let Some(doc) = self.documents.get_mut(self.active) else {
            return;
        };
        if let Some(label) = doc.redo() {
            self.drag_anchor = None;
            self.info(format!("Redid {label}"));
        }
    }

    fn set_tool(&mut self, tool: Tool) {
        self.commit_floating();
        if tool != Tool::Select {
            // A selection only belongs to the select tool. The tool applies to every
            // tab, so clear them all, or switching tabs would bring an old one back.
            for doc in &mut self.documents {
                doc.selection = None;
            }
            self.drag_anchor = None;
        }
        self.drawing = None;
        self.tool = tool;
    }

    /// Applies floating text and shapes to the image. Called before anything else
    /// happens, so that they are only movable until the user does something else.
    fn commit_floating(&mut self) {
        self.commit_text();
        self.commit_shape();
    }

    /// Turns the shape just dragged into a floating, still movable one.
    fn finish_drawing(&mut self) {
        let Some(drawing) = self.drawing.take() else {
            return;
        };
        let pending = PendingShape {
            tool: self.tool,
            drawing,
            offset: Vec2::ZERO,
            grab: None,
        };
        if !pending.primitives(self.shape_style.width).is_empty() {
            self.pending_shape = Some(pending);
        }
    }

    /// Draws the floating shape into the image, as one undoable edit.
    fn commit_shape(&mut self) {
        let Some(pending) = self.pending_shape.take() else {
            return;
        };
        let Some(doc) = self.documents.get_mut(self.active) else {
            return;
        };
        let primitives = pending.primitives(self.shape_style.width);
        let canvas = (doc.width(), doc.height());
        if let Some((image, (x, y))) = shapes::rasterize(&primitives, &self.shape_style, canvas) {
            doc.draw_overlay(pending.tool.name(), &image, x as i64, y as i64);
        }
    }

    /// Draws the text being typed into the image, as one undoable edit.
    fn commit_text(&mut self) {
        let Some(pending) = self.pending_text.take() else {
            return;
        };
        let Some(doc) = self.documents.get_mut(self.active) else {
            return;
        };
        if pending.text.trim().is_empty() {
            return;
        }
        let rendered = match pending.preview {
            Some(p) if p.text == pending.text && p.style == self.text_style => p.rendered,
            _ => text::render(&pending.text, &self.text_style),
        };
        let x = pending.anchor.0 + rendered.origin.0;
        let y = pending.anchor.1 + rendered.origin.1;
        doc.draw_overlay("Text", &rendered.image, x as i64, y as i64);
        self.info("Added text".to_owned());
    }

    /// Feeds typed characters into the pending text and removes those events, so they
    /// do not also trigger shortcuts or activate focused buttons.
    fn handle_text_input(&mut self, ctx: &egui::Context) {
        let Some(pending) = self.pending_text.as_mut().filter(|p| p.typing) else {
            return;
        };
        if ctx.egui_wants_keyboard_input() {
            return; // e.g. typing into the font size field
        }
        let mut stop_typing = false;
        ctx.input_mut(|i| {
            i.events.retain(|event| match event {
                Event::Text(t) => {
                    pending.text.push_str(t);
                    false
                }
                Event::Paste(t) => {
                    pending
                        .text
                        .push_str(&t.replace("\r\n", "\n").replace('\r', "\n"));
                    false
                }
                Event::Key {
                    key,
                    pressed,
                    modifiers,
                    ..
                } if !modifiers.command => match key {
                    Key::Backspace => {
                        if *pressed {
                            pending.text.pop();
                        }
                        false
                    }
                    Key::Enter => {
                        if *pressed {
                            pending.text.push('\n');
                        }
                        false
                    }
                    Key::Escape => {
                        stop_typing |= *pressed;
                        false
                    }
                    Key::Space | Key::Tab => false,
                    _ => true,
                },
                _ => true,
            })
        });
        if stop_typing {
            pending.typing = false;
            if pending.text.trim().is_empty() {
                self.pending_text = None;
            }
        }
    }

    fn is_typing(&self) -> bool {
        self.pending_text.as_ref().is_some_and(|p| p.typing)
    }

    /// Keys for floating text (when not typing) and shapes: nudge, remove or apply.
    fn handle_floating_keys(&mut self, ctx: &egui::Context) {
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        let removed = if let Some(text) = self.pending_text.as_mut().filter(|p| !p.typing) {
            let (dx, dy) = consume_nudge(ctx);
            text.anchor.0 += dx;
            text.anchor.1 += dy;
            "text"
        } else if let Some(shape) = self.pending_shape.as_mut().filter(|s| s.grab.is_none()) {
            let (dx, dy) = consume_nudge(ctx);
            shape.offset += vec2(dx as f32, dy as f32);
            "shape"
        } else {
            return;
        };

        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Delete)) {
            self.pending_text = None;
            self.pending_shape = None;
            self.info(format!("Removed {removed}"));
        } else if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            self.commit_floating();
        }
    }

    fn take_screenshot_when_ready(&mut self, ctx: &egui::Context) {
        let Some(path) = &self.screenshot_to else {
            return;
        };
        if ctx.input(|i| i.time) < 1.0 {
            ctx.request_repaint();
            return;
        }
        let path = path.clone();
        self.screenshot_to = None;
        let callback = egui::ScreenshotCallback::new(move |image: Arc<ColorImage>| {
            let bytes: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_array()).collect();
            let [w, h] = image.size;
            if let Err(e) =
                image::save_buffer(&path, &bytes, w as u32, h as u32, image::ColorType::Rgba8)
            {
                eprintln!("screenshot failed: {e}");
            }
            std::process::exit(0);
        });
        ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(callback));
        ctx.request_repaint();
    }

    // ---- UI sections ----

    fn toolbar(&mut self, ui: &mut Ui) {
        let rect = ui.max_rect();

        // A thin accent line along the top edge separates the toolbar from the window's
        // title bar, which in a dark theme can have the same color.
        let width = 2.0;
        let content = ui.ctx().content_rect();
        ui.painter().hline(
            content.x_range(),
            content.top() + width / 2.0,
            Stroke::new(width, ACCENT),
        );

        // Left: branding and the file actions.
        ui.scope_builder(
            UiBuilder::new()
                .max_rect(rect)
                .layout(Layout::left_to_right(Align::Center)),
            |ui| {
                ui.add(
                    egui::Image::from_texture(&self.logo)
                        .fit_to_exact_size(Vec2::splat(24.0 * icon_scale(ui)))
                        .corner_radius(5.0 * icon_scale(ui)),
                );
                ui.label(RichText::new("xpaint").strong().size(15.0));
                ui.add_space(4.0);
                ui.separator();
                let shortcut = |s| ui.ctx().format_shortcut(s);
                let open_tip = format!("Open image… ({})", shortcut(&OPEN_SHORTCUT));
                let paste_tip = format!(
                    "New image from clipboard ({})",
                    shortcut(&PASTE_NEW_SHORTCUT)
                );
                let save_tip = match self.documents.get(self.active).map(|d| &d.path) {
                    Some(Some(path)) => format!(
                        "Save ({})\nOverwrites {}",
                        shortcut(&SAVE_SHORTCUT),
                        path.display()
                    ),
                    _ => format!(
                        "Save ({})\nNot saved yet: asks for a file name",
                        shortcut(&SAVE_SHORTCUT)
                    ),
                };
                let save_as_tip = format!("Save as… ({})", shortcut(&SAVE_AS_SHORTCUT));
                let crop_tip = format!("Crop image to selection ({})", shortcut(&CROP_SHORTCUT));
                let copy_tip = format!(
                    "Copy selection to clipboard ({})\nCopies the whole image when nothing is selected",
                    shortcut(&COPY_SHORTCUT)
                );

                if icon_button(ui, OPEN_ICON, true, &open_tip).clicked() {
                    self.open_file();
                }
                if icon_button(ui, PASTE_ICON, true, &paste_tip).clicked() {
                    self.new_from_clipboard();
                }
                let has_image = !self.documents.is_empty();
                if icon_button(ui, SAVE_ICON, has_image, &save_tip).clicked() {
                    self.save();
                }
                let save_as = icon_button(ui, SAVE_ICON, has_image, &save_as_tip);
                paint_ellipsis_badge(ui, &save_as);
                if save_as.clicked() {
                    self.save_as();
                }
                ui.separator();
                self.history_buttons(ui);
                ui.separator();

                if icon_button(ui, COPY_ICON, has_image, &copy_tip).clicked() {
                    self.copy();
                }
                let can_crop = self.has_selection();
                let crop_tip = if can_crop {
                    crop_tip
                } else {
                    format!("{crop_tip}\nDrag over the image to select an area first")
                };
                if icon_button(ui, CROP_ICON, can_crop, &crop_tip).clicked() {
                    self.crop();
                }
            },
        );

        // Right: theme, About.
        ui.scope_builder(
            UiBuilder::new()
                .max_rect(rect)
                .layout(Layout::right_to_left(Align::Center)),
            |ui| {
                if icon_button(ui, ABOUT_ICON, true, "About xpaint").clicked() {
                    self.show_about = true;
                }
                let settings = icon_button(ui, SETTINGS_ICON, true, "Settings");
                egui::Popup::menu(&settings).show(|ui| self.settings_menu(ui));
                ui.separator();
                theme_buttons(ui);
            },
        );
    }

    fn settings_menu(&mut self, ui: &mut Ui) {
        ui.label(RichText::new("Icon size").weak());
        for size in IconSize::ALL {
            let label = format!("{} ({:.0}%)", size.name(), size.scale() * 100.0);
            ui.radio_value(&mut self.icon_size, size, label);
        }

        ui.separator();
        ui.label(RichText::new("Desktop integration").weak());
        let color = ui.visuals().text_color();
        let location = match install::Location::user() {
            Ok(location) => location,
            Err(e) => {
                ui.label(RichText::new(e).weak());
                return;
            }
        };
        let tip = format!(
            "Copies xpaint to {} and adds it to the application menu \
             and to \"Open with…\" for images. Only for your user, no root needed.",
            location.binary.display()
        );
        if location.is_installed() {
            // Running another build, e.g. a newly downloaded one: offer to replace
            // the installed binary with it.
            if !location.is_running_installed()
                && ui
                    .add(egui::Button::image_and_text(
                        menu_icon(UPDATE_ICON, color),
                        "Update installation",
                    ))
                    .on_hover_text(&tip)
                    .clicked()
            {
                self.install(&location);
                ui.close();
            }
            if ui
                .add(egui::Button::image_and_text(
                    menu_icon(UNINSTALL_ICON, color),
                    "Uninstall",
                ))
                .on_hover_text("Removes xpaint from the application menu and from your user")
                .clicked()
            {
                match location.uninstall() {
                    Ok(()) => self.info("Uninstalled xpaint".to_owned()),
                    Err(e) => self.error(e),
                }
                ui.close();
            }
        } else if ui
            .add(egui::Button::image_and_text(
                menu_icon(INSTALL_ICON, color),
                "Install for this user",
            ))
            .on_hover_text(&tip)
            .clicked()
        {
            self.install(&location);
            ui.close();
        }
    }

    fn install(&mut self, location: &install::Location) {
        let result = std::env::current_exe()
            .map_err(|e| format!("Could not find the running program: {e}"))
            .and_then(|exe| location.install(&exe));
        match result {
            Ok(()) => {
                self.info("Installation successful".to_owned());
                self.show_install_success = true;
            }
            Err(e) => self.error(e),
        }
    }

    /// Undo and Redo, named after the edit they would revert or reapply.
    fn history_buttons(&mut self, ui: &mut Ui) {
        let doc = self.documents.get(self.active);
        let undo_label = doc.and_then(Document::undo_label);
        let redo_label = doc.and_then(Document::redo_label);
        let ctx = ui.ctx().clone();
        let shortcut = |s| ctx.format_shortcut(s);

        let undo_tip = match undo_label {
            Some(label) => format!("Undo {label} ({})", shortcut(&UNDO_SHORTCUT)),
            None => format!("Undo ({})\nNothing to undo", shortcut(&UNDO_SHORTCUT)),
        };
        if icon_button(ui, UNDO_ICON, undo_label.is_some(), &undo_tip).clicked() {
            self.undo();
        }

        let redo_tip = match redo_label {
            Some(label) => format!("Redo {label} ({})", shortcut(&REDO_SHORTCUT)),
            None => format!("Redo ({})\nNothing to redo", shortcut(&REDO_SHORTCUT)),
        };
        if icon_button(ui, REDO_ICON, redo_label.is_some(), &redo_tip).clicked() {
            self.redo();
        }
    }

    /// The labeled "Open" and "From Clipboard" buttons of the empty state.
    fn file_buttons(&mut self, ui: &mut Ui) {
        let shortcut = |s| ui.ctx().format_shortcut(s);
        let open_tip = format!("Open an image ({})", shortcut(&OPEN_SHORTCUT));
        let paste_tip = format!(
            "Create a new image from the clipboard ({})",
            shortcut(&PASTE_NEW_SHORTCUT)
        );

        let color = ui.visuals().text_color();
        let open = egui::Button::image_and_text(menu_icon(OPEN_ICON, color), "Open…");
        if ui.add(open).on_hover_text(open_tip).clicked() {
            self.open_file();
        }
        let paste = egui::Button::image_and_text(menu_icon(PASTE_ICON, color), "From Clipboard");
        if ui.add(paste).on_hover_text(paste_tip).clicked() {
            self.new_from_clipboard();
        }
    }

    fn tool_panel(&mut self, ui: &mut Ui) {
        ui.vertical_centered(|ui| {
            for tool in Tool::ALL {
                let selected = self.tool == tool;
                let scale = icon_scale(ui);
                let button = egui::Button::selectable(selected, "").min_size(TOOL_BUTTON * scale);
                let response = ui.add(button);
                let color = selectable_icon_color(ui, &response, selected);
                paint_icon(ui, response.rect, tool.icon(), 20.0 * scale, color);
                if response.on_hover_text(tool.tooltip()).clicked() {
                    self.set_tool(tool);
                }
            }
        });
    }

    fn tool_options(&mut self, ui: &mut Ui) {
        ui.horizontal_centered(|ui| {
            ui.label(RichText::new(self.tool.name()).strong());
            ui.separator();
            let hint = match self.tool {
                Tool::Select => "Drag over the image to select an area · Esc to deselect",
                Tool::Brush | Tool::Arrow | Tool::Rectangle | Tool::Ellipse => {
                    let style = &mut self.shape_style;
                    ui.label("Width");
                    ui.add(
                        egui::DragValue::new(&mut style.width)
                            .range(1.0..=200.0)
                            .speed(0.2)
                            .max_decimals(0)
                            .suffix(" px"),
                    )
                    .on_hover_text("Line width in image pixels");
                    ui.color_edit_button_srgba_unmultiplied(&mut style.color)
                        .on_hover_text("Color");
                    ui.separator();
                    match (self.tool, self.pending_shape.is_some()) {
                        (_, true) => {
                            "Drag it or use arrow keys to move · Del to remove · \
                             Esc or Select (S) to apply"
                        }
                        (Tool::Brush, false) => "Drag on the image to paint",
                        (Tool::Arrow, false) => {
                            "Drag from where the line starts to where the arrow points"
                        }
                        (Tool::Rectangle, false) => {
                            "Drag to draw a rectangle · hold Shift for a square"
                        }
                        _ => "Drag to draw an ellipse · hold Shift for a circle",
                    }
                }
                Tool::Text => {
                    let style = &mut self.text_style;
                    ui.label("Size");
                    ui.add(
                        egui::DragValue::new(&mut style.size)
                            .range(4.0..=2000.0)
                            .speed(0.5)
                            .max_decimals(0)
                            .suffix(" px"),
                    )
                    .on_hover_text("Font size in image pixels");
                    ui.toggle_value(&mut style.bold, RichText::new("B").strong())
                        .on_hover_text("Bold");
                    ui.color_edit_button_srgba_unmultiplied(&mut style.color)
                        .on_hover_text("Text color");
                    ui.separator();
                    match &self.pending_text {
                        None => "Click on the image to place text",
                        Some(p) if p.typing => "Type your text · Esc when done",
                        Some(_) => {
                            "Drag or use arrow keys to move · Click to type · \
                             Del to remove · Esc or Select (S) to apply"
                        }
                    }
                }
            };
            // The normal text color: the faint "weak" one is hard to read.
            ui.label(RichText::new(hint).color(ui.visuals().text_color()));
        });
    }

    fn tabs(&mut self, ui: &mut Ui) {
        let mut close = None;
        let mut switch_to = None;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            for (i, doc) in self.documents.iter().enumerate() {
                // The current tab has a button background, bold text and an accent
                // underline; the others only show a background on hover.
                let selected = i == self.active;
                let text = if selected {
                    RichText::new(&doc.name)
                        .strong()
                        .color(ui.visuals().strong_text_color())
                } else {
                    // egui's button text color, which brightens on hover.
                    RichText::new(&doc.name)
                };
                let tab = ui.add(egui::Button::new(text).frame_when_inactive(selected));
                if selected {
                    let underline = tab.rect.shrink2(vec2(6.0, 0.0));
                    let y = underline.bottom() - 1.0;
                    ui.painter()
                        .hline(underline.x_range(), y, Stroke::new(2.0, ACCENT));
                }
                if tab.clicked() && !selected {
                    switch_to = Some(i);
                }
                if tab.middle_clicked() {
                    close = Some(i);
                }
                tab.on_hover_text(match &doc.path {
                    Some(path) => path.display().to_string(),
                    None => "From clipboard".to_owned(),
                });

                let close_button = egui::Button::new("")
                    .frame(false)
                    .min_size(vec2(20.0, ui.spacing().interact_size.y));
                let close_button = ui.add(close_button);
                let color = if close_button.hovered() {
                    ui.visuals().strong_text_color()
                } else {
                    ui.visuals().weak_text_color()
                };
                paint_icon(
                    ui,
                    close_button.rect,
                    CLOSE_ICON,
                    12.0 * icon_scale(ui),
                    color,
                );
                if close_button.on_hover_text("Close").clicked() {
                    close = Some(i);
                }
                ui.add_space(8.0);
            }
        });

        if let Some(i) = switch_to {
            self.commit_floating();
            self.active = i;
        }
        if let Some(i) = close {
            self.commit_floating();
            self.documents.remove(i);
            if self.active >= i && self.active > 0 {
                self.active -= 1;
            }
        }
    }

    fn status_bar(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            let text = RichText::new(&self.status.text);
            ui.label(if self.status.is_error {
                text.color(ui.visuals().error_fg_color)
            } else {
                text.weak()
            });

            let Some(doc) = self.documents.get_mut(self.active) else {
                return;
            };
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let actual_tip = format!(
                    "Zoom to 100% ({})",
                    ui.ctx().format_shortcut(&ACTUAL_SIZE_SHORTCUT)
                );
                if icon_button(ui, ACTUAL_SIZE_ICON, true, &actual_tip).clicked() {
                    doc.zoom = 1.0;
                }
                let fit_tip = format!(
                    "Zoom to fit the window ({})",
                    ui.ctx().format_shortcut(&FIT_SHORTCUT)
                );
                if icon_button(ui, FIT_ICON, true, &fit_tip).clicked() {
                    self.fit_requested = true;
                }
                ui.add(
                    egui::Slider::new(&mut doc.zoom, MIN_ZOOM..=MAX_ZOOM)
                        .logarithmic(true)
                        .custom_formatter(|v, _| format!("{:.0}%", v * 100.0))
                        .custom_parser(|s| {
                            let percent: f64 =
                                s.trim().trim_end_matches('%').trim().parse().ok()?;
                            Some(percent / 100.0)
                        }),
                );
                ui.add(menu_icon(ZOOM_ICON, ui.visuals().weak_text_color()));
                ui.separator();
                ui.label(RichText::new(format!("{} × {} px", doc.width(), doc.height())).weak());
                if let Some(sel) = doc.selection {
                    ui.separator();
                    let text = format!(
                        "Selection {} × {} at {}, {}",
                        sel.width, sel.height, sel.x, sel.y
                    );
                    ui.label(RichText::new(text).weak());
                }
            });
        });
    }

    fn empty_state(&mut self, ui: &mut Ui) {
        let area = Rect::from_center_size(ui.max_rect().center(), vec2(420.0, 240.0));
        ui.scope_builder(
            UiBuilder::new()
                .max_rect(area)
                .layout(Layout::top_down(Align::Center)),
            |ui| {
                ui.add(
                    egui::Image::from_texture(&self.logo)
                        .fit_to_exact_size(Vec2::splat(96.0))
                        .corner_radius(22),
                );
                ui.add_space(10.0);
                ui.label(
                    RichText::new("Open an image or create one from the clipboard")
                        .size(15.0)
                        .color(ui.visuals().text_color()),
                );
                ui.add_space(10.0);
                // A row is only centered if it is given its exact width, which is
                // known after laying it out: remember it and correct on the next frame.
                let width_id = ui.scope_id().with("file_buttons_width");
                let width = ui.data(|d| d.get_temp::<f32>(width_id)).unwrap_or(280.0);
                let measured = ui
                    .allocate_ui_with_layout(
                        vec2(width, ui.spacing().interact_size.y),
                        Layout::left_to_right(Align::Center),
                        |ui| {
                            self.file_buttons(ui);
                            ui.min_rect().width()
                        },
                    )
                    .inner;
                if (measured - width).abs() > 0.5 {
                    ui.data_mut(|d| d.insert_temp(width_id, measured));
                    ui.ctx().request_repaint();
                }
            },
        );
    }

    fn canvas(&mut self, ui: &mut Ui) {
        let style = self.text_style;
        let overlay = self.pending_text.as_mut().map(|pending| {
            let anchor = vec2(pending.anchor.0 as f32, pending.anchor.1 as f32);
            let preview = pending.preview(ui.ctx(), &style);
            let r = &preview.rendered;
            TextOverlay {
                anchor,
                texture: preview.texture.as_ref().map(TextureHandle::id),
                origin: vec2(r.origin.0 as f32, r.origin.1 as f32),
                image_size: vec2(r.image.width() as f32, r.image.height() as f32),
                box_size: vec2(r.size.0, r.size.1),
                caret: vec2(r.caret.0, r.caret.1),
                caret_height: r.caret_height,
                typing: pending.typing,
            }
        });
        let mut overlay = overlay;

        let Some(doc) = self.documents.get_mut(self.active) else {
            self.empty_state(ui);
            return;
        };

        // Ctrl + mouse wheel zooms.
        let zoom_delta = ui.input(|i| i.zoom_delta());
        if zoom_delta != 1.0 && ui.rect_contains_pointer(ui.max_rect()) {
            doc.zoom = (doc.zoom * zoom_delta).clamp(MIN_ZOOM, MAX_ZOOM);
        }

        let fit_requested = std::mem::take(&mut self.fit_requested);
        let fit_when_shown = std::mem::take(&mut doc.fit_when_shown);
        if fit_requested || fit_when_shown {
            // Leave a pixel of slack so rounding never triggers the scrollbars.
            let viewport = ui.available_size() - Vec2::splat(1.0);
            let fit = (viewport.x / doc.width() as f32).min(viewport.y / doc.height() as f32);
            // A newly shown image is only shrunk, never enlarged past 100%.
            let fit = if fit_requested { fit } else { fit.min(1.0) };
            doc.zoom = fit.clamp(MIN_ZOOM, MAX_ZOOM);
        }

        let image_size = Vec2::new(doc.width() as f32, doc.height() as f32) * doc.zoom;
        let texture_id = doc.texture(ui.ctx()).id();
        let shadow = ui.visuals().window_shadow;
        let drag_anchor = &mut self.drag_anchor;
        let tool = self.tool;
        let pending = &mut self.pending_text;
        let mut text_click = None;
        let drawing = &mut self.drawing;
        let pending_shape = &mut self.pending_shape;
        let shape_style = self.shape_style;
        let checkerboard = self.checkerboard.id();
        let mut finish_drawing = false;
        let mut commit_previous_shape = false;

        // Dragging the mouse selects, so the scroll area must not scroll on drag.
        let scroll_source = egui::scroll_area::ScrollSource {
            scroll_bar: true,
            mouse_wheel: true,
            ..egui::scroll_area::ScrollSource::NONE
        };
        egui::ScrollArea::both()
            .auto_shrink(false)
            .scroll_source(scroll_source)
            .show(ui, |ui| {
                // Center the image when it is smaller than the viewport.
                let available = ui.available_size();
                let (outer, response) =
                    ui.allocate_exact_size(image_size.max(available), Sense::click_and_drag());
                let rect = Rect::from_center_size(outer.center(), image_size);

                let to_image = |pos: Pos2| {
                    (
                        (pos.x - rect.min.x) / doc.zoom,
                        (pos.y - rect.min.y) / doc.zoom,
                    )
                };
                match tool {
                    Tool::Select => {
                        if response.drag_started() {
                            // Start at the press position, not where the drag threshold was crossed.
                            let origin = ui.input(|i| i.pointer.press_origin());
                            *drag_anchor = origin.map(to_image);
                        }
                        if response.dragged()
                            && let (Some(anchor), Some(pos)) =
                                (*drag_anchor, response.interact_pointer_pos())
                        {
                            doc.select_between(anchor, to_image(pos));
                        }
                        if response.drag_stopped() {
                            *drag_anchor = None;
                        }
                        if response.clicked() {
                            doc.selection = None;
                        }
                        response.on_hover_cursor(CursorIcon::Crosshair);
                    }
                    Tool::Text => {
                        // A little slack around the text makes it easier to grab.
                        let grab_box = overlay
                            .as_ref()
                            .map(|o| o.screen_box(rect, doc.zoom).expand(6.0));
                        let on_text = |pos: Pos2| grab_box.is_some_and(|b| b.contains(pos));

                        if response.drag_started()
                            && let Some(origin) = ui.input(|i| i.pointer.press_origin())
                            && on_text(origin)
                            && let Some(p) = pending.as_mut()
                        {
                            let (x, y) = to_image(origin);
                            p.grab = Some(vec2(x - p.anchor.0 as f32, y - p.anchor.1 as f32));
                        }
                        if response.dragged()
                            && let Some(p) = pending.as_mut()
                            && let (Some(grab), Some(pos)) =
                                (p.grab, response.interact_pointer_pos())
                        {
                            let (x, y) = to_image(pos);
                            p.anchor = ((x - grab.x).round() as i32, (y - grab.y).round() as i32);
                        }
                        if response.drag_stopped()
                            && let Some(p) = pending.as_mut()
                        {
                            p.grab = None;
                        }
                        if response.clicked()
                            && let Some(pos) = response.interact_pointer_pos()
                        {
                            match pending.as_mut() {
                                Some(p) if on_text(pos) => p.typing = true,
                                _ => text_click = Some((rect.contains(pos), to_image(pos))),
                            }
                        }

                        let cursor = if pending.as_ref().is_some_and(|p| p.grab.is_some()) {
                            CursorIcon::Grabbing
                        } else if response.hover_pos().is_some_and(on_text) {
                            CursorIcon::Move
                        } else {
                            CursorIcon::Text
                        };
                        response.on_hover_cursor(cursor);

                        // Draw at the position after this frame's move, not the old one.
                        if let (Some(o), Some(p)) = (overlay.as_mut(), pending.as_ref()) {
                            o.anchor = vec2(p.anchor.0 as f32, p.anchor.1 as f32);
                            o.typing = p.typing;
                        }
                    }
                    Tool::Brush | Tool::Arrow | Tool::Rectangle | Tool::Ellipse => {
                        // Use the raw button state instead of egui's drag detection:
                        // a drag is only recognized after the mouse moved a few points,
                        // which makes painting feel laggy.
                        let down = response.is_pointer_button_down_on();
                        let pressed = down && ui.input(|i| i.pointer.primary_pressed());
                        let shift = ui.input(|i| i.modifiers.shift);
                        // Every position the mouse reported this frame, not just the
                        // last one, so that fast strokes stay smooth.
                        let mut moves: Vec<Pos2> = ui.input(|i| {
                            i.events
                                .iter()
                                .filter_map(|e| match e {
                                    Event::PointerMoved(p) => Some(*p),
                                    _ => None,
                                })
                                .collect()
                        });
                        if moves.is_empty() {
                            moves.extend(response.interact_pointer_pos());
                        }

                        // Whether pressing at `pos` grabs the floating shape to move it.
                        let width = shape_style.width;
                        let grabs = |pending: &PendingShape, pos: Pos2| {
                            let slack = 6.0 / doc.zoom;
                            shapes::hit_test(
                                &pending.primitives(width),
                                to_image(pos),
                                width,
                                slack,
                            )
                        };

                        if pressed {
                            let origin = ui
                                .input(|i| i.pointer.press_origin())
                                .or(response.interact_pointer_pos())
                                .unwrap_or(rect.min);
                            match pending_shape.as_mut() {
                                Some(p) if grabs(p, origin) => {
                                    let (x, y) = to_image(origin);
                                    p.grab = Some(vec2(x, y) - p.offset);
                                }
                                _ => {
                                    commit_previous_shape = pending_shape.is_some();
                                    *drawing = Some(Drawing {
                                        points: vec![to_image(origin)],
                                        constrain: shift,
                                    });
                                }
                            }
                        }

                        if let Some(p) = pending_shape.as_mut()
                            && let Some(grab) = p.grab
                        {
                            if let Some(pos) = moves.last() {
                                let (x, y) = to_image(*pos);
                                p.offset = (vec2(x, y) - grab).round();
                            }
                            if !down {
                                p.grab = None;
                            }
                        }

                        if let Some(d) = drawing.as_mut() {
                            d.constrain = shift;
                            if down {
                                for pos in &moves {
                                    let p = to_image(*pos);
                                    let last = *d.points.last().expect("starts with a point");
                                    if tool == Tool::Brush {
                                        // The last point follows the mouse; it is only kept
                                        // once it is a few screen pixels from the one before.
                                        // Closer points just add the hand's jitter, which
                                        // shows as a zigzag edge when zoomed in.
                                        let n = d.points.len();
                                        let keep_last = n == 1 || {
                                            let before = d.points[n - 2];
                                            let dist = (last.0 - before.0).hypot(last.1 - before.1);
                                            dist >= BRUSH_POINT_SPACING / doc.zoom
                                        };
                                        if keep_last {
                                            d.points.push(p);
                                        } else {
                                            d.points[n - 1] = p;
                                        }
                                    } else if d.points.len() == 1 {
                                        d.points.push(p);
                                    } else {
                                        d.points[1] = p;
                                    }
                                }
                            } else {
                                finish_drawing = true;
                            }
                        }

                        let hovered = response.hover_pos();
                        let cursor = if pending_shape.as_ref().is_some_and(|p| p.grab.is_some()) {
                            CursorIcon::Grabbing
                        } else if let (Some(p), Some(pos)) = (pending_shape.as_ref(), hovered)
                            && grabs(p, pos)
                        {
                            CursorIcon::Move
                        } else {
                            CursorIcon::Crosshair
                        };
                        response.on_hover_cursor(cursor);
                    }
                }

                let painter = ui.painter();
                painter.add(shadow.as_shape(rect, CornerRadius::ZERO));
                paint_checkerboard(painter, rect, checkerboard);
                painter.image(
                    texture_id,
                    rect,
                    Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                    Color32::WHITE,
                );
                if let Some(sel) = doc.selection {
                    let min = rect.min + vec2(sel.x as f32, sel.y as f32) * doc.zoom;
                    let size = vec2(sel.width as f32, sel.height as f32) * doc.zoom;
                    paint_selection(painter, rect, Rect::from_min_size(min, size));
                }
                if let Some(overlay) = &overlay {
                    paint_text_overlay(ui, rect, doc.zoom, overlay, CANVAS_ACCENT);
                }
                if let Some(pending) = pending_shape.as_ref() {
                    let primitives = pending.primitives(shape_style.width);
                    paint_shape_preview(ui, rect, doc.zoom, &primitives, &shape_style);
                    if let Some(screen_box) = pending.screen_box(rect, doc.zoom, shape_style.width)
                    {
                        paint_dashed_rect(ui.painter(), screen_box);
                    }
                }
                if let Some(shape) = drawing.as_ref().and_then(|d| d.shape(tool)) {
                    let primitives = shape.primitives(shape_style.width);
                    paint_shape_preview(ui, rect, doc.zoom, &primitives, &shape_style);
                }
            });

        // Starting a new shape applies the previous one first.
        if commit_previous_shape {
            self.commit_shape();
        }
        if finish_drawing {
            self.finish_drawing();
        }

        // Clicking with the text tool finishes the current text and, on the image,
        // starts a new one there.
        if let Some((on_image, (x, y))) = text_click {
            self.commit_floating();
            if on_image {
                self.pending_text = Some(PendingText::new((x.floor() as i32, y.floor() as i32)));
            }
        }
    }

    fn about_window(&mut self, ctx: &egui::Context) {
        let logo = self.logo.clone();
        egui::Window::new("About")
            .open(&mut self.show_about)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(4.0);
                    ui.add(
                        egui::Image::from_texture(&logo)
                            .fit_to_exact_size(Vec2::splat(96.0))
                            .corner_radius(22),
                    );
                    ui.add_space(6.0);
                    ui.label(RichText::new("xpaint").size(20.0).strong());
                    ui.label(RichText::new(concat!("Version ", env!("CARGO_PKG_VERSION"))).weak());
                    ui.add_space(6.0);
                    ui.label("A simple image editor written in Rust with egui.");
                    ui.add_space(4.0);
                });
            });
    }
}

impl eframe::App for XPaintApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        set_icon_scale(&ctx, self.icon_size.scale());
        self.handle_shortcuts(&ctx);

        let style = ui.style().clone();
        let scale = self.icon_size.scale();
        let panel = Frame::side_top_panel(&style);

        egui::Panel::top("toolbar")
            .frame(panel.inner_margin(Margin::symmetric(12, 8)))
            .exact_size(TOOLBAR_HEIGHT * scale)
            .show(ui, |ui| self.toolbar(ui));
        egui::Panel::top("tool_options")
            .frame(panel.inner_margin(Margin::symmetric(12, 4)))
            .exact_size(36.0)
            .show(ui, |ui| self.tool_options(ui));
        egui::Panel::bottom("status")
            .frame(panel.inner_margin(Margin::symmetric(12, 6)))
            .show(ui, |ui| self.status_bar(ui));
        egui::Panel::left("tools")
            .frame(panel.inner_margin(Margin::symmetric(6, 8)))
            .exact_size(TOOL_BUTTON.x * scale + 12.0)
            .resizable(false)
            .show(ui, |ui| self.tool_panel(ui));
        if !self.documents.is_empty() {
            egui::Panel::top("tabs")
                .frame(panel.inner_margin(Margin::symmetric(12, 4)))
                .show(ui, |ui| self.tabs(ui));
        }

        let central = if self.documents.is_empty() {
            Frame::central_panel(&style).inner_margin(Margin::same(16))
        } else {
            Frame::central_panel(&style)
                .inner_margin(0)
                .fill(ui.visuals().extreme_bg_color)
        };
        egui::CentralPanel::default()
            .frame(central)
            .show(ui, |ui| self.canvas(ui));

        self.about_window(&ctx);
        if self.show_install_success {
            let modal = egui::Modal::new(egui::Id::unique("install_success")).show(&ctx, |ui| {
                ui.set_max_width(360.0);
                ui.heading("Installation successful");
                ui.label(
                    "You can now close xpaint and reopen it from the start menu (application menu).",
                );
                ui.add_space(8.0);
                ui.button("OK").clicked()
            });
            if modal.inner || modal.should_close() {
                self.show_install_success = false;
            }
        }
        self.take_screenshot_when_ready(&ctx);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string(ICON_SIZE_KEY, self.icon_size.name().to_owned());
    }
}

// ---- helpers ----

fn setup_style(ctx: &egui::Context) {
    ctx.all_styles_mut(|style| {
        let spacing = &mut style.spacing;
        spacing.item_spacing = vec2(8.0, 6.0);
        spacing.button_padding = vec2(10.0, 4.0);
        spacing.interact_size.y = 26.0;
        spacing.window_margin = Margin::same(16);

        // A desktop app: no selectable text (and no text cursor over it), instant tooltips.
        let interaction = &mut style.interaction;
        interaction.selectable_labels = false;
        interaction.tooltip_delay = 0.0;
        interaction.show_tooltips_only_when_still = false;

        let visuals = &mut style.visuals;
        let radius = CornerRadius::same(6);
        visuals.widgets.noninteractive.corner_radius = radius;
        visuals.widgets.inactive.corner_radius = radius;
        visuals.widgets.hovered.corner_radius = radius;
        visuals.widgets.active.corner_radius = radius;
        visuals.widgets.open.corner_radius = radius;
        visuals.window_corner_radius = CornerRadius::same(10);
        // Selected widgets and selected text: white on the accent.
        visuals.selection.bg_fill = ACCENT;
        visuals.selection.stroke.color = Color32::WHITE;
    });
}

fn icon_scale_id() -> egui::Id {
    egui::Id::unique("xpaint_icon_scale")
}

/// Makes the icon size setting's scale available to the icon helpers below. Set at the
/// start of every frame.
fn set_icon_scale(ctx: &egui::Context, scale: f32) {
    ctx.data_mut(|d| d.insert_temp(icon_scale_id(), scale));
}

/// The factor icons and their buttons are scaled by, from the icon size setting.
fn icon_scale(ui: &Ui) -> f32 {
    ui.data(|d| d.get_temp(icon_scale_id())).unwrap_or(1.0)
}

/// A fixed-size button showing only `icon`, with `tooltip` on hover.
fn icon_button(ui: &mut Ui, icon: ImageSource, enabled: bool, tooltip: &str) -> egui::Response {
    let scale = icon_scale(ui);
    let button = egui::Button::new("").min_size(ICON_BUTTON * scale);
    let response = ui.add_enabled(enabled, button);
    let mut color = ui.style().interact(&response).fg_stroke.color;
    if !enabled {
        color = color.gamma_multiply(ui.visuals().disabled_alpha());
    }
    paint_icon(ui, response.rect, icon, 18.0 * scale, color);
    response
        .on_hover_text(tooltip)
        .on_disabled_hover_text(tooltip)
}

/// Draws `icon` at `size` points, centered in `rect`, in `color`.
fn paint_icon(ui: &Ui, rect: Rect, icon: ImageSource, size: f32, color: Color32) {
    egui::Image::new(icon)
        .tint(color)
        .paint_at(ui, Rect::from_center_size(rect.center(), Vec2::splat(size)));
}

/// Icon color for a selectable button. When selected it is white, which stands out
/// on the accent background better than egui's accent-colored text does.
fn selectable_icon_color(ui: &Ui, response: &egui::Response, selected: bool) -> Color32 {
    if selected {
        Color32::WHITE
    } else {
        ui.style()
            .interact_selectable(response, false)
            .fg_stroke
            .color
    }
}

/// `icon` as a 16 point image in `color`, to sit next to text. It stays at that size
/// whatever the icon size setting, like the text it goes with.
fn menu_icon(icon: ImageSource, color: Color32) -> egui::Image {
    egui::Image::new(icon)
        .tint(color)
        .fit_to_exact_size(Vec2::splat(16.0))
}

/// Buttons for following the system theme or picking dark or light, like
/// [`egui::global_theme_preference_buttons`] but with xpaint's icons.
fn theme_buttons(ui: &mut Ui) {
    use egui::ThemePreference;
    let mut preference = ui.options(|o| o.theme_preference);
    // A scope keeps the toolbar's centered row, so the buttons line up with the others
    // at every icon size (a nested `horizontal` row is only as tall as text).
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        // Right to left, so the order on screen is System, Dark, Light.
        for (choice, icon, tip) in [
            (
                ThemePreference::Light,
                THEME_LIGHT_ICON,
                "Use the light theme",
            ),
            (ThemePreference::Dark, THEME_DARK_ICON, "Use the dark theme"),
            (
                ThemePreference::System,
                THEME_SYSTEM_ICON,
                "Follow the system theme",
            ),
        ] {
            let selected = preference == choice;
            let scale = icon_scale(ui);
            let button = egui::Button::selectable(selected, "").min_size(ICON_BUTTON * scale);
            let response = ui.add(button);
            let color = selectable_icon_color(ui, &response, selected);
            paint_icon(ui, response.rect, icon, 16.0 * scale, color);
            if response.on_hover_text(tip).clicked() {
                preference = choice;
            }
        }
    });
    ui.ctx().set_theme(preference);
}

/// Marks a button as "…" (asks for more, like a file name) with three dots in its
/// bottom right corner, on a small backing so they show over the icon.
fn paint_ellipsis_badge(ui: &Ui, button: &egui::Response) {
    let visuals = ui.style().interact(button);
    let mut color = visuals.fg_stroke.color;
    let mut backing = visuals.weak_bg_fill;
    if !button.enabled() {
        color = color.gamma_multiply(ui.visuals().disabled_alpha());
        backing = backing.gamma_multiply(ui.visuals().disabled_alpha());
    }
    let center = button.rect.right_bottom() - vec2(9.0, 7.0);
    ui.painter().rect_filled(
        Rect::from_center_size(center, vec2(14.0, 6.0)),
        3.0,
        backing,
    );
    for dx in [-4.0, 0.0, 4.0] {
        ui.painter()
            .circle_filled(center + vec2(dx, 0.0), 1.3, color);
    }
}

/// Turns sharper than this (cos of ~15°) get a round joint in the preview.
const SHARP_TURN_COS: f32 = 0.966;

/// Draws a shape that is being dragged with egui, from the same primitives that
/// [`shapes::rasterize`] draws into the image on release.
fn paint_shape_preview(
    ui: &Ui,
    image: Rect,
    zoom: f32,
    primitives: &[Primitive],
    style: &ShapeStyle,
) {
    let [r, g, b, a] = style.color;
    let color = Color32::from_rgba_unmultiplied(r, g, b, a);
    let width = style.width * zoom;
    let to_screen = |&(x, y): &shapes::Point| image.min + vec2(x, y) * zoom;
    // Clip to the image, like the committed shape will be.
    let painter = ui.painter().with_clip_rect(image.intersect(ui.clip_rect()));

    for primitive in primitives {
        match primitive {
            Primitive::Line { points, closed } => {
                let points: Vec<Pos2> = points.iter().map(to_screen).collect();
                let stroke = Stroke::new(width, color);
                if *closed {
                    painter.add(Shape::closed_line(points, stroke));
                } else {
                    // Round ends and round joints where the line turns sharply, like the
                    // rasterized line. egui joins line pieces with pointed corners, which
                    // would stick out as spikes there.
                    for end in [points[0], points[points.len() - 1]] {
                        painter.circle_filled(end, width / 2.0, color);
                    }
                    for w in points.windows(3) {
                        let (a, b) = ((w[1] - w[0]).normalized(), (w[2] - w[1]).normalized());
                        if a.dot(b) < SHARP_TURN_COS {
                            painter.circle_filled(w[1], width / 2.0, color);
                        }
                    }
                    painter.add(Shape::line(points, stroke));
                }
            }
            Primitive::Polygon(points) => {
                let points = points.iter().map(to_screen).collect();
                painter.add(Shape::convex_polygon(points, color, Stroke::NONE));
            }
            Primitive::Dot { center, radius } => {
                painter.circle_filled(to_screen(center), radius * zoom, color);
            }
        }
    }
}

/// Draws text that is not committed yet: its pixels, a dashed box around it and,
/// while typing, a blinking caret.
fn paint_text_overlay(ui: &Ui, image: Rect, zoom: f32, overlay: &TextOverlay, accent: Color32) {
    let to_screen = |p: Vec2| image.min + p * zoom;
    // Clip to the image, like the committed text will be.
    let painter = ui.painter().with_clip_rect(image.intersect(ui.clip_rect()));

    if let Some(texture) = overlay.texture {
        let min = to_screen(overlay.anchor + overlay.origin);
        painter.image(
            texture,
            Rect::from_min_size(min, overlay.image_size * zoom),
            Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
            Color32::WHITE,
        );
    }

    paint_dashed_rect(ui.painter(), overlay.screen_box(image, zoom).expand(3.0));
    if !overlay.typing {
        return;
    }

    let time = ui.input(|i| i.time);
    if (time * 2.0) as i64 % 2 == 0 {
        let top = to_screen(overlay.anchor + overlay.caret);
        let bottom = top + vec2(0.0, overlay.caret_height * zoom);
        painter.line_segment([top, bottom], Stroke::new(2.0, accent));
    }
    ui.ctx().request_repaint_after(Duration::from_millis(250));
}

/// Outlines `rect` with a black and white dashed line that shows on any background.
fn paint_dashed_rect(painter: &egui::Painter, rect: Rect) {
    painter.rect_stroke(
        rect,
        0.0,
        Stroke::new(1.0, Color32::BLACK),
        StrokeKind::Middle,
    );
    let outline = [
        rect.left_top(),
        rect.right_top(),
        rect.right_bottom(),
        rect.left_bottom(),
        rect.left_top(),
    ];
    painter.extend(Shape::dashed_line(
        &outline,
        Stroke::new(1.0, Color32::WHITE),
        4.0,
        4.0,
    ));
}

/// Dims the image outside `selection` and outlines it with a dashed line.
fn paint_selection(painter: &egui::Painter, image: Rect, selection: Rect) {
    let dim = Color32::from_black_alpha(110);
    let above = Rect::from_min_max(image.min, pos2(image.max.x, selection.min.y));
    let below = Rect::from_min_max(pos2(image.min.x, selection.max.y), image.max);
    let left = Rect::from_min_max(pos2(image.min.x, selection.min.y), selection.left_bottom());
    let right = Rect::from_min_max(selection.right_top(), pos2(image.max.x, selection.max.y));
    for r in [above, below, left, right] {
        if r.is_positive() {
            painter.rect_filled(r, 0.0, dim);
        }
    }
    paint_dashed_rect(painter, selection);
}

/// Paints a transparency checkerboard over `rect` as one quad that repeats the 2×2
/// `checkerboard` texture, instead of a rectangle per cell.
fn paint_checkerboard(painter: &egui::Painter, rect: Rect, checkerboard: TextureId) {
    let repeats = rect.size() / (2.0 * CHECKER_SIZE);
    let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(repeats.x, repeats.y));
    painter.image(checkerboard, rect, uv, Color32::WHITE);
}

/// Consumes the arrow keys pressed this frame and returns how far they move a
/// floating item: 1 px per press, 10 px with Shift.
fn consume_nudge(ctx: &egui::Context) -> (i32, i32) {
    let step = if ctx.input(|i| i.modifiers.shift) {
        10
    } else {
        1
    };
    let mut total = (0, 0);
    for (key, dx, dy) in [
        (Key::ArrowLeft, -step, 0),
        (Key::ArrowRight, step, 0),
        (Key::ArrowUp, 0, -step),
        (Key::ArrowDown, 0, step),
    ] {
        while ctx.input_mut(|i| i.consume_key(Modifiers::NONE, key)) {
            total.0 += dx;
            total.1 += dy;
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(key: Key, pressed: bool) -> Event {
        Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: Modifiers::NONE,
        }
    }

    /// Runs one frame of shortcut and text handling with the given input events.
    fn frame(app: &mut XPaintApp, ctx: &egui::Context, events: Vec<Event>) {
        let input = egui::RawInput {
            events,
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| app.handle_shortcuts(ui.ctx()));
        output.textures_delta.clear(); // there is no renderer to upload textures to
    }

    #[test]
    fn typing_with_text_tool_draws_text_on_finish() {
        let ctx = egui::Context::default();
        let mut app = XPaintApp::with_context(&ctx, Vec::new());
        let white = RgbaImage::from_pixel(200, 100, image::Rgba([255, 255, 255, 255]));
        app.add_document(Document::new("test", white.clone()));

        // "T" switches to the text tool; then place the text as a canvas click would.
        frame(&mut app, &ctx, vec![key(Key::T, true), key(Key::T, false)]);
        assert!(app.tool == Tool::Text);
        app.pending_text = Some(PendingText::new((10, 10)));

        // Typing "S" must insert text, not switch to the select tool.
        let typing = vec![
            Event::Text("Sx".into()),
            key(Key::S, true),
            key(Key::Backspace, true),
            key(Key::Enter, true),
            Event::Text("y".into()),
        ];
        frame(&mut app, &ctx, typing);
        assert!(app.tool == Tool::Text);
        assert_eq!(app.pending_text.as_ref().unwrap().text, "S\ny");
        assert_eq!(
            app.documents[0].image, white,
            "nothing drawn before finishing"
        );

        // Esc stops typing, but the text stays selected and is not drawn yet.
        frame(&mut app, &ctx, vec![key(Key::Escape, true)]);
        assert!(app.pending_text.as_ref().is_some_and(|p| !p.typing));
        assert_eq!(app.documents[0].image, white);

        // Switching to the select tool applies it.
        frame(&mut app, &ctx, vec![key(Key::S, true)]);
        assert!(app.tool == Tool::Select);
        assert!(app.pending_text.is_none());
        let doc = &app.documents[0];
        assert_ne!(doc.image, white, "text was drawn");
        assert_eq!(doc.undo_label(), Some("Text"));
        assert!(
            doc.image.pixels().any(|p| p.0 == [0, 0, 0, 255]),
            "default color is black"
        );

        app.undo();
        assert_eq!(app.documents[0].image, white);
    }

    #[test]
    fn empty_text_is_discarded() {
        let ctx = egui::Context::default();
        let mut app = XPaintApp::with_context(&ctx, Vec::new());
        app.add_document(Document::new("test", RgbaImage::new(50, 50)));
        app.pending_text = Some(PendingText {
            text: "  ".into(),
            ..PendingText::new((0, 0))
        });
        app.set_tool(Tool::Select);
        assert!(app.pending_text.is_none());
        assert_eq!(app.documents[0].undo_label(), None);
    }

    #[test]
    fn selected_text_can_be_moved_restyled_and_removed() {
        let ctx = egui::Context::default();
        let mut app = XPaintApp::with_context(&ctx, Vec::new());
        let white = RgbaImage::from_pixel(300, 100, image::Rgba([255, 255, 255, 255]));
        app.add_document(Document::new("test", white.clone()));
        app.tool = Tool::Text;
        app.pending_text = Some(PendingText {
            text: "Hi".into(),
            typing: false,
            ..PendingText::new((10, 10))
        });

        // Arrow keys nudge by 1 px, with Shift by 10 px.
        frame(
            &mut app,
            &ctx,
            vec![key(Key::ArrowRight, true), key(Key::ArrowDown, true)],
        );
        assert_eq!(app.pending_text.as_ref().unwrap().anchor, (11, 11));
        let shift_left = Event::Key {
            key: Key::ArrowLeft,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::SHIFT,
        };
        frame(
            &mut app,
            &ctx,
            vec![Event::ModifiersChanged(Modifiers::SHIFT), shift_left],
        );
        assert_eq!(app.pending_text.as_ref().unwrap().anchor, (1, 11));

        // Style changes apply to the selected text when it is committed.
        app.text_style.color = [0, 0, 255, 255];
        frame(&mut app, &ctx, vec![key(Key::Escape, true)]);
        assert!(app.pending_text.is_none());
        let image = &app.documents[0].image;
        assert!(
            image.pixels().any(|p| p.0 == [0, 0, 255, 255]),
            "drawn in blue"
        );
        assert!(
            !image.pixels().any(|p| p.0 == [0, 0, 0, 255]),
            "not in the old black"
        );
        assert!(
            (0..100).all(|y| image.get_pixel(0, y).0 == [255; 4]),
            "moved text starts right of x = 1"
        );

        // Del removes selected text without drawing it.
        app.undo();
        app.pending_text = Some(PendingText {
            text: "Bye".into(),
            typing: false,
            ..PendingText::new((10, 10))
        });
        frame(&mut app, &ctx, vec![key(Key::Delete, true)]);
        assert!(app.pending_text.is_none());
        assert_eq!(app.documents[0].image, white);
    }

    #[test]
    fn dragging_selected_text_moves_it() {
        let ctx = egui::Context::default();
        let mut app = XPaintApp::with_context(&ctx, Vec::new());
        app.add_document(Document::new("test", RgbaImage::new(300, 100)));
        app.tool = Tool::Text;
        app.pending_text = Some(PendingText {
            text: "Hi".into(),
            typing: false,
            ..PendingText::new((10, 10))
        });

        let screen = Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0));
        let mut canvas_frame = |events: Vec<Event>| {
            let input = egui::RawInput {
                screen_rect: Some(screen),
                events,
                ..Default::default()
            };
            let mut output = ctx.run_ui(input, |ui| app.canvas(ui));
            output.textures_delta.clear();
        };
        let button = |pos: Pos2, pressed| Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };

        // The 300×100 image is centered, so its top left is at (250, 250) and the
        // text's anchor at (260, 260). Grab the text a bit inside and drag it.
        let start = pos2(265.0, 270.0);
        let end = start + vec2(50.0, 20.0);
        canvas_frame(vec![Event::PointerMoved(start)]);
        canvas_frame(vec![button(start, true)]);
        canvas_frame(vec![Event::PointerMoved(start + vec2(25.0, 10.0))]);
        canvas_frame(vec![Event::PointerMoved(end)]);
        canvas_frame(vec![button(end, false)]);

        let pending = app
            .pending_text
            .as_ref()
            .expect("still selected after the move");
        assert_eq!(pending.anchor, (60, 30));
        assert!(pending.grab.is_none());
        assert!(!pending.typing, "a drag does not start typing");
        assert_eq!(app.documents[0].undo_label(), None, "nothing committed");
    }

    #[test]
    fn switching_away_from_select_clears_selections() {
        let ctx = egui::Context::default();
        let mut app = XPaintApp::with_context(&ctx, Vec::new());
        for name in ["a", "b"] {
            let mut doc = Document::new(name, RgbaImage::new(100, 100));
            doc.select_between((10.0, 10.0), (50.0, 50.0));
            app.add_document(doc);
        }

        // Re-selecting the select tool keeps the selection.
        app.set_tool(Tool::Select);
        assert!(app.documents.iter().all(|d| d.selection.is_some()));

        frame(&mut app, &ctx, vec![key(Key::T, true)]);
        assert!(app.tool == Tool::Text);
        assert!(app.documents.iter().all(|d| d.selection.is_none()));
    }

    /// Runs one frame of the canvas alone, in an 800×600 window. A 300×100 image is
    /// centered in it, so image pixel (x, y) is at screen (250 + x, 250 + y).
    fn canvas_frame(app: &mut XPaintApp, ctx: &egui::Context, events: Vec<Event>) {
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0))),
            events,
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| {
            app.handle_shortcuts(ui.ctx());
            app.canvas(ui);
        });
        output.textures_delta.clear();
    }

    fn screen(x: f32, y: f32) -> Pos2 {
        pos2(250.0 + x, 250.0 + y)
    }

    fn button(pos: Pos2, pressed: bool, modifiers: Modifiers) -> Event {
        Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers,
        }
    }

    /// Drags from image point `from` to `to` in a few steps, with `modifiers` held.
    fn drag(app: &mut XPaintApp, ctx: &egui::Context, from: Pos2, to: Pos2, modifiers: Modifiers) {
        let (from, to) = (screen(from.x, from.y), screen(to.x, to.y));
        canvas_frame(
            app,
            ctx,
            vec![
                Event::ModifiersChanged(modifiers),
                Event::PointerMoved(from),
            ],
        );
        canvas_frame(app, ctx, vec![button(from, true, modifiers)]);
        for t in [0.3, 0.6, 1.0] {
            canvas_frame(app, ctx, vec![Event::PointerMoved(from + (to - from) * t)]);
        }
        canvas_frame(app, ctx, vec![button(to, false, modifiers)]);
    }

    fn drawing_app(tool: Tool) -> (XPaintApp, egui::Context) {
        let ctx = egui::Context::default();
        let mut app = XPaintApp::with_context(&ctx, Vec::new());
        let white = RgbaImage::from_pixel(300, 100, image::Rgba([255, 255, 255, 255]));
        app.add_document(Document::new("test", white));
        app.tool = tool;
        app.shape_style = ShapeStyle {
            color: [255, 0, 0, 255],
            width: 4.0,
        };
        (app, ctx)
    }

    /// Checks that the shape just drawn floats (nothing drawn yet), then applies it.
    fn apply_floating_shape(app: &mut XPaintApp) {
        assert!(app.drawing.is_none());
        assert!(
            app.pending_shape.is_some(),
            "the shape floats after the drag"
        );
        let image_before = app.documents[0].image.clone();
        app.commit_floating();
        assert!(app.pending_shape.is_none());
        assert!(
            app.documents[0].undo_label().is_some(),
            "applying adds an undo step"
        );
        assert_ne!(app.documents[0].image, image_before, "applying draws it");
    }

    fn is_red(app: &XPaintApp, x: u32, y: u32) -> bool {
        app.documents[0].image.get_pixel(x, y).0 == [255, 0, 0, 255]
    }

    #[test]
    fn shift_drag_with_rectangle_tool_draws_a_square() {
        let (mut app, ctx) = drawing_app(Tool::Rectangle);
        drag(
            &mut app,
            &ctx,
            pos2(20.0, 10.0),
            pos2(80.0, 40.0),
            Modifiers::SHIFT,
        );

        apply_floating_shape(&mut app);
        assert_eq!(app.documents[0].undo_label(), Some("Rectangle"));
        // Shift turned the 60×30 drag into a 60×60 square, down to y = 70.
        assert!(is_red(&app, 80, 60), "right edge continues below the drag");
        assert!(is_red(&app, 50, 70), "bottom edge");
        assert!(!is_red(&app, 50, 40), "not at the unconstrained bottom");
    }

    #[test]
    fn plain_drag_with_ellipse_tool_draws_an_oval() {
        let (mut app, ctx) = drawing_app(Tool::Ellipse);
        drag(
            &mut app,
            &ctx,
            pos2(20.0, 10.0),
            pos2(220.0, 90.0),
            Modifiers::NONE,
        );
        apply_floating_shape(&mut app);
        assert_eq!(app.documents[0].undo_label(), Some("Ellipse"));
        assert!(is_red(&app, 20, 50), "left end");
        assert!(is_red(&app, 220, 50), "right end, 200 px wide");
        assert!(is_red(&app, 120, 10), "top, only 80 px tall");
        assert!(!is_red(&app, 120, 50), "not filled");
    }

    #[test]
    fn dragging_with_arrow_tool_draws_an_arrow() {
        let (mut app, ctx) = drawing_app(Tool::Arrow);
        drag(
            &mut app,
            &ctx,
            pos2(20.0, 50.0),
            pos2(200.0, 50.0),
            Modifiers::NONE,
        );
        apply_floating_shape(&mut app);
        assert_eq!(app.documents[0].undo_label(), Some("Arrow"));
        assert!(is_red(&app, 30, 50), "shaft starts where the drag started");
        assert!(is_red(&app, 185, 55), "head near the end");
        assert!(!is_red(&app, 205, 50), "nothing past the tip");
    }

    #[test]
    fn brush_paints_dots_and_strokes() {
        let (mut app, ctx) = drawing_app(Tool::Brush);
        let at = screen(40.0, 40.0);
        canvas_frame(&mut app, &ctx, vec![Event::PointerMoved(at)]);
        canvas_frame(&mut app, &ctx, vec![button(at, true, Modifiers::NONE)]);
        canvas_frame(&mut app, &ctx, vec![button(at, false, Modifiers::NONE)]);
        assert!(app.pending_shape.is_some(), "a click makes a floating dot");
        assert!(!is_red(&app, 40, 40));

        drag(
            &mut app,
            &ctx,
            pos2(100.0, 20.0),
            pos2(250.0, 80.0),
            Modifiers::NONE,
        );
        assert!(
            is_red(&app, 40, 40),
            "starting the next stroke applied the dot"
        );
        apply_floating_shape(&mut app);
        assert!(is_red(&app, 175, 50), "the stroke runs along the drag");
        app.undo();
        assert!(!is_red(&app, 175, 50));
        assert!(is_red(&app, 40, 40), "undo removes one stroke at a time");
    }

    #[test]
    fn escape_cancels_a_shape_being_dragged() {
        let (mut app, ctx) = drawing_app(Tool::Rectangle);
        let (from, to) = (screen(20.0, 20.0), screen(80.0, 80.0));
        canvas_frame(&mut app, &ctx, vec![Event::PointerMoved(from)]);
        canvas_frame(&mut app, &ctx, vec![button(from, true, Modifiers::NONE)]);
        canvas_frame(&mut app, &ctx, vec![Event::PointerMoved(to)]);
        assert!(app.drawing.is_some());
        canvas_frame(&mut app, &ctx, vec![key(Key::Escape, true)]);
        canvas_frame(&mut app, &ctx, vec![button(to, false, Modifiers::NONE)]);
        assert!(app.drawing.is_none());
        assert_eq!(app.documents[0].undo_label(), None);
    }

    #[test]
    fn tool_shortcuts() {
        let ctx = egui::Context::default();
        let mut app = XPaintApp::with_context(&ctx, Vec::new());
        for tool in Tool::ALL {
            frame(&mut app, &ctx, vec![key(tool.key(), true)]);
            assert_eq!(app.tool, tool);
        }
    }

    /// Frame time while painting a long brush stroke over a full HD image.
    /// Run with `cargo test --release -- --ignored --nocapture brush_frame_time`.
    #[test]
    #[ignore]
    fn brush_frame_time() {
        let ctx = egui::Context::default();
        let mut app = XPaintApp::with_context(&ctx, Vec::new());
        app.add_document(Document::new("hd", RgbaImage::new(1920, 1080)));
        app.tool = Tool::Brush;
        let size = vec2(1920.0, 1080.0);
        let run = |app: &mut XPaintApp, events: Vec<Event>| {
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
                events,
                ..Default::default()
            };
            let start = std::time::Instant::now();
            let mut output = ctx.run_ui(input, |ui| {
                app.handle_shortcuts(ui.ctx());
                app.canvas(ui);
            });
            let _ = ctx.tessellate(std::mem::take(&mut output.shapes), 1.0);
            output.textures_delta.clear();
            start.elapsed()
        };
        let at = |i: usize| {
            pos2(
                100.0 + (i % 1700) as f32,
                200.0 + (i as f32 / 40.0).sin() * 150.0,
            )
        };
        run(&mut app, vec![Event::PointerMoved(at(0))]);
        run(&mut app, vec![button(at(0), true, Modifiers::NONE)]);
        let mut times = Vec::new();
        for i in 1..=2000 {
            times.push(run(&mut app, vec![Event::PointerMoved(at(i))]));
        }
        let idle = run(&mut app, vec![]);
        let avg =
            |t: &[std::time::Duration]| t.iter().sum::<std::time::Duration>() / t.len() as u32;
        println!(
            "points: {}  first 100 frames: {:?}  last 100 frames: {:?}  idle frame: {idle:?}",
            app.drawing.as_ref().map_or(0, |d| d.points.len()),
            avg(&times[..100]),
            avg(&times[times.len() - 100..]),
        );
    }

    #[test]
    fn floating_shape_can_be_dragged_restyled_and_nudged() {
        let (mut app, ctx) = drawing_app(Tool::Rectangle);
        drag(
            &mut app,
            &ctx,
            pos2(20.0, 10.0),
            pos2(80.0, 40.0),
            Modifiers::NONE,
        );
        // Grab it inside and move it by (100, 20).
        drag(
            &mut app,
            &ctx,
            pos2(50.0, 25.0),
            pos2(150.0, 45.0),
            Modifiers::NONE,
        );
        let pending = app
            .pending_shape
            .as_ref()
            .expect("still floating after the move");
        assert_eq!(pending.offset, vec2(100.0, 20.0));
        assert_eq!(app.documents[0].undo_label(), None, "moving draws nothing");

        frame(&mut app, &ctx, vec![key(Key::ArrowRight, true)]);
        assert_eq!(
            app.pending_shape.as_ref().unwrap().offset,
            vec2(101.0, 20.0)
        );

        // Color and width changes apply to the floating shape.
        app.shape_style = ShapeStyle {
            color: [0, 0, 255, 255],
            width: 2.0,
        };
        frame(&mut app, &ctx, vec![key(Key::Escape, true)]);
        assert!(app.pending_shape.is_none());
        let image = &app.documents[0].image;
        let blue = |x, y| image.get_pixel(x, y).0 == [0, 0, 255, 255];
        assert!(blue(181, 45), "right edge, moved from x = 80 to 181");
        assert!(blue(150, 60), "bottom edge, moved from y = 40 to 60");
        assert!(
            !blue(80, 25) && !is_red(&app, 80, 25),
            "nothing left at the old place"
        );
    }

    #[test]
    fn delete_removes_a_floating_shape() {
        let (mut app, ctx) = drawing_app(Tool::Ellipse);
        drag(
            &mut app,
            &ctx,
            pos2(20.0, 10.0),
            pos2(80.0, 40.0),
            Modifiers::NONE,
        );
        frame(&mut app, &ctx, vec![key(Key::Delete, true)]);
        assert!(app.pending_shape.is_none());
        app.commit_floating();
        assert_eq!(app.documents[0].undo_label(), None);
    }

    #[test]
    fn floating_brush_stroke_moves_like_the_other_shapes() {
        let (mut app, ctx) = drawing_app(Tool::Brush);
        drag(
            &mut app,
            &ctx,
            pos2(20.0, 50.0),
            pos2(120.0, 50.0),
            Modifiers::NONE,
        );

        // Pressing on the stroke moves it.
        drag(
            &mut app,
            &ctx,
            pos2(70.0, 51.0),
            pos2(100.0, 61.0),
            Modifiers::NONE,
        );
        assert_eq!(app.pending_shape.as_ref().unwrap().offset, vec2(30.0, 10.0));
        assert_eq!(app.documents[0].undo_label(), None);

        // Pressing next to it paints a new stroke and applies the old one.
        drag(
            &mut app,
            &ctx,
            pos2(200.0, 10.0),
            pos2(200.0, 40.0),
            Modifiers::NONE,
        );
        assert_eq!(app.documents[0].undo_label(), Some("Brush"));
        assert!(
            is_red(&app, 100, 60),
            "the first stroke was applied where it was moved to"
        );
        assert!(!is_red(&app, 30, 50), "and not where it was drawn");
        assert_eq!(app.pending_shape.as_ref().unwrap().offset, Vec2::ZERO);
    }

    #[test]
    fn brush_strokes_start_at_once_and_use_every_mouse_position() {
        let (mut app, ctx) = drawing_app(Tool::Brush);
        let start = screen(10.0, 10.0);
        canvas_frame(&mut app, &ctx, vec![Event::PointerMoved(start)]);
        canvas_frame(&mut app, &ctx, vec![button(start, true, Modifiers::NONE)]);
        assert!(
            app.drawing.is_some(),
            "painting starts on press, without a drag threshold"
        );

        // Three small moves in one frame, less than egui's drag threshold together.
        let moves = [(11.0, 10.0), (12.0, 10.0), (13.0, 11.0)]
            .map(|(x, y)| Event::PointerMoved(screen(x, y)))
            .to_vec();
        canvas_frame(&mut app, &ctx, moves);
        // 1 px moves are closer than the point spacing, so the second point followed
        // the mouse until it was 2 px from the first and was then kept.
        let points = &app.drawing.as_ref().unwrap().points;
        assert_eq!(points, &[(10.0, 10.0), (12.0, 10.0), (13.0, 11.0)]);
    }

    #[test]
    fn new_images_shrink_to_fit_but_never_enlarge() {
        let (mut app, ctx) = drawing_app(Tool::Brush);
        assert_eq!(app.documents[0].zoom, 1.0, "300×100 fits as is");
        app.add_document(Document::new("big", RgbaImage::new(2000, 1500)));
        canvas_frame(&mut app, &ctx, vec![]);
        let zoom = app.documents[1].zoom;
        assert!(zoom < 1.0 && 2000.0 * zoom < 800.0, "zoom {zoom}");
        // Zooming afterwards is not undone by the next frame.
        app.documents[1].zoom = 1.0;
        canvas_frame(&mut app, &ctx, vec![]);
        assert_eq!(app.documents[1].zoom, 1.0);
    }

    #[test]
    fn brush_keeps_points_a_few_screen_pixels_apart_at_any_zoom() {
        let (mut app, ctx) = drawing_app(Tool::Brush);
        app.documents[0].zoom = 5.0;
        app.documents[0].fit_when_shown = false;
        canvas_frame(&mut app, &ctx, vec![]); // lay out at the new zoom
        // At 500 % the 300×100 image is 1500×500 screen px, scrolled to its top left.
        let start = pos2(10.0, 300.0);
        canvas_frame(&mut app, &ctx, vec![Event::PointerMoved(start)]);
        canvas_frame(&mut app, &ctx, vec![button(start, true, Modifiers::NONE)]);
        // A slightly shaky, 200 screen px long move, in 1 px steps.
        let moves = (1..=200)
            .map(|i| Event::PointerMoved(start + vec2(i as f32, (i % 3) as f32 - 1.0)))
            .collect();
        canvas_frame(&mut app, &ctx, moves);
        let points = &app.drawing.as_ref().unwrap().points;
        let spacing = points
            .windows(2)
            .take(points.len() - 2) // the last point is still following the mouse
            .map(|w| (w[1].0 - w[0].0).hypot(w[1].1 - w[0].1))
            .fold(f32::MAX, f32::min);
        assert!(
            spacing >= 2.0 / 5.0 - 1e-4,
            "points {spacing} image px apart"
        );
        // The zigzag makes the path about 1.6× longer than 200 px; without the spacing
        // every one of the 200 moves would add a point.
        assert!(points.len() < 170, "{} points for 200 moves", points.len());
        let tip = *points.last().unwrap();
        assert!(
            (tip.0 - points[0].0 - 40.0).abs() < 0.01,
            "the tip is at the mouse"
        );
    }

    #[test]
    fn save_overwrites_the_opened_file() {
        let dir = std::env::temp_dir().join(format!("xpaint-save-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("picture.png");
        RgbaImage::from_pixel(60, 40, image::Rgba([255, 255, 255, 255]))
            .save(&path)
            .unwrap();

        let ctx = egui::Context::default();
        let mut app = XPaintApp::with_context(&ctx, vec![path.clone()]);
        app.tool = Tool::Rectangle;
        app.shape_style.color = [255, 0, 0, 255];
        // A floating rectangle; Ctrl+S applies it before saving.
        app.pending_shape = Some(PendingShape {
            tool: Tool::Rectangle,
            drawing: Drawing {
                points: vec![(10.0, 10.0), (50.0, 30.0)],
                constrain: false,
            },
            offset: Vec2::ZERO,
            grab: None,
        });
        let ctrl_s = Event::Key {
            key: Key::S,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::COMMAND,
        };
        frame(
            &mut app,
            &ctx,
            vec![Event::ModifiersChanged(Modifiers::COMMAND), ctrl_s],
        );

        assert!(app.pending_shape.is_none());
        let saved = image::open(&path).unwrap().into_rgba8();
        assert_eq!(
            saved, app.documents[0].image,
            "the file holds the edited image"
        );
        assert_eq!(saved.get_pixel(10, 20).0, [255, 0, 0, 255]);
        assert_eq!(
            app.tool,
            Tool::Rectangle,
            "Ctrl+S is not the S tool shortcut"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn empty_state_buttons_are_centered() {
        let ctx = egui::Context::default();
        let mut app = XPaintApp::with_context(&ctx, Vec::new());
        let screen = Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 600.0));
        let mut buttons = Rect::NOTHING;
        // The first frame measures the row, the second lays it out centered.
        for _ in 0..3 {
            let input = egui::RawInput {
                screen_rect: Some(screen),
                ..Default::default()
            };
            let mut output = ctx.run_ui(input, |ui| app.empty_state(ui));
            output.textures_delta.clear();
            buttons = ctx.viewport(|v| {
                v.this_pass
                    .widgets
                    .layers()
                    .flat_map(|(_, widgets)| widgets.iter())
                    .filter(|w| w.sense.senses_click())
                    .fold(Rect::NOTHING, |r, w| r.union(w.rect))
            });
        }
        assert!(buttons.width() > 150.0, "both buttons found: {buttons:?}");
        let off_center = buttons.center().x - screen.center().x;
        assert!(off_center.abs() < 1.0, "buttons {off_center} px off center");
    }

    #[test]
    fn icon_size_scales_buttons_and_is_stored_by_name() {
        for size in IconSize::ALL {
            assert_eq!(IconSize::from_name(size.name()), Some(size));
        }
        assert_eq!(IconSize::from_name("Huge"), None);

        let ctx = egui::Context::default();
        let width_at = |size: IconSize| {
            set_icon_scale(&ctx, size.scale());
            let mut width = 0.0;
            let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                width = icon_button(ui, OPEN_ICON, true, "Open").rect.width();
            });
            output.textures_delta.clear();
            width
        };
        assert_eq!(width_at(IconSize::Normal), ICON_BUTTON.x);
        assert_eq!(width_at(IconSize::Larger), ICON_BUTTON.x * 1.5);
    }
}
