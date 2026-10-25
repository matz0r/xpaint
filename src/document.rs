use std::path::{Path, PathBuf};

use egui::{ColorImage, TextureHandle, TextureOptions};
use image::codecs::jpeg::JpegEncoder;
use image::{ImageFormat, RgbImage, RgbaImage};

const JPEG_QUALITY: u8 = 92;

/// A rectangle in image pixel coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PixelRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// Undo snapshots are dropped, oldest first, once together they use more memory than this.
/// The most recent snapshot is always kept, however large.
const HISTORY_BYTES: usize = 1 << 30;

/// The state of a document before an edit, so the edit can be undone (or redone).
struct Snapshot {
    /// Name of the edit, e.g. "Crop", for "Undo Crop".
    label: &'static str,
    image: RgbaImage,
    selection: Option<PixelRect>,
}

/// A single open image, shown as one tab in the editor.
pub struct Document {
    pub name: String,
    pub path: Option<PathBuf>,
    pub image: RgbaImage,
    pub zoom: f32,
    pub selection: Option<PixelRect>,
    undo_stack: Vec<Snapshot>,
    redo_stack: Vec<Snapshot>,
    texture: Option<TextureHandle>,
}

impl Document {
    pub fn new(name: impl Into<String>, image: RgbaImage) -> Self {
        Self {
            name: name.into(),
            path: None,
            image,
            zoom: 1.0,
            selection: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            texture: None,
        }
    }

    pub fn open(path: &Path) -> Result<Self, String> {
        let image = image::open(path)
            .map_err(|e| format!("Could not open {}: {e}", path.display()))?
            .into_rgba8();
        let mut doc = Self::new(file_name(path), image);
        doc.path = Some(path.to_owned());
        Ok(doc)
    }

    /// Writes the image to `path` in the format its extension names, adding `.png` if it
    /// has none, and from then on shows the document under the new name. Returns the
    /// path actually written.
    pub fn save_as(&mut self, path: &Path) -> Result<PathBuf, String> {
        let path = if path.extension().is_some() {
            path.to_owned()
        } else {
            path.with_extension("png")
        };
        let error = |e: &dyn std::fmt::Display| format!("Could not save {}: {e}", path.display());

        let format = ImageFormat::from_path(&path).map_err(|e| error(&e))?;
        if format == ImageFormat::Jpeg {
            // JPEG has no alpha channel, so flatten transparent areas onto white.
            let file = std::fs::File::create(&path).map_err(|e| error(&e))?;
            let encoder =
                JpegEncoder::new_with_quality(std::io::BufWriter::new(file), JPEG_QUALITY);
            flatten_on_white(&self.image)
                .write_with_encoder(encoder)
                .map_err(|e| error(&e))?;
        } else {
            self.image
                .save_with_format(&path, format)
                .map_err(|e| error(&e))?;
        }

        self.name = file_name(&path);
        self.path = Some(path.clone());
        Ok(path)
    }

    /// A file name to suggest when saving: the current one, or the tab name as PNG.
    pub fn suggested_file_name(&self) -> String {
        match &self.path {
            Some(path) => file_name(path),
            None => format!("{}.png", self.name),
        }
    }

    pub fn width(&self) -> u32 {
        self.image.width()
    }

    pub fn height(&self) -> u32 {
        self.image.height()
    }

    /// Selects the rectangle spanned by two corners given in (fractional) image
    /// coordinates, clamped to the image. An empty rectangle clears the selection.
    pub fn select_between(&mut self, a: (f32, f32), b: (f32, f32)) {
        let clamp_x = |v: f32| v.round().clamp(0.0, self.width() as f32) as u32;
        let clamp_y = |v: f32| v.round().clamp(0.0, self.height() as f32) as u32;
        let (x0, x1) = (clamp_x(a.0.min(b.0)), clamp_x(a.0.max(b.0)));
        let (y0, y1) = (clamp_y(a.1.min(b.1)), clamp_y(a.1.max(b.1)));

        self.selection = (x1 > x0 && y1 > y0).then_some(PixelRect {
            x: x0,
            y: y0,
            width: x1 - x0,
            height: y1 - y0,
        });
    }

    /// The pixels inside the selection, or the whole image when nothing is selected.
    pub fn selected_image(&self) -> RgbaImage {
        match self.selection {
            Some(sel) => {
                image::imageops::crop_imm(&self.image, sel.x, sel.y, sel.width, sel.height)
                    .to_image()
            }
            None => self.image.clone(),
        }
    }

    /// Crops the image to the current selection and clears it.
    pub fn crop_to_selection(&mut self) {
        let Some(sel) = self.selection else {
            return;
        };
        self.record("Crop");
        self.selection = None;
        self.image =
            image::imageops::crop_imm(&self.image, sel.x, sel.y, sel.width, sel.height).to_image();
        self.texture = None;
    }

