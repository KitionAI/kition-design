//! File menu: documents, save/open, import/export, saved versions (the host's checkpoints).

use std::sync::Arc;

use base64::Engine as _;
use kition_design_doc::*;
use serde_json::{Value, json};

use super::{CommandSpec, ok};
use crate::params::*;
use crate::{DocState, EngineError, Result, Session, bad, spec};

mod export;
mod load;
mod open;
mod save;
mod version;

pub(crate) use export::FORBIDDEN_FILE_NAME_CHARS;
#[cfg(test)]
use export::add_member;
pub(super) use export::export_setting;
pub use export::{
    ExportJob, copy_png, drawn_layers, export_area, export_bytes, export_bytes_with, export_jobs, export_scale, exportable_layers, fig_bytes,
    frames_pdf, jobs_for, safe_file_name,
};
use export::{export, export_frames_pdf, export_layers, pdf_bytes};
#[cfg(test)]
pub(crate) use load::{installed_family, sketch_line};
pub use load::{load, load_bytes, load_path};
pub(crate) use load::{load_fig, read_capped, recovered, variables_param, write};
pub use open::MAX_RECENT;
#[cfg(test)]
use open::append_pages;
use open::{import, open, open_recent};
pub(crate) use save::save_ends_preview;
pub use save::{FileSnapshot, file_bytes, thumbnail};
use save::{color_profile, mark_saved, save, save_as};
pub use version::content_at;
#[cfg(test)]
pub(crate) use version::saved_version;
use version::{preview_version, restore_version};

