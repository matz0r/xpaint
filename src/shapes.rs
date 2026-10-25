//! Geometry and rasterization for the drawing tools: brush strokes, arrows,
//! rectangles and ellipses.
//!
//! A [`Shape`] is first broken down into [`Primitive`]s. The canvas draws those with
//! egui as a live preview, and [`rasterize`] draws the same ones into pixels when the
//! shape is committed, so the preview matches the result.

use image::{Rgba, RgbaImage};
use tiny_skia::{FillRule, LineCap, LineJoin, Paint, PathBuilder, Pixmap, Stroke, Transform};

/// A point in image pixel coordinates.
pub type Point = (f32, f32);

/// How shapes are drawn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShapeStyle {
    pub color: [u8; 4],
    /// Line width in image pixels.
    pub width: f32,
}

impl Default for ShapeStyle {
    fn default() -> Self {
        Self {
            color: [220, 40, 40, 255],
            width: 4.0,
        }
    }
}

pub enum Shape {
    /// A freehand line through the points the mouse moved over.
    Brush(Vec<Point>),
    /// A line from the first point with an arrowhead at the second.
    Arrow(Point, Point),
    /// A rectangle outline spanned by two corners.
    Rectangle(Point, Point),
    /// An ellipse outline inscribed in the rectangle spanned by two corners.
    Ellipse(Point, Point),
}

/// The basic pieces every shape is drawn from.
#[derive(Debug, PartialEq)]
pub enum Primitive {
    /// A line of the style's width with round ends; `closed` connects the last
    /// point back to the first, with sharp corners.
    Line {
        points: Vec<Point>,
        closed: bool,
    },
    Polygon(Vec<Point>),
    Dot {
        center: Point,
        radius: f32,
    },
}

/// Moves `end` so that the rectangle from `start` to it is a square, keeping the
/// direction of the drag. Used while Shift is held.
pub fn constrain_square(start: Point, end: Point) -> Point {
    let (dx, dy) = (end.0 - start.0, end.1 - start.1);
    let side = dx.abs().max(dy.abs());
    (start.0 + side.copysign(dx), start.1 + side.copysign(dy))
}

impl Shape {
    pub fn primitives(&self, width: f32) -> Vec<Primitive> {
        match self {
            Shape::Brush(points) => match points.as_slice() {
                [] => Vec::new(),
                [center] => vec![Primitive::Dot {
                    center: *center,
                    radius: width / 2.0,
                }],
                _ => vec![Primitive::Line {
                    points: smooth(points),
                    closed: false,
                }],
            },
            Shape::Arrow(start, tip) => arrow(*start, *tip, width),
            Shape::Rectangle(a, b) => {
                let (x0, y0, x1, y1) = (a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1));
                if x1 - x0 < 1.0 || y1 - y0 < 1.0 {
                    return Vec::new();
                }
                vec![Primitive::Line {
                    points: vec![(x0, y0), (x1, y0), (x1, y1), (x0, y1)],
                    closed: true,
                }]
            }
            Shape::Ellipse(a, b) => {
                let (rx, ry) = ((b.0 - a.0).abs() / 2.0, (b.1 - a.1).abs() / 2.0);
                if rx < 0.5 || ry < 0.5 {
                    return Vec::new();
                }
                let center = ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
                // Enough segments that the outline looks smooth at any size.
                let steps = ((rx.max(ry) * 2.0) as usize).clamp(32, 720);
                let points = (0..steps)
                    .map(|i| {
                        let t = std::f32::consts::TAU * i as f32 / steps as f32;
                        (center.0 + rx * t.cos(), center.1 + ry * t.sin())
                    })
                    .collect();
                vec![Primitive::Line {
                    points,
                    closed: true,
                }]
            }
        }
    }
}

