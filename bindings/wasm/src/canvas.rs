//! Canvas marks the upstream UI computed from the session itself and drew with its toolkit, as
//! data in document pixels for the editor's overlay layer (queries, see [`crate::query`]):
//!
//! - `protoConnections`: the Prototype tab's connections ("noodles") from layers to the frames
//!   their interactions go to, the "+" handles on the selected layers, and what a point is over
//!   (upstream `canvas/proto_canvas.rs`);
//! - `commentPins`: comment pins and the clusters crowded ones gather into (upstream
//!   `canvas/comments.rs` `pins`, `spots`);
//! - `paintHandles`: a gradient's handles and stops, or a Crop-mode image's crop box, for the paint
//!   open in the colour picker (upstream `canvas/handles.rs`);
//! - `blurHandles`: a progressive blur's start and end points.
//!
//! Sizes marked "px" are screen pixels; the queries take the view's `zoom` where geometry depends
//! on it.

use kition_design_color::{ImageScaleMode, PaintKind};
use kition_design_doc::{Document, NodeId};
use kition_design_engine::cmd::comment::{self as cc, Filter, Query};
use kition_design_geom::{Point, Rect};
use serde_json::{Value, json};

use crate::{Area, DesignDocument};

/// The "+" connection handle's radius (px), as upstream.
pub const HANDLE_R: f64 = 7.0;
/// A comment pin's bubble side (px), and the distance (px) under which pins gather into a cluster.
pub const PIN: f64 = 28.0;
pub const CLUSTER_R: f64 = 24.0;
/// How near (px) a point must be to a connection to be over it.
pub const NOODLE_SLOP: f64 = 5.0;

fn zoom_of(params: &Value) -> f64 {
    params.get("zoom").and_then(Value::as_f64).filter(|z| z.is_finite() && *z > 0.0).unwrap_or(1.0)
}

fn point_of(params: &Value) -> Option<(f64, f64)> {
    let x = params.get("x").and_then(Value::as_f64).filter(|v| v.is_finite())?;
    let y = params.get("y").and_then(Value::as_f64).filter(|v| v.is_finite())?;
    Some((x, y))
}

fn pt(area: &Area, p: Point) -> Value {
    let (x, y) = area.to_doc(p);
    json!([x, y])
}

// ---------------------------------------------------------------------------------------------
// Prototype connections.

/// A connection's cubic (page space) from layer box `a` to frame box `b`: it leaves the side facing
/// the destination; the control points reach half the horizontal distance, at least 40 px.
fn curve_between(a: Rect, b: Rect, zoom: f64) -> [Point; 4] {
    let right = b.center().x >= a.center().x;
    let (p0, p3) = if right {
        (Point::new(a.x1, a.center().y), Point::new(b.x0, b.center().y))
    } else {
        (Point::new(a.x0, a.center().y), Point::new(b.x1, b.center().y))
    };
    let s = if right { 1.0 } else { -1.0 };
    let dx = ((p3.x - p0.x).abs() * 0.5).max(40.0 / zoom) * s;
    [p0, Point::new(p0.x + dx, p0.y), Point::new(p3.x - dx, p3.y), p3]
}

fn bezier(p: &[Point; 4], t: f64) -> Point {
    let u = 1.0 - t;
    let w = [u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t];
    Point::new(w[0] * p[0].x + w[1] * p[1].x + w[2] * p[2].x + w[3] * p[3].x, w[0] * p[0].y + w[1] * p[1].y + w[2] * p[2].y + w[3] * p[3].y)
}

fn segment_distance(a: Point, b: Point, q: Point) -> f64 {
    let (abx, aby) = (b.x - a.x, b.y - a.y);
    let len2 = abx * abx + aby * aby;
    let t = if len2 > 0.0 { (((q.x - a.x) * abx + (q.y - a.y) * aby) / len2).clamp(0.0, 1.0) } else { 0.0 };
    ((a.x + abx * t - q.x).powi(2) + (a.y + aby * t - q.y).powi(2)).sqrt()
}

/// The distance from `q` to the cubic (sampled, as upstream).
fn bezier_distance(p: &[Point; 4], q: Point) -> f64 {
    let mut best = f64::INFINITY;
    let mut prev = p[0];
    for i in 1..=32 {
        let cur = bezier(p, f64::from(i) / 32.0);
        best = best.min(segment_distance(prev, cur, q));
        prev = cur;
    }
    best
}