    /// Alpha-blends `overlay` onto the image with its top left corner at (`x`, `y`),
    /// as one undoable edit named `label`. Parts outside the image are clipped.
    pub fn draw_overlay(&mut self, label: &'static str, overlay: &RgbaImage, x: i64, y: i64) {
        self.record(label);
        image::imageops::overlay(&mut self.image, overlay, x, y);
        self.texture = None;
    }

    /// Name of the edit that [`Self::undo`] would revert.
    pub fn undo_label(&self) -> Option<&'static str> {
        self.undo_stack.last().map(|s| s.label)
    }

    /// Name of the edit that [`Self::redo`] would reapply.
    pub fn redo_label(&self) -> Option<&'static str> {
        self.redo_stack.last().map(|s| s.label)
    }

    /// Reverts the last edit and returns its name.
    pub fn undo(&mut self) -> Option<&'static str> {
        let snapshot = self.undo_stack.pop()?;
        let current = self.restore(snapshot);
        let label = current.label;
        self.redo_stack.push(current);
        Some(label)
    }

    /// Reapplies the last undone edit and returns its name.
    pub fn redo(&mut self) -> Option<&'static str> {
        let snapshot = self.redo_stack.pop()?;
        let current = self.restore(snapshot);
        let label = current.label;
        self.undo_stack.push(current);
        Some(label)
    }

    /// Saves the current state before an edit named `label`. A new edit makes the
    /// undone ones unreachable, so it also clears the redo history.
    fn record(&mut self, label: &'static str) {
        self.redo_stack.clear();
        self.undo_stack.push(Snapshot {
            label,
            image: self.image.clone(),
            selection: self.selection,
        });
        trim_history(&mut self.undo_stack, HISTORY_BYTES);
    }

    /// Replaces the current state with `snapshot` and returns the replaced state,
    /// labeled like `snapshot` since it is the other side of the same edit.
    fn restore(&mut self, snapshot: Snapshot) -> Snapshot {
        self.texture = None;
        Snapshot {
            label: snapshot.label,
            image: std::mem::replace(&mut self.image, snapshot.image),
            selection: std::mem::replace(&mut self.selection, snapshot.selection),
        }
    }

    /// Returns the GPU texture for this image, uploading it on first use.
    pub fn texture(&mut self, ctx: &egui::Context) -> &TextureHandle {
        self.texture.get_or_insert_with(|| {
            let size = [self.image.width() as usize, self.image.height() as usize];
            let color_image = ColorImage::from_rgba_unmultiplied(size, self.image.as_raw());
            ctx.load_texture(&self.name, color_image, TextureOptions::NEAREST)
        })
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Untitled".to_owned())
}

/// Composites `image` over a white background, dropping its alpha channel.
fn flatten_on_white(image: &RgbaImage) -> RgbImage {
    RgbImage::from_fn(image.width(), image.height(), |x, y| {
        let [r, g, b, a] = image.get_pixel(x, y).0;
        let over_white = |c: u8| ((c as u32 * a as u32 + 255 * (255 - a as u32) + 127) / 255) as u8;
        image::Rgb([over_white(r), over_white(g), over_white(b)])
    })
}