/// A new file of `kind` in a tab of its own.
fn new_file(s: &mut Session, p: &Value, kind: FileKind) -> Result<Value> {
    let name = string(p, "name").unwrap_or_else(|| if kind == FileKind::Board { "Untitled board" } else { "Untitled" }.into());
    let mut doc = Document::new(name);
    doc.kind = kind;
    // A property of the page from here on: a later change of theme leaves it alone. Boards are
    // light whatever the theme.
    let background = if kind == FileKind::Board { BOARD_PAGE } else { s.new_page_background() };
    for page in doc.pages.clone() {
        if let Some(NodeKind::Page { background: b, .. }) = doc.node_mut(page).map(|n| &mut n.kind) {
            *b = background;
        }
    }
    s.docs.push(DocState::new(doc, None));
    s.active = s.docs.len() - 1;
    Ok(json!({"index": s.active, "kind": kind.id()}))
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        spec!("file.new", "New design file", "{name?}", false, |s, p| new_file(s, p, FileKind::Design)),
        spec!(
            "file.newBoard",
            "New board",
            "{name?}  a board: sticky notes, shapes with text, connectors, marker and stamps on a light page",
            false,
            |s, p| new_file(s, p, FileKind::Board)
        ),
        spec!(
            "file.open",
            "Open",
            "{path | data (base64) + name (tells the format; opens as a new unsaved document), variables? (REST: the local variables JSON as an object, JSON text or a path; default `<name>.variables.json` or `variables.json` next to the file)} (.kdesign, .json: Kition Design or REST API file JSON, .fig (best effort), .sketch (Sketch 48 or newer, best effort), .svg, .png, .jpg …); REST, .fig and .sketch imports return `warnings`; a file opened anew returns `summary` {name, pages, layers, images} (one that was already open just returns its `index`)",
            false,
            open
        ),
        spec!(
            "file.openRecent",
            "Open recent",
            "{path? | index? (into the list), clear? (forget them all), remove? (a path: forget that one)}  opens a recent file; a file that no longer exists is dropped from the list. Returns the list, newest first: {files: [{path, name, exists}]} (opening returns what `file.open` does)",
            false,
            open_recent
        ),
        spec!(
            "file.save",
            "Save",
            "{path?, overwrite? (replace what another app wrote to the file since it was opened or saved: without it that save is refused)} (programmatic saves need a path the first time); a .fig path writes an experimental export copy of the shown document with `warnings`, keeping the native path, unsaved changes and version preview",
            false,
            save
        ),
        spec!(
            "file.saveAs",
            "Save as",
            "{path | data: true (no path: returns the file as base64 `data` with its `name`, for a download)} (no extension adds .kdesign); a .fig path writes an experimental export copy of the shown document with `warnings`, keeping the native path, unsaved changes and version preview",
            false,
            save_as
        ),
        spec!(
            "file.markSaved",
            "Mark saved",
            "{}  clears the unsaved changes of the active tab's file: run it once the file from `file.saveAs {data: true}` was delivered (a refused download keeps them)",
            false,
            mark_saved
        ),
        spec!(
            "file.export",
            "Export",
            "{ids? (default selection, else the page), format? (png|jpg|svg|pdf|webp|xml (an Android Vector Drawable, with `warnings` for what it changed or left out), or fig: the whole file as .fig, experimental, with `warnings` for what was left out; other apps may not open every file), scale? (default 1) | size? (2x|512w|512h or {by: scale|width|height, value}: a scale, or a width or height in pixels), path? (else returns base64 `data`), bleed? (PDF: points, or with a unit: \"3mm\", \"0.125in\", \"9pt\"), cropMarks?, bleedMarks?, cmyk? (PDF print options), svgIdAttribute? (SVG ids, XML android:name), vdCompat? (XML: bestEffort (default: approximate with a warning)|exact (leave out what can't be exact, with a warning)|ignore (no warnings))}",
            false,
            export
        ),
        spec!(
            "file.exportLayers",
            "Export layers",
            "{ids? (default the selection, else every layer on the page with export settings), assets? (true: the Inspect tab's assets in and under them instead: layers with export settings, icons as SVG, images as PNG), dir? (folder to write into; else each file comes back as base64 `data`), zip? (no dir: the files come back together as one base64 zip `data` instead), bleed? (PDF: points, or with a unit: \"3mm\", \"0.125in\", \"9pt\"), cropMarks?, bleedMarks?, cmyk? (PDF print options): for every PDF instead of each setting's own, vdCompat? (bestEffort|exact|ignore: for every XML instead of each setting's own)}  every layer once per export setting, named `<layer><suffix>.<format>` (\"@2x\" when a scaled export has no suffix; repeats get \" 2\"; slashes in a layer name make folders). Returns {files: [{name, id, bytes, path | data, warnings (XML only)}], skipped (layers without settings), data? (zip)}",
            false,
            export_layers
        ),
        spec!("file.exportSvg", "Export selection as SVG", "{ids?, path?}", false, |s, p| {
            let mut q = p.clone();
            if let Some(o) = q.as_object_mut() {
                o.insert("format".into(), json!("svg"));
            }
            export(s, &q)
        }),
        spec!(
            "file.exportFramesPdf",
            "Export frames to PDF",
            "{path?, page? (page id, default the current page), bleed? (PDF: points, or with a unit: \"3mm\", \"0.125in\", \"9pt\"), cropMarks?, bleedMarks?, cmyk? (PDF print options)}  every top-level frame in reading order, one PDF page each; returns base64 `data` without a path",
            false,
            export_frames_pdf
        ),
        spec!(
            "file.import",
            "Import",
            "{path?, data? (base64, instead of path), name? (with data: the file's name), x?, y?, center? (SVG: centre it on x, y), variables? (REST: the local variables JSON as an object, JSON text or a path)}  SVG becomes an editable frame (shapes, text, gradients, masks, effects; with `warnings` for what it left out), images become image layers, .kdesign, REST API file JSON, .fig and .sketch pages are appended with their variables and flows (REST, .fig and .sketch: with `warnings`; those return `summary` {name, pages, layers, images} of the file read)",
            true,
            import
        ),
        spec!(
            "file.placeImage",
            "Place image",
            "{data? (base64 of the image), name? (the image's name), x?, y?, width?, height?, mode? (as image.place)}",
            true,
            super::create::place_image
        ),
        spec!("file.close", "Close tab", "{index?, force? (discard unsaved changes)}", false, close),
        spec!("file.moveTab", "Move tab", "{from, to} (indices among open documents)", false, |s, p| {
            let from = req_index("file.moveTab", p, "from")?;
            let to = req_index("file.moveTab", p, "to")?;
            if from >= s.docs.len() || to >= s.docs.len() {
                return Err(bad("file.moveTab", "no such document"));
            }
            if from != to {
                let active_uid = s.docs.get(s.active).map(|d| d.uid);
                let doc = s.docs.remove(from);
                s.docs.insert(to, doc);
                s.active = active_uid.and_then(|uid| s.docs.iter().position(|d| d.uid == uid)).unwrap_or(0);
            }
            Ok(json!({"index": to}))
        }),
        spec!(
            "file.recover",
            "Recover file",
            "{path}  open a recovery copy (written while the app runs, read back after a crash) as a document with unsaved changes and no path: saving asks where → {index}",
            false,
            |s, p| {
                let path = req_string("file.recover", p, "path")?;
                s.docs.push(recovered(&path)?);
                s.active = s.docs.len() - 1;
                Ok(json!({"index": s.active}))
            }
        ),
        spec!("file.rename", "Rename file", "{name}", true, |s, p| {
            let name = req_string("file.rename", p, "name")?;
            s.doc_mut()?.name = name;
            ok()
        }),
        spec!(
            "file.colorProfile",
            "Color profile",
            "{profile?: srgb|displayP3, convert? (default true: convert colour values so colours keep their appearance; false: keep the values and reinterpret them)}  without `profile`, returns the current one (and the app opens the dialog). Returns {profile, changed}",
            true,
            color_profile
        ),
        spec!("file.duplicate", "Duplicate file", "{}", false, |s, _| {
            let ds = s.ds()?;
            let mut d = (*ds.doc).clone();
            d.name = format!("{} (Copy)", d.name);
            s.docs.push(DocState::new(d, None));
            s.active = s.docs.len() - 1;
            Ok(json!({"index": s.active}))
        }),
        spec!("file.revert", "Revert to saved", "{}  leave the live session first, if any", false, |s, _| {
            let path = s.ds()?.path.clone().ok_or_else(|| EngineError::Disabled("file.revert".into(), "the file was never saved".into()))?;
            // Stamped before reading: a change made during the read makes the next save refuse.
            let on_disk = crate::library::stat(&path).ok();
            let (loaded, _) = load_path(&path, None)?;
            let i = s.active;
            if let Some(slot) = s.docs.get_mut(i) {
                *slot = DocState::new(loaded.doc, Some(path.clone()))
                    .with_replica(loaded.replica)
                    .with_images_left_in_file(&path, loaded.images_past_budget);
                slot.on_disk = on_disk;
            }
            ok()
        }),
        spec!(
            "file.saveVersion",
            "Save to version history",
            "{}  opens the version history's save form: the editor saves the version with the host (its checkpoints; nothing about versions is kept in the design or its file)",
            false,
            |s, _| {
                s.request_ui("panel.versionHistory", json!({"save": true}));
                ok()
            }
        ),
        spec!("file.versionHistory", "Show version history", "{}  opens the panel (the host keeps the saved versions)", false, |s, _| {
            s.request_ui("panel.versionHistory", json!({}));
            ok()
        }),
        spec!(
            "version.restore",
            "Restore version",
            "{version (a replica version, base64, as the host's checkpoint names it)}  the design takes the content it had at that version, as one undoable edit (the file name, what it published and the comments stay); refused when the design's history here does not reach the version",
            true,
            restore_version
        ),
        spec!(
            "version.preview",
            "View version",
            "{version (base64), id?, name?, time? (the host's checkpoint, shown in the banner)}  show the design as it was at that version on the canvas in place of the document (read-only; `version.restore` keeps it, `version.exitPreview` goes back)",
            false,
            preview_version
        ),
        spec!("version.exitPreview", "Back to current version", "{}  end a version preview", false, |s, _| {
            Ok(json!({"ended": s.ds_mut()?.end_preview()}))
        }),
        spec!("doc.switch", "Switch document", "{index}", false, |s, p| {
            let i = req_index("doc.switch", p, "index")?;
            if i >= s.docs.len() {
                return Err(bad("doc.switch", "no such document"));
            }
            s.active = i;
            ok()
        }),
        spec!("doc.nextTab", "Next tab", "{}", false, |s, _| cycle_doc(s, 1)),
        spec!("doc.previousTab", "Previous tab", "{}", false, |s, _| cycle_doc(s, -1)),
        spec!("doc.list", "List documents", "{}", false, |s, _| {
            Ok(Value::Array(
                s.docs
                    .iter()
                    .enumerate()
                    .map(|(i, d)| json!({"index": i, "name": d.doc.name, "path": d.path, "dirty": d.is_dirty(), "active": i == s.active}))
                    .collect(),
            ))
        }),
        spec!("doc.home", "Home", "{}", false, |s, _| {
            s.request_ui("home", json!({}));
            ok()
        }),
    ]
}

