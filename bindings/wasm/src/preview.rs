//! Pictures the editor's panels show that only the engine can draw (upstream drew them as
//! textures with its own renderer and text engine): layer and asset thumbnails
//! (`panels/assets_panel.rs` `thumbnail_pixmap`), text in its own font for the font list and the
//! text style browser (`widgets/font_preview.rs`), and the colour under a point for the colour
//! picker's eyedropper (`canvas/tools/click.rs` `eyedrop`). None of them changes the document.

use kition_design_color::{Color, Paint};
use kition_design_doc::{Document, Node, NodeId, NodeKind, StyleKind, TextData, TextStyle};
use serde_json::Value;

use crate::DesignDocument;

/// The largest thumbnail side, in pixels.
pub const MAX_THUMBNAIL: f64 = 2048.0;
/// The largest text preview: upstream's one line of at most 400 × 60 points, at up to 3x.
pub const MAX_PREVIEW_WIDTH: f64 = 2000.0;
pub const MAX_PREVIEW_HEIGHT: f64 = 400.0;

fn png(mut pix: kition_design_render::Pixmap, profile: kition_design_color::ColorProfile) -> Result<Vec<u8>, String> {
    kition_design_render::to_srgb(&mut pix, profile);
    kition_design_render::encode_png(&pix)
}

/// Layer `id` drawn to fit a `size` × `size` square (at most 2x its own size), as a transparent
/// PNG: the Assets tab's tiles, instance swap menus and layer thumbnails. A slice shows the layers
/// its first export setting writes (it paints nothing itself).
pub fn node_thumbnail(doc: &DesignDocument, id: &str, size: f64) -> Result<Vec<u8>, String> {
    let id: NodeId = id.parse().map_err(|_| format!("invalid-params: {id:?} is not a layer id"))?;
    thumbnail_of(&doc.ds()?.doc, id, size)
}

/// Layer `id` of `d` drawn to fit a `size` × `size` square, as [`node_thumbnail`] draws it (also
/// for a library's snapshot, [`crate::libraries`]).
pub(crate) fn thumbnail_of(d: &Document, id: NodeId, size: f64) -> Result<Vec<u8>, String> {
    let size = if size.is_finite() && size >= 1.0 { size.min(MAX_THUMBNAIL) } else { 64.0 };
    let n = d.node(id).ok_or_else(|| format!("not-found: no layer {id}"))?;
    if n.is_page() {
        return Err(format!("invalid-params: {id} is a page"));
    }
    let slice = matches!(n.kind, NodeKind::Slice);
    let b = if slice { d.world_bounds(id) } else { kition_design_render::export_bounds(d, id) }.ok_or_else(|| format!("layer {id} draws nothing"))?;
    let side = b.width().max(b.height());
    if !(side.is_finite() && side > 0.0) {
        return Err(format!("layer {id} draws nothing"));
    }
    let scale = (size / side).min(2.0);
    let pix = if slice {
        let setting = n.exports.first().cloned().unwrap_or_default();
        let layers = kition_design_engine::cmd::file::drawn_layers(d, &[id], &setting);
        kition_design_render::render_area(d, &layers, b, scale)
    } else {
        kition_design_render::render_node(d, id, scale)
    };
    png(pix.ok_or_else(|| format!("cannot draw layer {id} at {size} px"))?, d.color_profile)
}

fn color_param(params: &Value) -> Result<Color, String> {
    match params.get("color") {
        None | Some(Value::Null) => Ok(Color::BLACK),
        Some(Value::String(s)) => Color::parse_css(s).ok_or_else(|| format!("invalid-params: `color` {s:?} is not a colour")),
        Some(other) => Err(format!("invalid-params: `color` must be a CSS colour, not {other}")),
    }
}