/// Drops the oldest snapshots until the rest fit in `budget` bytes, keeping at least one.
fn trim_history(stack: &mut Vec<Snapshot>, budget: usize) {
    let mut total: usize = stack.iter().map(|s| s.image.as_raw().len()).sum();
    while total > budget && stack.len() > 1 {
        total -= stack.remove(0).image.as_raw().len();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(width: u32, height: u32) -> Document {
        let image = RgbaImage::from_fn(width, height, |x, y| {
            image::Rgba([x as u8, y as u8, 0, 255])
        });
        Document::new("test", image)
    }

    #[test]
    fn select_between_normalizes_and_clamps() {
        let mut d = doc(100, 50);
        d.select_between((80.4, 40.0), (-10.0, 10.6));
        assert_eq!(
            d.selection,
            Some(PixelRect {
                x: 0,
                y: 11,
                width: 80,
                height: 29
            })
        );

        d.select_between((90.0, 20.0), (500.0, 500.0));
        assert_eq!(
            d.selection,
            Some(PixelRect {
                x: 90,
                y: 20,
                width: 10,
                height: 30
            })
        );
    }

    #[test]
    fn empty_selection_clears() {
        let mut d = doc(100, 50);
        d.select_between((10.0, 10.0), (20.0, 20.0));
        d.select_between((10.0, 10.0), (10.2, 30.0));
        assert_eq!(d.selection, None);
        // Entirely outside the image.
        d.select_between((120.0, 10.0), (150.0, 30.0));
        assert_eq!(d.selection, None);
    }

    #[test]
    fn selected_image_is_selection_or_whole_image() {
        let mut d = doc(100, 50);
        assert_eq!(d.selected_image(), d.image);

        d.select_between((10.0, 5.0), (30.0, 25.0));
        let copied = d.selected_image();
        assert_eq!(copied.dimensions(), (20, 20));
        assert_eq!(copied.get_pixel(0, 0).0, [10, 5, 0, 255]);
        assert_eq!(copied.get_pixel(19, 19).0, [29, 24, 0, 255]);
        assert_eq!(d.width(), 100, "copying leaves the image untouched");
        assert!(d.undo_label().is_none(), "copying is not an edit");
    }

    #[test]
    fn crop_keeps_selected_pixels() {
        let mut d = doc(100, 50);
        d.select_between((10.0, 5.0), (30.0, 25.0));
        d.crop_to_selection();
        assert_eq!((d.width(), d.height()), (20, 20));
        assert_eq!(d.image.get_pixel(0, 0).0, [10, 5, 0, 255]);
        assert_eq!(d.image.get_pixel(19, 19).0, [29, 24, 0, 255]);
        assert_eq!(d.selection, None);
    }

    #[test]
    fn undo_and_redo_crop() {
        let mut d = doc(100, 50);
        assert_eq!(d.undo(), None);

        d.select_between((10.0, 5.0), (30.0, 25.0));
        let selection = d.selection;
        d.crop_to_selection();
        assert_eq!(d.undo_label(), Some("Crop"));

        assert_eq!(d.undo(), Some("Crop"));
        assert_eq!((d.width(), d.height()), (100, 50));
        assert_eq!(d.selection, selection, "undo restores the selection");
        assert_eq!(d.undo_label(), None);
        assert_eq!(d.redo_label(), Some("Crop"));

        assert_eq!(d.redo(), Some("Crop"));
        assert_eq!((d.width(), d.height()), (20, 20));
        assert_eq!(d.selection, None);
        assert_eq!(d.redo(), None);
    }

    #[test]
    fn new_edit_clears_redo() {
        let mut d = doc(100, 50);
        d.select_between((0.0, 0.0), (50.0, 50.0));
        d.crop_to_selection();
        d.undo();
        d.select_between((0.0, 0.0), (10.0, 10.0));
        d.crop_to_selection();
        assert_eq!(d.redo_label(), None);
        assert_eq!(d.undo(), Some("Crop"));
        assert_eq!((d.width(), d.height()), (100, 50));
    }

    #[test]
    fn draw_overlay_blends_clips_and_undoes() {
        let mut d = doc(10, 10);
        let before = d.image.clone();
        let overlay = RgbaImage::from_pixel(4, 4, image::Rgba([255, 255, 255, 255]));
        d.draw_overlay("Text", &overlay, 8, -2); // hangs off the top right corner

        assert_eq!(d.image.get_pixel(9, 0).0, [255, 255, 255, 255]);
        assert_eq!(d.image.get_pixel(8, 1).0, [255, 255, 255, 255]);
        assert_eq!(d.image.get_pixel(7, 0), before.get_pixel(7, 0));
        assert_eq!(d.image.get_pixel(9, 2), before.get_pixel(9, 2));

        assert_eq!(d.undo(), Some("Text"));
        assert_eq!(d.image, before);
    }

    #[test]
    fn save_as_round_trips_and_renames() {
        let dir = std::env::temp_dir().join(format!("xpaint-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut d = doc(40, 30);

        for ext in ["png", "bmp", "tga", "tiff", "webp", "qoi"] {
            let written = d.save_as(&dir.join(format!("out.{ext}"))).unwrap();
            let back = image::open(&written).unwrap().into_rgba8();
            assert_eq!(back, d.image, "{ext} should be lossless");
            assert_eq!(d.name, format!("out.{ext}"));
        }

        // No extension: saved as PNG.
        let written = d.save_as(&dir.join("noext")).unwrap();
        assert_eq!(written, dir.join("noext.png"));
        assert_eq!(image::open(&written).unwrap().into_rgba8(), d.image);

        // JPEG is lossy, so only check that it decodes at the right size.
        let written = d.save_as(&dir.join("out.jpg")).unwrap();
        let back = image::open(&written).unwrap();
        assert_eq!((back.width(), back.height()), (40, 30));

        assert!(d.save_as(&dir.join("out.unknown")).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn flatten_on_white_blends_alpha() {
        let image =
            RgbaImage::from_raw(3, 1, vec![0, 0, 0, 0, 0, 0, 0, 255, 0, 0, 0, 128]).unwrap();
        let flat = flatten_on_white(&image);
        assert_eq!(flat.get_pixel(0, 0).0, [255, 255, 255]);
        assert_eq!(flat.get_pixel(1, 0).0, [0, 0, 0]);
        assert_eq!(flat.get_pixel(2, 0).0, [127, 127, 127]);
    }

    #[test]
    fn history_drops_oldest_but_keeps_last_step() {
        let snapshot = |width| Snapshot {
            label: "Crop",
            image: RgbaImage::new(width, 1), // width × 4 bytes
            selection: None,
        };
        let mut stack = vec![snapshot(10), snapshot(20), snapshot(30)];
        trim_history(&mut stack, 200);
        let widths: Vec<u32> = stack.iter().map(|s| s.image.width()).collect();
        assert_eq!(widths, [20, 30]);

        trim_history(&mut stack, 1);
        assert_eq!(stack.len(), 1);
        assert_eq!(stack[0].image.width(), 30);
    }
}