/// The arrowhead at the curve's end: tip and the two back corners (8 px back, 4.5 px wide).
fn arrow(p: &[Point; 4], zoom: f64) -> [Point; 3] {
    let tip = p[3];
    let back = bezier(p, 0.9);
    let (mut dx, mut dy) = (tip.x - back.x, tip.y - back.y);
    let len = (dx * dx + dy * dy).sqrt();
    if len.is_finite() && len > 1e-9 {
        (dx, dy) = (dx / len, dy / len);
    } else {
        (dx, dy) = (1.0, 0.0);
    }
    let (nx, ny) = (-dy, dx);
    let (b, w) = (8.0 / zoom, 4.5 / zoom);
    [tip, Point::new(tip.x - dx * b + nx * w, tip.y - dy * b + ny * w), Point::new(tip.x - dx * b - nx * w, tip.y - dy * b - ny * w)]
}

struct Noodle {
    source: NodeId,
    index: usize,
    dest: NodeId,
    pts: [Point; 4],
}

fn noodles(d: &Document, page: NodeId, zoom: f64) -> Vec<Noodle> {
    let mut out = Vec::new();
    for id in d.subtree(page) {
        let Some(n) = d.node(id) else { continue };
        if n.interactions.is_empty() {
            continue;
        }
        let Some(from) = d.world_bounds(id) else { continue };
        for (index, it) in n.interactions.iter().enumerate() {
            let Some(dest) = kition_design_doc::proto::walk(&it.actions).find_map(|a| a.dest()) else { continue };
            if d.page_of(dest) != Some(page) {
                continue;
            }
            let Some(to) = d.world_bounds(dest) else { continue };
            out.push(Noodle { source: id, index, dest, pts: curve_between(from, to, zoom) });
        }
        if out.len() >= 5000 {
            break;
        }
    }
    out
}

/// The top-level frame under a page point (top-most first), else the innermost section there.
fn frame_at(d: &Document, page: NodeId, p: Point) -> Option<NodeId> {
    let under = |i: &NodeId| d.world_bounds(*i).is_some_and(|b| b.contains(p));
    d.top_frames(page).into_iter().rev().find(under).or_else(|| d.sections_in(page).into_iter().rev().find(under))
}

/// `protoConnections { zoom?, x?, y? }`:
///
/// ```text
/// { connections: [{ source, index, dest,          // layer, its interaction's index, the frame
///                   points: [[x, y] × 4],          // the cubic (start, two controls, end)
///                   arrow: [[x, y] × 3],           // the arrowhead (tip first)
///                   selected,                      // its layer is selected: draw it brighter,
///                   destination: rect }],          //   and outline `destination`
///   handles: [{ id, x, y }],                       // "+" handles of the selected layers
///   handleRadius,                                  // HANDLE_R px; hit within it + 3 px
///   hit: null | { source, index },                 // the connection within 5 px of (x, y)
///   handleHit: null | id,                          // the handle (x, y) is on
///   frameAt: null | id }                           // the frame a connection dropped at (x, y)
///                                                  //   goes to (proto.interaction.add { ids, dest })
/// ```
///
/// Positions are document pixels; (`x`, `y`) is a document pixel to test. Upstream shows these only
/// in the Prototype tab, and the handles not while text is edited.
pub fn proto_connections(doc: &DesignDocument, params: &Value) -> Result<Value, String> {
    let ds = doc.ds()?;
    let d: &Document = &ds.doc;
    let area = doc.area();
    let zoom = zoom_of(params);
    let list = noodles(d, ds.page, zoom);
    let connections: Vec<Value> = list
        .iter()
        .map(|n| {
            json!({
                "source": n.source,
                "index": n.index,
                "dest": n.dest,
                "points": n.pts.iter().map(|p| pt(&area, *p)).collect::<Vec<_>>(),
                "arrow": arrow(&n.pts, zoom).iter().map(|p| pt(&area, *p)).collect::<Vec<_>>(),
                "selected": ds.selection.contains(&n.source),
                "destination": d.world_bounds(n.dest).map(|b| area.rect_json(b)),
            })
        })
        .collect();
    let handles: Vec<(NodeId, Point)> = if ds.text_edit.is_some() {
        Vec::new()
    } else {
        ds.selection
            .iter()
            .take(32)
            .filter(|i| d.node(**i).is_some_and(|n| !n.is_page()))
            .filter_map(|i| d.world_bounds(*i).map(|b| (*i, Point::new(b.x1 + (HANDLE_R + 4.0) / zoom, b.center().y))))
            .collect()
    };
    let at = point_of(params).map(|(x, y)| area.to_canvas(x, y));
    let hit = at.and_then(|q| {
        list.iter()
            .map(|n| (n, bezier_distance(&n.pts, q)))
            .filter(|(_, dist)| *dist * zoom < NOODLE_SLOP)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(n, _)| json!({ "source": n.source, "index": n.index }))
    });
    let handle_hit =
        at.and_then(|q| handles.iter().find(|(_, c)| ((c.x - q.x).powi(2) + (c.y - q.y).powi(2)).sqrt() * zoom <= HANDLE_R + 3.0).map(|(id, _)| *id));
    Ok(json!({
        "connections": connections,
        "handles": handles.iter().map(|(id, c)| { let (x, y) = area.to_doc(*c); json!({ "id": id, "x": x, "y": y }) }).collect::<Vec<_>>(),
        "handleRadius": HANDLE_R,
        "hit": hit,
        "handleHit": handle_hit,
        "frameAt": at.and_then(|q| frame_at(d, ds.page, q)),
    }))
}