/// The most pixels a raster export has (128 MiB as a 32-bit picture); a larger one is drawn at the
/// largest scale that fits.
pub const MAX_EXPORT_PIXELS: f64 = 32.0 * 1024.0 * 1024.0;

/// `scale`, lowered as little as it must be for `area` drawn at it (as `render_area` sizes it) to
/// have at most [`MAX_EXPORT_PIXELS`] pixels.
pub fn fit_export_scale(area: kition_design_geom::Rect, scale: f64) -> f64 {
    let pixels = |s: f64| (area.width() * s - 1e-6).ceil().max(1.0) * (area.height() * s - 1e-6).ceil().max(1.0);
    if !(scale.is_finite() && scale > 0.0) || pixels(scale) <= MAX_EXPORT_PIXELS {
        return scale;
    }
    // The pixel count grows with the scale: the largest scale that fits, by halving the interval.
    let (mut fits, mut over) = (0.0, scale);
    for _ in 0..60 {
        let mid = (fits + over) / 2.0;
        if pixels(mid) <= MAX_EXPORT_PIXELS { fits = mid } else { over = mid }
    }
    if fits > 0.0 { fits } else { scale }
}

/// [`export_bytes_with`] and what the export changed or left out (Android Vector Drawable only;
/// empty for the other formats).
pub fn export_file(doc: &Document, ids: &[NodeId], setting: &ExportSetting, scale: f64) -> Result<(Vec<u8>, Vec<String>)> {
    let nothing = || EngineError::Disabled("file.export".into(), "nothing to export".into());
    if ids.is_empty() {
        return Err(nothing());
    }
    let drawn = drawn_layers(doc, ids, setting);
    if setting.format.is_vector() {
        kition_design_render::check_export_depth(doc, &drawn).map_err(|e| EngineError::Other(e.to_string()))?;
    }
    let area = export_area(doc, ids, setting).ok_or_else(nothing)?;
    // The layers' own area unless an option or a slice says otherwise.
    let own_area = setting.contents_only
        && !setting.include_bounding_box
        && !ids.iter().any(|i| doc.node(*i).is_some_and(|n| matches!(n.kind, NodeKind::Slice)));
    match setting.format {
        // SVG and PDF colours are sRGB: a Display P3 file exports converted (out-of-gamut colours clip).
        ExportFormat::Svg => {
            let opts = kition_design_svg::SvgOptions {
                include_ids: setting.svg_id_attribute,
                outline_text: setting.svg_outline_text,
                simplify_stroke: setting.svg_simplify_stroke,
                bounds: Some(area),
            };
            let svg = kition_design_svg::try_export_svg(&doc.in_srgb(), &drawn, &opts).map_err(|e| match e {
                kition_design_svg::SvgError::Empty => nothing(),
                other => EngineError::Other(other.to_string()),
            })?;
            Ok((svg.into_bytes(), Vec::new()))
        }
        // Android colours are sRGB too.
        ExportFormat::VectorDrawable => {
            let compat = match setting.vd_compat {
                VdCompat::BestEffort => kition_design_vectordrawable::Compat::BestEffort,
                VdCompat::Exact => kition_design_vectordrawable::Compat::Exact,
                VdCompat::Ignore => kition_design_vectordrawable::Compat::Ignore,
            };
            let opts = kition_design_vectordrawable::VdOptions { bounds: Some(area), include_names: setting.svg_id_attribute, compat };
            let out = kition_design_vectordrawable::try_export(&doc.in_srgb(), &drawn, &opts).map_err(|e| match e {
                kition_design_vectordrawable::VdError::Empty => nothing(),
                other => EngineError::Other(other.to_string()),
            })?;
            Ok((out.xml.into_bytes(), out.warnings))
        }
        ExportFormat::Pdf => {
            let doc = &*doc.in_srgb();
            // Frames export one page each; anything else shares one page.
            let all_frames = own_area && ids.iter().all(|i| doc.node(*i).is_some_and(|n| n.kind.is_frame_like()));
            let groups: Vec<Vec<NodeId>> = if all_frames { ids.iter().map(|i| vec![*i]).collect() } else { vec![drawn] };
            Ok((pdf_bytes(doc, &groups, &setting.print, (!own_area).then_some(area))?, Vec::new()))
        }
        _ => {
            let pix = kition_design_render::render_area(doc, &drawn, area, fit_export_scale(area, scale))
                .ok_or_else(|| EngineError::Other("render failed (empty or too large)".into()))?;
            Ok((kition_design_render::encode_export(&pix, setting.format, doc.color_profile).map_err(EngineError::Other)?, Vec::new()))
        }
    }
}

