//! `render` and `render_area`: the current page drawn as the upstream canvas draws it (the page's
//! background colour, outline mode and the Dev Mode focus view as the View menu sets them, the text
//! being composed), in sRGB, straight alpha.
//!
//! Every scale is drawn straight at that scale, zoomed out too (the canvas's level of detail
//! applies there), so a frame at 50 % costs the pixels it has, not four times as many drawn at 1:1
//! and reduced (stage 4 of the hand plan, M1/M2). The canvas patches its cached frame with
//! `render_area` at the frame's own whole-number reduction: the patch is drawn on the same pixel
//! grid as the frame, so it matches a full render of the frame.

use kition_design_render::{Options, Outline, View};
use kition_kit::MAX_RENDER_SIDE;

use crate::DesignDocument;

/// `width × height` straight-alpha pixels of the area whose top-left is document pixel (`x`, `y`),
/// `scale` screen pixels per document pixel.
fn draw(doc: &DesignDocument, x: f64, y: f64, scale: f64, width: u32, height: u32) -> Result<Vec<u8>, String> {
    let ds = doc.ds()?;
    let area = doc.area();
    let preedit = kition_design_engine::textedit::preedit_doc(doc.session());
    let shown = preedit.as_ref().map_or(&*ds.doc, |(d, _, _)| d);
    let view = View { origin: (area.x + x, area.y + y), scale, width, height };
    let options = Options {
        outline_mode: ds.view.outlines,
        outline: Outline { hidden: ds.view.outline_hidden, bounds: ds.view.outline_bounds, light: false },
        outlined: if ds.view.outlines_selection { ds.selection.clone() } else { Vec::new() },
        solo: ds.focused(),
        lod: scale < 1.0,
        ..Options::default()
    };
    // A window onto the page: a patch of the canvas's frame matches the frame's own pixels.
    let rendered = kition_design_render::render_window(shown, ds.page, &view, &options);
    let mut pixmap = rendered.ok_or_else(|| format!("cannot render {width} x {height}"))?;
    kition_design_render::to_srgb(&mut pixmap, shown.color_profile);
    let rgba = kition_design_render::to_rgba8(&pixmap);
    if rgba.len() != width as usize * height as usize * 4 {
        return Err(format!("the renderer answered {} bytes for {width} x {height}", rgba.len()));
    }
    Ok(rgba)
}

pub fn render(doc: &DesignDocument, x: f64, y: f64, scale: f64, width: u32, height: u32) -> Result<Vec<u8>, String> {
    let (width, height) = (width.clamp(1, MAX_RENDER_SIDE), height.clamp(1, MAX_RENDER_SIDE));
    let scale = if scale.is_finite() && scale > 0.0 { scale } else { 1.0 };
    let (x, y) = (if x.is_finite() { x } else { 0.0 }, if y.is_finite() { y } else { 0.0 });
    draw(doc, x, y, scale, width, height)
}

/// The integer area (`x0`, `y0`)–(`x1`, `y1`) in `width` × `height` pixels: drawn at 1:1 when it is
/// that size, else straight at the scale that fits it (the same scale both ways for the canvas's
/// whole-number reductions).
pub fn render_area(doc: &DesignDocument, x0: i32, y0: i32, x1: i32, y1: i32, width: u32, height: u32) -> Result<Vec<u8>, String> {
    let (width, height) = (width.clamp(1, MAX_RENDER_SIDE), height.clamp(1, MAX_RENDER_SIDE));
    let span_x = u32::try_from(x1.saturating_sub(x0).max(1)).unwrap_or(1);
    let span_y = u32::try_from(y1.saturating_sub(y0).max(1)).unwrap_or(1);
    let scale = (f64::from(width) / f64::from(span_x)).min(f64::from(height) / f64::from(span_y));
    draw(doc, f64::from(x0), f64::from(y0), scale, width, height)
}