/// Evens out the small wobble of a hand-drawn path with two rounds of Chaikin
/// corner cutting: every segment is replaced by points at 1/4 and 3/4 of it. The
/// result follows the path closely but no longer through every jittery point. The
/// first and last points stay where they are, so the stroke still starts and ends
/// exactly at the mouse.
pub fn smooth(points: &[Point]) -> Vec<Point> {
    let mut points = points.to_vec();
    for _ in 0..2 {
        if points.len() < 3 {
            break;
        }
        let mut cut = Vec::with_capacity(points.len() * 2);
        cut.push(points[0]);
        for w in points.windows(2) {
            let ((x0, y0), (x1, y1)) = (w[0], w[1]);
            cut.push((0.75 * x0 + 0.25 * x1, 0.75 * y0 + 0.25 * y1));
            cut.push((0.25 * x0 + 0.75 * x1, 0.25 * y0 + 0.75 * y1));
        }
        cut.push(points[points.len() - 1]);
        points = cut;
    }
    points
}

fn arrow(start: Point, tip: Point, width: f32) -> Vec<Primitive> {
    let (dx, dy) = (tip.0 - start.0, tip.1 - start.1);
    let length = dx.hypot(dy);
    if length < 1.0 {
        return Vec::new();
    }
    let (ux, uy) = (dx / length, dy / length);
    let head_length = (width * 4.0 + 6.0).min(length);
    let head_half_width = head_length * 0.55;
    let base = (tip.0 - ux * head_length, tip.1 - uy * head_length);
    let head = Primitive::Polygon(vec![
        tip,
        (base.0 - uy * head_half_width, base.1 + ux * head_half_width),
        (base.0 + uy * head_half_width, base.1 - ux * head_half_width),
    ]);

    // End the line inside the head, so its round end stays hidden under the head.
    let line_end = (
        tip.0 - ux * head_length * 0.8,
        tip.1 - uy * head_length * 0.8,
    );
    if length <= head_length {
        return vec![head];
    }
    vec![
        Primitive::Line {
            points: vec![start, line_end],
            closed: false,
        },
        head,
    ]
}

/// Moves every point of `primitives` by (`dx`, `dy`).
pub fn translate(primitives: &mut [Primitive], dx: f32, dy: f32) {
    let shift = |p: &mut Point| *p = (p.0 + dx, p.1 + dy);
    for primitive in primitives {
        match primitive {
            Primitive::Line { points, .. } | Primitive::Polygon(points) => {
                points.iter_mut().for_each(shift)
            }
            Primitive::Dot { center, .. } => shift(center),
        }
    }
}

/// The area `primitives` cover when drawn `width` wide, grown by `margin` on every
/// side, as (left, top, right, bottom). `None` if there is nothing to draw.
pub fn bounds(primitives: &[Primitive], width: f32, margin: f32) -> Option<(f32, f32, f32, f32)> {
    let line_margin = width / 2.0 + margin;
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    let mut extend = |(x, y): Point, r: f32| {
        x0 = x0.min(x - r);
        y0 = y0.min(y - r);
        x1 = x1.max(x + r);
        y1 = y1.max(y + r);
    };
    for primitive in primitives {
        match primitive {
            Primitive::Line { points, .. } => points.iter().for_each(|&p| extend(p, line_margin)),
            Primitive::Polygon(points) => points.iter().for_each(|&p| extend(p, margin)),
            Primitive::Dot { center, radius } => extend(*center, radius + margin),
        }
    }
    (x0 <= x1).then_some((x0, y0, x1, y1))
}

/// Whether `point` is on the shape drawn `width` wide, give or take `slack`. Closed
/// outlines (rectangles, ellipses) also count their inside, so they can be grabbed
/// anywhere.
pub fn hit_test(primitives: &[Primitive], point: Point, width: f32, slack: f32) -> bool {
    let near = width / 2.0 + slack;
    primitives.iter().any(|primitive| match primitive {
        Primitive::Line { points, closed } => {
            (*closed && contains(points, point))
                || segments(points, *closed).any(|(a, b)| distance_to_segment(point, a, b) <= near)
        }
        Primitive::Polygon(points) => {
            contains(points, point)
                || segments(points, true).any(|(a, b)| distance_to_segment(point, a, b) <= slack)
        }
        Primitive::Dot { center, radius } => {
            (point.0 - center.0).hypot(point.1 - center.1) <= radius + slack
        }
    })
}