// ---------------------------------------------------------------------------------------------
// Comment pins.

/// Groups points closer than `r` to a group's centre, in order (upstream `comments::cluster`).
fn cluster(points: &[(f64, f64)], r: f64) -> Vec<Vec<usize>> {
    let mut groups: Vec<(Vec<usize>, (f64, f64))> = Vec::new();
    for (i, p) in points.iter().enumerate() {
        let near = groups.iter_mut().find(|(g, sum)| {
            let k = g.len().max(1) as f64;
            ((sum.0 / k - p.0).powi(2) + (sum.1 / k - p.1).powi(2)).sqrt() < r
        });
        match near {
            Some((g, sum)) => {
                g.push(i);
                sum.0 += p.0;
                sum.1 += p.1;
            }
            None => groups.push((vec![i], *p)),
        }
    }
    groups.into_iter().map(|(g, _)| g).collect()
}

/// A stable colour per author (upstream `comments::author_color`, the live session's palette).
pub fn author_color(name: &str) -> &'static str {
    let h = name.bytes().fold(5381u32, |h, b| h.wrapping_mul(33) ^ u32::from(b));
    let people = kition_design_engine::live::PEOPLE;
    people.get(h as usize % people.len()).copied().unwrap_or("#0D99FF")
}

/// `commentPins { zoom?, filter?, open? }`: the current page's comment pins, oldest first and the
/// open thread (`open`, a thread id) last, crowded ones gathered into clusters (the open one
/// always stands alone):
///
/// ```text
/// { visible,                                        // the Comment tool or View > Comments is on
///   pins: [{ id, author, initials, color,           // the author's colour (resolved: draw grey)
///            resolved, unread, mine, open,
///            x, y }],                               // the bubble's bottom-left corner; the bubble
///                                                   //   is PIN px square above and right of it
///   clusters: [{ ids, x, y }],                      // a stack of bubbles with ids.length on it,
///                                                   //   (x, y) its bottom-left corner
///   pinSize }                                       // PIN px
/// ```
///
/// `filter`: `all` (default), `open`, `resolved`, `mine`, `unread` (the Comments panel's filter).
pub fn comment_pins(doc: &DesignDocument, params: &Value) -> Result<Value, String> {
    let session = doc.session();
    let ds = doc.ds()?;
    let d: &Document = &ds.doc;
    let area = doc.area();
    let zoom = zoom_of(params);
    let filter = match params.get("filter").and_then(Value::as_str) {
        None => Filter::All,
        Some(f) => Filter::parse(f).ok_or_else(|| format!("invalid-params: `filter` is all, open, resolved, mine or unread, not {f:?}"))?,
    };
    let open = params.get("open").and_then(Value::as_u64);
    let me = cc::user_name(session);
    let q = Query { page: Some(ds.page), filter, ..Query::default() };
    let mut list = cc::query(d, &q, &me);
    list.reverse();
    if let Some(c) = open.and_then(|o| d.comment(o)).filter(|c| c.page == ds.page && !list.iter().any(|x| x.id == c.id)) {
        list.push(c);
    }
    list.sort_by_key(|c| open == Some(c.id));
    let pin = |c: &kition_design_doc::Comment| {
        let (x, y) = c.position(d);
        let (x, y) = area.to_doc(Point::new(x, y));
        json!({
            "id": c.id,
            "author": c.first.author,
            "initials": kition_design_engine::live::initials(&c.first.author),
            "color": author_color(&c.first.author),
            "resolved": c.resolved,
            "unread": c.is_unread(&me),
            "mine": c.first.author == me,
            "open": open == Some(c.id),
            "x": x,
            "y": y,
        })
    };
    let (solo, rest): (Vec<_>, Vec<_>) = list.into_iter().partition(|c| open == Some(c.id));
    // Clusters gather by the bubbles' centres on screen.
    let centres: Vec<(f64, f64)> = rest
        .iter()
        .map(|c| {
            let (x, y) = c.position(d);
            (x * zoom + PIN / 2.0, y * zoom - PIN / 2.0)
        })
        .collect();
    let (mut pins, mut clusters) = (Vec::new(), Vec::new());
    for group in cluster(&centres, CLUSTER_R) {
        let members: Vec<&kition_design_doc::Comment> = group.iter().filter_map(|i| rest.get(*i).copied()).collect();
        match members.as_slice() {
            [one] => pins.push(pin(one)),
            many => {
                let k = many.len().max(1) as f64;
                let (sx, sy) = many.iter().map(|c| c.position(d)).fold((0.0, 0.0), |(a, b), (x, y)| (a + x, b + y));
                let (x, y) = area.to_doc(Point::new(sx / k, sy / k));
                clusters.push(json!({ "ids": many.iter().map(|c| c.id).collect::<Vec<_>>(), "x": x, "y": y }));
            }
        }
    }
    pins.extend(solo.into_iter().map(pin));
    let visible = session.tool.tool == kition_design_engine::ToolId::Comment || ds.view.comments || open.is_some();
    Ok(json!({ "visible": visible, "pins": pins, "clusters": clusters, "pinSize": PIN }))
}

