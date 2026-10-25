use std::sync::LazyLock;

use ab_glyph::{Font, FontRef, PxScale, ScaleFont, point};
use image::{Rgba, RgbaImage};

static REGULAR: LazyLock<FontRef<'static>> = LazyLock::new(|| {
    FontRef::try_from_slice(include_bytes!("../assets/fonts/DejaVuSans.ttf"))
        .expect("embedded font is valid")
});
static BOLD: LazyLock<FontRef<'static>> = LazyLock::new(|| {
    FontRef::try_from_slice(include_bytes!("../assets/fonts/DejaVuSans-Bold.ttf"))
        .expect("embedded font is valid")
});

/// How text is drawn into the image.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextStyle {
    /// Font size in image pixels.
    pub size: f32,
    pub color: [u8; 4],
    pub bold: bool,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            size: 32.0,
            color: [0, 0, 0, 255],
            bold: false,
        }
    }
}

/// Text rasterized into its own image, ready to be composited onto a document.
/// All positions are relative to the text's anchor, the top left of its first line.
pub struct RenderedText {
    /// The glyphs' pixels; may extend left of or above the anchor (e.g. for "j").
    pub image: RgbaImage,
    /// Where `image`'s top left corner goes, relative to the anchor.
    pub origin: (i32, i32),
    /// Size of the layout box: widest line by total line height.
    pub size: (f32, f32),
    /// Top of the caret after the last character, and the caret height.
    pub caret: (f32, f32),
    pub caret_height: f32,
}

/// Lays out `text` (lines separated by `\n`) and rasterizes it.
pub fn render(text: &str, style: &TextStyle) -> RenderedText {
    let font = if style.bold { &*BOLD } else { &*REGULAR };
    let font = font.as_scaled(PxScale::from(style.size.max(1.0)));
    let line_height = font.ascent() - font.descent() + font.line_gap();

    // Position every glyph and track the union of their pixel bounds.
    let mut outlines = Vec::new();
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
    let mut width: f32 = 0.0;
    let mut caret = (0.0, 0.0);
    for (row, line) in text.split('\n').enumerate() {
        let top = row as f32 * line_height;
        let mut x = 0.0;
        let mut previous = None;
        for c in line.chars() {
            let id = font.glyph_id(c);
            if let Some(previous) = previous {
                x += font.kern(previous, id);
            }
            let glyph = id.with_scale_and_position(font.scale(), point(x, top + font.ascent()));
            x += font.h_advance(id);
            previous = Some(id);

            if let Some(outline) = font.outline_glyph(glyph) {
                let b = outline.px_bounds();
                min_x = min_x.min(b.min.x.floor() as i32);
                min_y = min_y.min(b.min.y.floor() as i32);
                max_x = max_x.max(b.max.x.ceil() as i32);
                max_y = max_y.max(b.max.y.ceil() as i32);
                outlines.push(outline);
            }
        }
        width = width.max(x);
        caret = (x, top);
    }

    if outlines.is_empty() {
        (min_x, min_y, max_x, max_y) = (0, 0, 0, 0);
    }
    let [r, g, b, a] = style.color;
    let mut image = RgbaImage::new((max_x - min_x) as u32, (max_y - min_y) as u32);
    for outline in &outlines {
        let bounds = outline.px_bounds();
        let (left, top) = (bounds.min.x.floor() as i32, bounds.min.y.floor() as i32);
        outline.draw(|gx, gy, coverage| {
            let x = (left + gx as i32 - min_x) as u32;
            let y = (top + gy as i32 - min_y) as u32;
            if x >= image.width() || y >= image.height() {
                return;
            }
            // Overlapping glyphs keep the stronger coverage instead of adding up.
            let alpha = (coverage.clamp(0.0, 1.0) * a as f32).round() as u8;
            let pixel = image.get_pixel_mut(x, y);
            if alpha > pixel.0[3] {
                *pixel = Rgba([r, g, b, alpha]);
            }
        });
    }

    let lines = text.split('\n').count();
    RenderedText {
        image,
        origin: (min_x, min_y),
        size: (width, lines as f32 * line_height),
        caret,
        caret_height: line_height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_text_has_no_pixels_but_a_caret() {
        let r = render("", &TextStyle::default());
        assert_eq!(r.image.dimensions(), (0, 0));
        assert_eq!(r.caret, (0.0, 0.0));
        assert!(r.caret_height > 30.0);
    }

    #[test]
    fn text_is_drawn_in_the_style_color() {
        let style = TextStyle {
            color: [200, 10, 20, 255],
            ..TextStyle::default()
        };
        let r = render("Hello", &style);
        let (w, h) = r.image.dimensions();
        assert!(w > 60 && h > 15, "unexpected size {w}×{h}");
        let solid = r
            .image
            .pixels()
            .filter(|p| p.0 == [200, 10, 20, 255])
            .count();
        assert!(solid > 50, "expected fully covered pixels, got {solid}");
        assert!(
            r.image
                .pixels()
                .all(|p| p.0[3] == 0 || p.0[..3] == [200, 10, 20])
        );
    }

    #[test]
    fn lines_stack_and_caret_follows_last_line() {
        let style = TextStyle::default();
        let one = render("ab", &style);
        let two = render("ab\ncd", &style);
        assert_eq!(two.size.1, 2.0 * one.size.1);
        assert_eq!(two.caret.1, one.caret_height);
        assert!(two.image.height() > one.image.height() + 20);
    }

    #[test]
    fn bold_is_wider() {
        let regular = render("Bold", &TextStyle::default());
        let bold = render(
            "Bold",
            &TextStyle {
                bold: true,
                ..TextStyle::default()
            },
        );
        assert!(bold.size.0 > regular.size.0);
    }
}