fn segments(points: &[Point], closed: bool) -> impl Iterator<Item = (Point, Point)> + '_ {
    let closing = closed.then(|| (points[points.len() - 1], points[0]));
    points.windows(2).map(|w| (w[0], w[1])).chain(closing)
}

fn distance_to_segment(p: Point, a: Point, b: Point) -> f32 {
    let (abx, aby) = (b.0 - a.0, b.1 - a.1);
    let length_squared = abx * abx + aby * aby;
    let t = if length_squared == 0.0 {
        0.0
    } else {
        (((p.0 - a.0) * abx + (p.1 - a.1) * aby) / length_squared).clamp(0.0, 1.0)
    };
    (p.0 - (a.0 + t * abx)).hypot(p.1 - (a.1 + t * aby))
}

/// Even-odd point in polygon test.
fn contains(polygon: &[Point], p: Point) -> bool {
    let mut inside = false;
    for (a, b) in segments(polygon, true) {
        if (a.1 > p.1) != (b.1 > p.1) && p.0 < a.0 + (p.1 - a.1) / (b.1 - a.1) * (b.0 - a.0) {
            inside = !inside;
        }
    }
    inside
}

/// Draws `primitives` into an image covering their bounds, clipped to an image of
/// `canvas` size. Returns the pixels and their top left position, or `None` if
/// nothing would be visible.
pub fn rasterize(
    primitives: &[Primitive],
    style: &ShapeStyle,
    canvas: (u32, u32),
) -> Option<(RgbaImage, (i32, i32))> {
    let (x0, y0, x1, y1) = bounds(primitives, style.width, 2.0)?;
    let left = (x0.floor() as i32).max(0);
    let top = (y0.floor() as i32).max(0);
    let right = (x1.ceil() as i32).min(canvas.0 as i32);
    let bottom = (y1.ceil() as i32).min(canvas.1 as i32);
    if right <= left || bottom <= top {
        return None;
    }

    let mut pixmap = Pixmap::new((right - left) as u32, (bottom - top) as u32)?;
    let transform = Transform::from_translate(-left as f32, -top as f32);
    // Draw opaque and apply the color's alpha afterwards, so that overlapping parts
    // (an arrow's line under its head, a brush stroke crossing itself) do not get
    // darker than the rest.
    let mut paint = Paint::default();
    paint.set_color_rgba8(255, 255, 255, 255);
    paint.anti_alias = true;

    for primitive in primitives {
        match primitive {
            Primitive::Line { points, closed } => {
                let mut path = PathBuilder::new();
                path.move_to(points[0].0, points[0].1);
                for p in &points[1..] {
                    path.line_to(p.0, p.1);
                }
                if *closed {
                    path.close();
                }
                let Some(path) = path.finish() else { continue };
                let stroke = Stroke {
                    width: style.width,
                    line_cap: LineCap::Round,
                    line_join: if *closed {
                        LineJoin::Miter
                    } else {
                        LineJoin::Round
                    },
                    ..Stroke::default()
                };
                pixmap.stroke_path(&path, &paint, &stroke, transform, None);
            }
            Primitive::Polygon(points) => {
                let mut path = PathBuilder::new();
                path.move_to(points[0].0, points[0].1);
                for p in &points[1..] {
                    path.line_to(p.0, p.1);
                }
                path.close();
                if let Some(path) = path.finish() {
                    pixmap.fill_path(&path, &paint, FillRule::Winding, transform, None);
                }
            }
            Primitive::Dot { center, radius } => {
                if let Some(path) = PathBuilder::from_circle(center.0, center.1, radius.max(0.5)) {
                    pixmap.fill_path(&path, &paint, FillRule::Winding, transform, None);
                }
            }
        }
    }

    let [r, g, b, a] = style.color;
    let mut image = RgbaImage::new(pixmap.width(), pixmap.height());
    for (pixel, coverage) in image.pixels_mut().zip(pixmap.pixels()) {
        let alpha = (coverage.alpha() as u32 * a as u32 + 127) / 255;
        *pixel = Rgba([r, g, b, alpha as u8]);
    }
    Some((image, (left, top)))
}