// ---------------------------------------------------------------------------------------------
// Paint and blur handles.

/// The affine that takes layer `id`'s unit box (0..1 on both axes) to document pixels:
/// `[a, b, c, d, e, f]` with x' = a·u + c·v + e, y' = b·u + d·v + f.
fn unit_affine(d: &Document, id: NodeId, area: &Area) -> Option<[f64; 6]> {
    let n = d.node(id)?;
    let w = d.world_transform(id);
    let o = w * Point::new(0.0, 0.0);
    let ux = w * Point::new(n.width, 0.0);
    let vy = w * Point::new(0.0, n.height);
    let (ox, oy) = area.to_doc(o);
    let m = [ux.x - o.x, ux.y - o.y, vy.x - o.x, vy.y - o.y, ox, oy];
    m.iter().all(|v| v.is_finite()).then_some(m)
}

fn apply(m: &[f64; 6], (u, v): (f64, f64)) -> Value {
    json!([m[0] * u + m[2] * v + m[4], m[1] * u + m[3] * v + m[5]])
}

/// The `index` parameter (default 0); one past the platform's `usize` (2^32 on wasm32) is
/// `usize::MAX`, which names nothing, never a wrapped-around index.
fn index_param(params: &Value) -> usize {
    params.get("index").and_then(Value::as_u64).map_or(0, |i| usize::try_from(i).unwrap_or(usize::MAX))
}

fn layer_param(params: &Value) -> Result<NodeId, String> {
    kition_design_engine::params::id(params, "id").ok_or_else(|| "invalid-params: `id` must be a layer id".to_owned())
}