/// The `vdCompat` `p` passes: bestEffort, exact or ignore (anything else is an error).
fn vd_compat_param(cmd: &str, p: &Value) -> Result<Option<VdCompat>> {
    match get(p, "vdCompat") {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v.as_str().and_then(VdCompat::from_key).map(Some).ok_or_else(|| bad(cmd, "vdCompat: bestEffort|exact|ignore")),
    }
}

fn close(s: &mut Session, p: &Value) -> Result<Value> {
    let i = if get(p, "index").is_none() { s.active } else { req_index("file.close", p, "index")? };
    let Some(ds) = s.docs.get(i) else { return Err(bad("file.close", "no such document")) };
    if ds.is_dirty() && !boolean(p, "force").unwrap_or(false) {
        s.request_ui("dialog.unsaved", json!({"index": i}));
        return Err(EngineError::Disabled("file.close".into(), "the document has unsaved changes (pass force: true to discard)".into()));
    }
    let uid = ds.uid;
    let active_uid = s.docs.get(s.active).map(|d| d.uid);
    s.remove_tabs(|d| d.uid == uid);
    // Removing a tab before the active one shifts its index; keep the same document selected.
    s.active = active_uid.and_then(|a| s.docs.iter().position(|d| d.uid == a)).unwrap_or(i.min(s.docs.len().saturating_sub(1)));
    ok()
}

#[cfg(test)]
mod tab_order_tests;

/// Ctrl+Tab / Ctrl+Shift+Tab: the next or previous open document, wrapping around.
fn cycle_doc(s: &mut Session, dir: i64) -> Result<Value> {
    let n = s.docs.len() as i64;
    if n == 0 {
        return Err(crate::EngineError::NoDocument);
    }
    s.active = (s.active as i64 + dir).rem_euclid(n) as usize;
    Ok(json!({"index": s.active}))
}

#[cfg(test)]
mod rest_import_tests;

#[cfg(test)]
mod staging_write_tests;

#[cfg(test)]
mod zip_cap_tests;