#[cfg(test)]
mod tests {
    use super::*;

    const RED: ShapeStyle = ShapeStyle {
        color: [255, 0, 0, 255],
        width: 4.0,
    };

    /// Draws `shape` onto a transparent 100×100 canvas.
    fn draw(shape: Shape, style: &ShapeStyle) -> RgbaImage {
        let mut canvas = RgbaImage::new(100, 100);
        if let Some((image, (x, y))) = rasterize(&shape.primitives(style.width), style, (100, 100))
        {
            image::imageops::overlay(&mut canvas, &image, x as i64, y as i64);
        }
        canvas
    }

    fn alpha(image: &RgbaImage, x: u32, y: u32) -> u8 {
        image.get_pixel(x, y).0[3]
    }

    #[test]
    fn constrain_square_keeps_direction() {
        assert_eq!(constrain_square((10.0, 10.0), (40.0, 20.0)), (40.0, 40.0));
        assert_eq!(constrain_square((10.0, 10.0), (0.0, -30.0)), (-30.0, -30.0));
    }

    #[test]
    fn rectangle_is_an_outline() {
        let image = draw(Shape::Rectangle((20.0, 20.0), (80.0, 60.0)), &RED);
        assert_eq!(image.get_pixel(20, 40).0, [255, 0, 0, 255], "left edge");
        assert_eq!(alpha(&image, 50, 60), 255, "bottom edge");
        assert_eq!(alpha(&image, 50, 40), 0, "not filled");
        assert_eq!(alpha(&image, 10, 10), 0, "nothing outside");
    }

    #[test]
    fn ellipse_and_circle() {
        let image = draw(Shape::Ellipse((10.0, 30.0), (90.0, 70.0)), &RED);
        assert_eq!(alpha(&image, 10, 50), 255, "left end");
        assert_eq!(alpha(&image, 50, 30), 255, "top");
        assert_eq!(alpha(&image, 50, 50), 0, "not filled");
        assert_eq!(
            alpha(&image, 12, 32),
            0,
            "corner of the bounding box is outside"
        );

        let end = constrain_square((10.0, 10.0), (90.0, 40.0));
        let circle = draw(Shape::Ellipse((10.0, 10.0), end), &RED);
        assert_eq!(
            alpha(&circle, 50, 90),
            255,
            "Shift makes it as tall as wide"
        );
    }

    #[test]
    fn arrow_has_line_and_head() {
        let image = draw(Shape::Arrow((10.0, 50.0), (90.0, 50.0)), &RED);
        assert_eq!(alpha(&image, 30, 50), 255, "shaft");
        assert_eq!(alpha(&image, 30, 56), 0, "shaft is only 4 px wide");
        assert_eq!(alpha(&image, 75, 55), 255, "head is wider than the shaft");
        assert_eq!(alpha(&image, 93, 50), 0, "nothing past the tip");
    }

    #[test]
    fn translucent_color_does_not_double_up_where_parts_overlap() {
        let style = ShapeStyle {
            color: [0, 0, 255, 128],
            width: 6.0,
        };
        let image = draw(Shape::Arrow((10.0, 50.0), (90.0, 50.0)), &style);
        // Where the shaft ends under the head, it is no more opaque than elsewhere.
        assert_eq!(alpha(&image, 30, 50), 128);
        assert_eq!(alpha(&image, 70, 50), 128);
    }

    #[test]
    fn brush_click_paints_a_dot_and_strokes_follow_the_mouse() {
        let dot = draw(Shape::Brush(vec![(50.0, 50.0)]), &RED);
        assert_eq!(alpha(&dot, 50, 50), 255);
        assert_eq!(alpha(&dot, 56, 50), 0);

        let stroke = draw(
            Shape::Brush(vec![(10.0, 10.0), (50.0, 10.0), (50.0, 80.0)]),
            &RED,
        );
        assert_eq!(alpha(&stroke, 30, 10), 255);
        assert_eq!(alpha(&stroke, 50, 60), 255);
        assert_eq!(alpha(&stroke, 30, 60), 0);
    }