/// `paintHandles { id, index, fills? }`: the handles of paint `index` of layer `id`'s fills (or its
/// strokes with `fills: false`), for the paint open in the colour picker:
///
/// ```text
/// { kind: "gradient", gradientType: "linear"|"radial"|"angular"|"diamond",
///   from: [x, y], to: [x, y], width: [x, y],     // the handles (no width handle on linear)
///   stops: [{ position, color: "#rrggbbaa", point: [x, y] }],   // squares along from → to
///   unit: [a, b, c, d, e, f] }                   // unit box → document pixels (invert it to
///                                                //   write a dragged handle back: fill.set)
/// { kind: "crop", corners: [[x, y] × 4],         // the image's box: tl, tr, br, bl
///   rect: [u0, v0, u1, v1], unit }               // the same in the layer's unit box
/// { kind: "none" }                               // a solid, or an image not in Crop mode
/// ```
pub fn paint_handles(doc: &DesignDocument, params: &Value) -> Result<Value, String> {
    let id = layer_param(params)?;
    let index = index_param(params);
    let fills = params.get("fills").and_then(Value::as_bool).unwrap_or(true);
    let ds = doc.ds()?;
    let d: &Document = &ds.doc;
    let area = doc.area();
    let n = d.node(id).ok_or_else(|| format!("not-found: no layer {id}"))?;
    let list = if fills { &n.fills } else { &n.strokes };
    let paint = list.get(index).ok_or_else(|| format!("not-found: layer {id} has no {} {index}", if fills { "fill" } else { "stroke" }))?;
    let m = unit_affine(d, id, &area).ok_or_else(|| format!("layer {id} has no finite box"))?;
    Ok(match &paint.kind {
        PaintKind::Gradient(g) => {
            let along = |t: f64| (g.from.0 + (g.to.0 - g.from.0) * t, g.from.1 + (g.to.1 - g.from.1) * t);
            json!({
                "kind": "gradient",
                "gradientType": serde_json::to_value(g.kind).unwrap_or(Value::Null),
                "from": apply(&m, g.from),
                "to": apply(&m, g.to),
                "width": apply(&m, g.width),
                "stops": g.stops.iter().map(|s| json!({
                    "position": s.position,
                    "color": format!("#{}", s.color.to_hex()),
                    "point": apply(&m, along(f64::from(s.position))),
                })).collect::<Vec<_>>(),
                "unit": m,
            })
        }
        PaintKind::Image(img) if img.scale_mode == ImageScaleMode::Crop => {
            let data = d.images.get(&img.image).ok_or_else(|| format!("not-found: the image of layer {id} is not in the file"))?;
            let crop = img.effective_crop(f64::from(data.width), f64::from(data.height), n.width, n.height);
            let (u0, v0, u1, v1) = kition_design_color::ImagePaint::crop_rect(crop);
            json!({
                "kind": "crop",
                "corners": [apply(&m, (u0, v0)), apply(&m, (u1, v0)), apply(&m, (u1, v1)), apply(&m, (u0, v1))],
                "rect": [u0, v0, u1, v1],
                "unit": m,
            })
        }
        _ => json!({ "kind": "none" }),
    })
}

/// `blurHandles { id, index }`: effect `index` of layer `id` when it is a progressive blur:
/// `{ start: [x, y], end: [x, y], unit }` (the start is drawn hollow, the end filled), else null.
pub fn blur_handles(doc: &DesignDocument, params: &Value) -> Result<Value, String> {
    let id = layer_param(params)?;
    let index = index_param(params);
    let ds = doc.ds()?;
    let d: &Document = &ds.doc;
    let area = doc.area();
    let n = d.node(id).ok_or_else(|| format!("not-found: no layer {id}"))?;
    let Some(b) = n.effects.get(index).and_then(|e| e.blur()).filter(|b| b.is_progressive()) else { return Ok(Value::Null) };
    let b = b.sanitized();
    let m = unit_affine(d, id, &area).ok_or_else(|| format!("layer {id} has no finite box"))?;
    Ok(json!({ "start": apply(&m, b.start), "end": apply(&m, b.end), "unit": m }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_index_never_wraps() {
        assert_eq!(index_param(&json!({})), 0);
        assert_eq!(index_param(&json!({ "index": 3 })), 3);
        assert_ne!(index_param(&json!({ "index": 1u64 << 32 })), 0, "2^32 names no paint (it wrapped to 0 on wasm32)");
        assert_ne!(index_param(&json!({ "index": u64::MAX })), 0);
    }

    #[test]
    fn curves_leave_the_facing_side() {
        let a = Rect::new(0.0, 0.0, 50.0, 20.0);
        let b = Rect::new(200.0, 100.0, 300.0, 300.0);
        let c = curve_between(a, b, 1.0);
        assert_eq!((c[0].x, c[0].y), (50.0, 10.0));
        assert_eq!((c[3].x, c[3].y), (200.0, 200.0));
        let back = curve_between(b, a, 1.0);
        assert_eq!((back[0].x, back[0].y), (200.0, 200.0));
        assert_eq!((back[3].x, back[3].y), (50.0, 10.0));
        assert!(bezier_distance(&c, bezier(&c, 0.5)) < 0.5);
        assert!(bezier_distance(&c, Point::new(0.0, 300.0)) > 50.0);
    }

    #[test]
    fn crowded_points_cluster() {
        let groups = cluster(&[(0.0, 0.0), (10.0, 0.0), (100.0, 0.0)], 24.0);
        assert_eq!(groups, vec![vec![0, 1], vec![2]]);
    }
}
