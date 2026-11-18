//! Drawing the player's screen (upstream's `present/draw.rs` drew it with egui textures): the
//! presented frame on the prototype's background, each open overlay over its background colour,
//! and a transition's two screens composed at a point of it.

use kition_design_color::Color;
use kition_design_geom::Rect;
use kition_design_render::Pixmap;

use super::Screen;

/// The longest side of a player picture, in pixels.
pub const MAX_SIDE: u32 = 8192;
/// The most pixels a player picture has.
pub const MAX_PIXELS: u64 = 32 * 1024 * 1024;

/// The most pixels a picture inside a transition has: drawing one holds both screens and the
/// picture at once, so it is drawn at a lower scale than the screen alone may be.
pub const MAX_TRANSITION_PIXELS: u64 = MAX_PIXELS / 4;

/// `scale`, lowered as little as it must be for `screen` drawn at it to have at most `budget`
/// pixels ([`pixel_size`]'s rounding).
pub fn fit_scale(screen: (f64, f64), scale: f64, budget: u64) -> f64 {
    let side = |v: f64, s: f64| (v * s - 1e-6).ceil().max(1.0);
    let pixels = |s: f64| side(screen.0, s) * side(screen.1, s);
    #[allow(clippy::cast_precision_loss)]
    let budget = budget as f64;
    if !(scale.is_finite() && scale > 0.0) || pixels(scale) <= budget {
        return scale;
    }
    let (mut fits, mut over) = (0.0, scale);
    for _ in 0..60 {
        let mid = (fits + over) / 2.0;
        if pixels(mid) <= budget { fits = mid } else { over = mid }
    }
    if fits > 0.0 { fits } else { scale }
}

/// The pixel size of `screen` at `scale`, or why it cannot be drawn.
pub fn pixel_size(screen: (f64, f64), scale: f64) -> Result<(u32, u32), String> {
    let side = |v: f64| (v * scale - 1e-6).ceil().max(1.0);
    let (w, h) = (side(screen.0), side(screen.1));
    if !(w.is_finite() && h.is_finite()) || w > f64::from(MAX_SIDE) || h > f64::from(MAX_SIDE) || (w * h) as u64 > MAX_PIXELS {
        return Err(format!("invalid-params: the picture would be {w} x {h} pixels; use a smaller `scale`"));
    }
    Ok((w as u32, h as u32))
}

/// A colour as premultiplied RGBA8 at `alpha`.
fn premultiplied(c: Color, alpha: f64) -> [u8; 4] {
    let [r, g, b, a] = c.to_rgba8();
    let a = (f64::from(a) * alpha.clamp(0.0, 1.0)).round();
    let mul = |v: u8| (f64::from(v) * a / 255.0).round() as u8;
    [mul(r), mul(g), mul(b), a as u8]
}

/// Source-over of premultiplied `src` onto `dst`.
fn over(dst: &mut [u8], src: [u8; 4]) {
    let inv = 255 - u32::from(src[3]);
    for (d, s) in dst.iter_mut().zip(src) {
        *d = (u32::from(s) + (u32::from(*d) * inv + 127) / 255).min(255) as u8;
    }
}

/// Fills `dst` with `color` at `alpha` (over what is there).
pub fn fill(dst: &mut Pixmap, color: Color, alpha: f64) {
    let src = premultiplied(color, alpha);
    if src[3] == 0 {
        return;
    }
    for px in dst.data_mut().chunks_exact_mut(4) {
        over(px, src);
    }
}

/// Draws `src` over `dst` with its top-left at pixel (`x`, `y`), at `alpha`.
pub fn blit(dst: &mut Pixmap, src: &Pixmap, x: i64, y: i64, alpha: f64) {
    let alpha = if alpha.is_finite() { alpha.clamp(0.0, 1.0) } else { 1.0 };
    if alpha <= 0.0 {
        return;
    }
    let k = (alpha * 256.0).round() as u32;
    let (dw, dh) = (i64::from(dst.width()), i64::from(dst.height()));
    let (sw, sh) = (i64::from(src.width()), i64::from(src.height()));
    let (x0, y0) = (x.max(0), y.max(0));
    let (x1, y1) = ((x + sw).min(dw), (y + sh).min(dh));
    if x0 >= x1 || y0 >= y1 {
        return;
    }
    let src_data = src.data();
    let dst_data = dst.data_mut();
    for row in y0..y1 {
        let s_start = (((row - y) * sw + (x0 - x)) * 4) as usize;
        let d_start = ((row * dw + x0) * 4) as usize;
        let len = ((x1 - x0) * 4) as usize;
        let (Some(s), Some(d)) = (src_data.get(s_start..s_start + len), dst_data.get_mut(d_start..d_start + len)) else { continue };
        for (dp, sp) in d.chunks_exact_mut(4).zip(s.chunks_exact(4)) {
            let scaled = |v: u8| if k >= 256 { v } else { ((u32::from(v) * k) >> 8) as u8 };
            let px = [scaled(sp[0]), scaled(sp[1]), scaled(sp[2]), scaled(sp[3])];
            if px[3] != 0 {
                over(dp, px);
            }
        }
    }
}

/// One overlay drawn moved by (`dx`, `dy`) screen units and at `alpha` (a transition).
#[derive(Clone, Copy, Debug)]
pub struct Moved {
    pub index: usize,
    pub dx: f64,
    pub dy: f64,
    pub alpha: f64,
}

/// `screen` drawn at `scale`: the background, the presented frame, then each overlay over its
/// background colour (`moved` shifts and fades one of them). In the document's colour space.
pub fn compose(screen: &Screen, scale: f64, moved: Option<Moved>) -> Result<Pixmap, String> {
    let (w, h) = pixel_size(screen.size, scale)?;
    let mut pix = Pixmap::new(w, h).ok_or_else(|| format!("cannot draw {w} x {h}"))?;
    fill(&mut pix, screen.background, 1.0);
    let f = &screen.frame;
    let area = Rect::new(
        f.page_min.0 - f.origin.0,
        f.page_min.1 - f.origin.1,
        f.page_min.0 - f.origin.0 + screen.size.0,
        f.page_min.1 - f.origin.1 + screen.size.1,
    );
    if let Some(frame) = kition_design_render::render_area(&screen.disp, &[screen.current], area, scale) {
        blit(&mut pix, &frame, 0, 0, 1.0);
    }
    for (i, o) in screen.overlays.iter().enumerate() {
        let m = moved.filter(|m| m.index == i);
        let (dx, dy, alpha) = m.map_or((0.0, 0.0, 1.0), |m| (m.dx, m.dy, m.alpha));
        if let Some(bg) = o.background {
            fill(&mut pix, bg, alpha);
        }
        let ob = Rect::new(o.place.page_min.0, o.place.page_min.1, o.place.page_min.0 + o.rect.width(), o.place.page_min.1 + o.rect.height());
        if let Some(over) = kition_design_render::render_area(&screen.disp, &[o.id], ob, scale) {
            blit(&mut pix, &over, ((o.rect.x0 + dx) * scale).round() as i64, ((o.rect.y0 + dy) * scale).round() as i64, alpha);
        }
    }
    Ok(pix)
}