    #[test]
    fn shapes_are_clipped_to_the_canvas() {
        let shape = Shape::Rectangle((-50.0, -50.0), (50.0, 50.0));
        let (image, origin) = rasterize(&shape.primitives(4.0), &RED, (100, 100)).unwrap();
        assert_eq!(origin, (0, 0));
        assert_eq!(
            image.dimensions(),
            (54, 54),
            "up to the edge plus half the width and AA"
        );

        let outside = Shape::Rectangle((200.0, 200.0), (300.0, 300.0));
        assert!(rasterize(&outside.primitives(4.0), &RED, (100, 100)).is_none());
    }

    #[test]
    fn hit_test_lines_and_outlines() {
        let arrow = Shape::Arrow((10.0, 50.0), (90.0, 50.0)).primitives(4.0);
        assert!(
            hit_test(&arrow, (50.0, 53.0), 4.0, 2.0),
            "just next to the shaft"
        );
        assert!(
            !hit_test(&arrow, (50.0, 60.0), 4.0, 2.0),
            "too far from the shaft"
        );
        assert!(hit_test(&arrow, (85.0, 51.0), 4.0, 0.0), "on the head");

        let rect = Shape::Rectangle((20.0, 20.0), (80.0, 60.0)).primitives(4.0);
        assert!(hit_test(&rect, (50.0, 40.0), 4.0, 2.0), "inside counts");
        assert!(hit_test(&rect, (81.0, 40.0), 4.0, 2.0), "on the edge");
        assert!(!hit_test(&rect, (90.0, 40.0), 4.0, 2.0));

        let brush = Shape::Brush(vec![(10.0, 10.0), (50.0, 10.0), (50.0, 50.0)]).primitives(4.0);
        assert!(
            !hit_test(&brush, (30.0, 30.0), 4.0, 2.0),
            "open lines have no inside"
        );
    }

    #[test]
    fn translate_moves_everything_and_bounds_follow() {
        let mut arrow = Shape::Arrow((10.0, 50.0), (90.0, 50.0)).primitives(4.0);
        let before = bounds(&arrow, 4.0, 0.0).unwrap();
        translate(&mut arrow, 5.0, -10.0);
        let after = bounds(&arrow, 4.0, 0.0).unwrap();
        assert_eq!(after.0, before.0 + 5.0);
        assert_eq!(after.1, before.1 - 10.0);
        assert_eq!(after.2, before.2 + 5.0);
        assert_eq!(after.3, before.3 - 10.0);
        assert!(bounds(&[], 4.0, 0.0).is_none());
    }

    #[test]
    fn smoothing_evens_out_jitter_but_keeps_the_ends() {
        // A straight line with half a pixel of zigzag, like a hand tremor at high zoom.
        let jittery: Vec<Point> = (0..=40)
            .map(|i| (i as f32, if i % 2 == 0 { 10.25 } else { 9.75 }))
            .collect();
        let smoothed = smooth(&jittery);
        assert_eq!(smoothed.first(), jittery.first());
        assert_eq!(smoothed.last(), jittery.last());
        let inner = &smoothed[3..smoothed.len() - 3];
        let wobble = inner.iter().map(|p| (p.1 - 10.0).abs()).fold(0.0, f32::max);
        assert!(wobble <= 0.13, "zigzag of ±0.25 reduced to ±{wobble}");

        // Short strokes are left alone.
        let two = [(0.0, 0.0), (5.0, 5.0)];
        assert_eq!(smooth(&two), two);
    }

    #[test]
    fn tiny_shapes_draw_nothing() {
        assert!(
            Shape::Rectangle((5.0, 5.0), (5.5, 30.0))
                .primitives(4.0)
                .is_empty()
        );
        assert!(
            Shape::Ellipse((5.0, 5.0), (5.0, 5.0))
                .primitives(4.0)
                .is_empty()
        );
        assert!(
            Shape::Arrow((5.0, 5.0), (5.2, 5.0))
                .primitives(4.0)
                .is_empty()
        );
    }
}