/// Text drawn in its own font on a transparent background, one line (`textPreview`), as a PNG.
/// `params` (JSON):
///
/// - `family`, `style?` (default the family's Regular, else its first style), `size?` (default 13):
///   a font list row; `text?` defaults to the family's name. A family that does not draw itself
///   (a symbol font, one the engine resolves to another family, one missing glyphs of its name)
///   is an error (`no-preview: …`): show the name in the panel's own font then, as upstream does.
/// - or `styleId`: a text style's typography (`text?` defaults to the style's name; `size?`
///   overrides its size); with `library`, the style that enabled library publishes.
/// - `color?` (CSS, default `#000000`), `scale?` (device pixels per CSS pixel, 1–3, default 1),
///   `maxWidth?` (default 400) and `maxHeight?` (default 60) in CSS pixels: longer text is cut.
pub fn text_preview(doc: &DesignDocument, params: &Value) -> Result<Vec<u8>, String> {
    if !params.is_object() {
        return Err("invalid-params: params must be an object".into());
    }
    let string = |key: &str| params.get(key).and_then(Value::as_str).map(str::to_owned);
    let number = |key: &str| params.get(key).and_then(Value::as_f64).filter(|v| v.is_finite());
    let (mut style, default_text) = if let Some(style_id) = string("styleId") {
        // `library`: a style an enabled library publishes (the style picker's library sections).
        let from = match string("library") {
            Some(id) => Some(
                crate::libraries::enabled(doc)
                    .into_iter()
                    .find(|(link, _)| link.id == id)
                    .ok_or_else(|| format!("not-found: no enabled library {id}"))?
                    .1?,
            ),
            None => None,
        };
        let d = match &from {
            Some(lib) => lib.doc.as_ref(),
            None => &doc.ds()?.doc,
        };
        let s = d.style(&style_id).ok_or_else(|| format!("not-found: no style {style_id}"))?;
        let StyleKind::Text { style } = &s.kind else { return Err(format!("invalid-params: style {style_id} is not a text style")) };
        (style.clone(), s.name.rsplit('/').next().unwrap_or(&s.name).trim().to_owned())
    } else {
        let family = string("family").filter(|f| !f.trim().is_empty()).ok_or("invalid-params: pass `family` or `styleId`")?;
        let styles = kition_design_text::family_styles(&family);
        let font_style = match string("style") {
            Some(s) => s,
            None => styles.iter().find(|s| s.eq_ignore_ascii_case("Regular")).or(styles.first()).cloned().unwrap_or_else(|| "Regular".into()),
        };
        let face = kition_design_text::find_face(&family, &font_style).ok_or_else(|| format!("no-preview: no font {family}"))?;
        let text = string("text").unwrap_or_else(|| family.clone());
        if !face.family.eq_ignore_ascii_case(&family) || !text.chars().all(|c| c.is_whitespace() || face.has_char(c)) {
            return Err(format!("no-preview: {family} does not draw this text"));
        }
        (TextStyle { font_family: family.clone(), font_style, font_size: 13.0, ..TextStyle::default() }, family)
    };
    if let Some(size) = number("size").filter(|s| *s > 0.0) {
        style.font_size = size.min(512.0);
    }
    let text = string("text").unwrap_or(default_text);
    if text.is_empty() {
        return Err("invalid-params: the text is empty".into());
    }
    let text: String = text.chars().take(200).collect();
    let color = color_param(params)?;
    let scale = number("scale").unwrap_or(1.0).clamp(1.0, 3.0);
    let max_w = number("maxWidth").unwrap_or(400.0).clamp(1.0, MAX_PREVIEW_WIDTH);
    let max_h = number("maxHeight").unwrap_or(60.0).clamp(1.0, MAX_PREVIEW_HEIGHT);
    let mut d = Document::new("preview");
    let page = d.first_page().ok_or("cannot make a preview document")?;
    let id = d.alloc_id();
    let mut td = TextData::new(text);
    td.style = TextStyle { fills: None, link: None, style_id: None, ..style };
    let mut n = Node::new(id, NodeKind::Text(td), "preview", 1.0, 1.0);
    n.fills = vec![Paint::solid(color)];
    let (w, h) = kition_design_text::auto_size(&n).ok_or("no-preview: the text does not lay out")?;
    n.width = w.min(max_w);
    n.height = h.min(max_h);
    d.insert(n, page, None).map_err(|e| e.to_string())?;
    let pix = kition_design_render::render_node(&d, id, scale).ok_or("no-preview: nothing to draw")?;
    kition_design_render::encode_png(&pix)
}

/// The colour the canvas shows at document pixel (`x`, `y`) at `zoom`, as the eyedropper samples
/// it (the page's background under transparent layers): `#rrggbb`.
pub fn sample_pixel(doc: &DesignDocument, x: f64, y: f64, zoom: f64) -> Result<String, String> {
    if !(x.is_finite() && y.is_finite()) {
        return Err("invalid-params: the point is not finite".into());
    }
    let zoom = if zoom.is_finite() && zoom > 0.0 { zoom.clamp(0.01, 256.0) } else { 1.0 };
    let rgba = crate::render::render(doc, x, y, zoom.max(1.0), 1, 1)?;
    let [r, g, b] = match rgba.get(..3) {
        Some([r, g, b]) => [*r, *g, *b],
        _ => return Err("the renderer answered no pixel".into()),
    };
    Ok(format!("#{r:02x}{g:02x}{b:02x}"))
}
