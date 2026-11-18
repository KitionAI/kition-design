//! `render` and `render_area`: the current page drawn as the upstream canvas draws it (the page's
//! background colour, outline mode and the Dev Mode focus view as the View menu sets them, the text
//! being composed), in sRGB, straight alpha.
//!
//! Above 1:1 the page is drawn at the zoom, so it stays sharp. Below, it is drawn at 1:1 and
//! reduced exactly (the contract's box mean), as long as the 1:1 area stays under [`TILE_LIMIT`]
//! pixels; a larger area (the whole of a big page as a thumbnail) is drawn straight at its scale.

use kition_design_render::{Options, Outline, View};
use kition_kit::MAX_RENDER_SIDE;
use kition_kit::raster::{self, Frame};

use crate::DesignDocument;

/// The most pixels a 1:1 tile may have before an area is drawn straight at its scale.
pub const TILE_LIMIT: u64 = 4096 * 4096;

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
    #[cfg(target_arch = "wasm32")]
    let rendered = kition_design_render::render_view_single(shown, ds.page, &view, &options);
    #[cfg(not(target_arch = "wasm32"))]
    let rendered = kition_design_render::render_view(shown, ds.page, &view, &options);
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
    if scale >= 1.0 {
        return draw(doc, x, y, scale, width, height);
    }
    let x0 = x.floor() as i32;
    let y0 = y.floor() as i32;
    let span_x = ((f64::from(width) / scale).ceil().min(f64::from(i32::MAX / 2)) as i32).max(1);
    let span_y = ((f64::from(height) / scale).ceil().min(f64::from(i32::MAX / 2)) as i32).max(1);
    render_area(doc, x0, y0, x0.saturating_add(span_x), y0.saturating_add(span_y), width, height)
}

pub fn render_area(doc: &DesignDocument, x0: i32, y0: i32, x1: i32, y1: i32, width: u32, height: u32) -> Result<Vec<u8>, String> {
    let (width, height) = (width.clamp(1, MAX_RENDER_SIDE), height.clamp(1, MAX_RENDER_SIDE));
    let span_x = u32::try_from(x1.saturating_sub(x0).max(1)).unwrap_or(1);
    let span_y = u32::try_from(y1.saturating_sub(y0).max(1)).unwrap_or(1);
    if span_x == width && span_y == height {
        return draw(doc, f64::from(x0), f64::from(y0), 1.0, width, height);
    }
    let fits = u64::from(span_x) * u64::from(span_y) <= TILE_LIMIT && span_x <= 16_384 && span_y <= 16_384;
    if fits {
        let full = draw(doc, f64::from(x0), f64::from(y0), 1.0, span_x, span_y)?;
        let frame = Frame { rgba: &full, width: span_x, height: span_y };
        return Ok(raster::render_area(frame, 0, 0, span_x as i32, span_y as i32, width, height));
    }
    // Too large to draw at 1:1: straight at the scale that fits the area into the output.
    let scale = (f64::from(width) / f64::from(span_x)).min(f64::from(height) / f64::from(span_y));
    draw(doc, f64::from(x0), f64::from(y0), scale, width, height)
}
