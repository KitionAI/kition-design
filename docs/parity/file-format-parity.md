# File format parity

> **Last reviewed:** 2026-11-17 · **Last updated:** 2026-11-17 · **Target:** Figma Design (desktop app 126.10.7)

Every format Figma reads or writes, with Kition Design's support. Sources: `Info.plist` of
`/Applications/Figma.app` (document types only; nothing inside the bundle's code was read), Figma's
help center (image and video types, export formats), and Kition Design's source.

**Summary (estimated):** ~60% ready. The core interchange formats (SVG, PNG, JPG, PDF) are solid
and tested; Figma's own document, `.fig`, opens (best effort, not yet checked against real-world
files) and can be written, experimentally: written files round trip through Kition Design's own
importer and were checked by opening them in other software (layers, fills, gradients, strokes,
shadows, vectors, components and text draw as in Kition Design); effects such as glass and noise, images
and prototype interactions are still unchecked there.

## Documents

| Format | Figma | Kition Design read | Kition Design write | Fidelity / testing | Hours |
|---|---|---|---|---|---|
| `.fig` (Figma Design, `com.figma.document`, `application/x-figma`) | native (cloud; "Save local copy") | **yes, best effort** (`file.import`, drop, and the host opening it with the kit) | **experimental** ([below](#export-to-fig-experimental)) | `.fig` files open (best effort): the zip, its deflate/zstd chunks and the kiwi schema are decoded generically and mapped by field name onto the REST importer, so pages, the layer tree, geometry, paints (styles resolved), strokes, effects, text with styled runs, vectors as editable vector networks (points, curved segments, filled areas with their winding, per-point corner radius and handle mirroring, end caps; outlines when the network is missing or damaged), auto layout (grid included; a flow that records no primary-axis sizing hugs on that axis, an instance that records no auto layout has its main's), constraints, ellipse arcs, boolean operations, components, sets (with their descriptions), instances (expanded from their main with overrides, derived layout and property values; the size and resizing their layers override stay overrides), styles, variables with modes and bindings, prototyping (set-variable, set-variable-mode and conditional actions included) and image fills from the archive come across; sticky notes, shapes with text, connectors, tables, code blocks, slides and similar layers come in as the closest frames, text layers and vectors, with a warning; anything else is a warning. Also: an instance's own identity and the cached library components a file carries, overrides applied by key and order, local text values and instance overrides over a shared text style, grid track sizes (fixed, content-sized, fill), slot content, labels of connectors saved with their drawn path (a static snapshot; the ends aren't live), skip ink, paragraph options and hanging lists, list start numbers, vertical trim, per-point stroke joins, editable text on a path, image adjustments and images carried in the message. Budgets come from the file's own size (one value per compressed byte, 20M–128M, lower ceilings on 32-bit targets; the layer cap follows), so a 15.4 MB synthetic file with 300,000 layers opens whole; the file's bytes stay out of the command journal; an import report lists what was left out or changed once the file opens. Not read: elbowed or curved connectors without a saved path (drawn straight) and labels of connectors without a saved snapshot, table cells beyond their own layers, filled areas with their own fills, variables bound to fields the editor can't bind, video fills. Tested on fixtures generated in code (`crates/formats/format/src/fig/tests.rs`, `tail_tests.rs` and the per-feature `*_tests.rs` beside them, fuzzed) and by reading back what the export writes; no real-world files yet | read tail 5–10, write: checks against real files 5–15 |
| `.sketch` (Sketch 48 and newer) | import (file browser ▸ Import or drop, desktop app too; converted to a Figma file, symbols as components; per the help center) | **yes, best effort** (`file.import`, drop, and the host opening it with the kit) | no (out of scope) | zip + JSON read from the public format description; Sketch 43–47 (text as binary property lists) and older (SQLite) files are refused with a message to resave them in a newer Sketch. Pages, artboards as frames with their background, groups; Sketch 2025 and newer: frames (clipping or not) with their style, graphics, stacks as auto layout (direction, gap, alignment, padding, wrap, hug/fill sizing, items ignoring the stack), corners in the style (rounded, smooth with its smoothing; angled, inside square and inside arc as paths), pins and sizing, per-side borders, lists of blurs (progressive background blurs, glass), inner shadows in the shadow list, tints (a graphic's, group's or instance's fill colouring its layers, and colour overrides), text colour and visibility overrides, text sized by its sizing; rectangles with corner radii, ovals, stars and polygons while their points are the generated ones, paths as editable vector networks, shape groups as their shape, as one compound path (children without an operation, by the group's winding rule) or as boolean operations (mixed operations nested from the left); open paths filled as if closed, edited ovals and rectangles as paths, bitmaps (with their colour adjustments as image filters), slices, hotspots; transforms (rotation, flips after it), resizing constraints; fills (solid, gradients with Sketch's geometry, image patterns), borders (the first enabled border's width and position), shadows, inner shadows, layer and background blurs, opacity and blend modes; clipping masks (an outline mask that shows its own style draws a copy under itself, as Sketch does; alpha masks only mask; broken mask chains wrapped), progressive opacity (Fade) as an alpha mask of its gradient; text with styled runs (UTF-16 ranges), paragraph and vertical alignment, lines placed by Sketch's line spacing behaviour, box sizing, fonts found by their PostScript names among the installed ones (else the family read from the name), variable font axes, Sketch's natural line (ascent and descent rounded, no line gap, a fifth more for classic Mac fonts) for text without a fixed line height; symbols as components (library copies on a page of their own) and instances carrying their masters' layers with overrides (text, nested swaps and hiding, images, layer and text styles, nested paths), laid out for their size the way Sketch does it (proportional sharing per pins from format 110, scaling before) and still live; master backgrounds in instances only where Sketch shows them; Apple's system fonts found under the names designs use, with the platform's tracking kept as letter spacing; shared layer styles as paint (fills, border colours) and effect styles, shared text styles as text styles; Color Variables (also library ones) as colour variables with bindings; prototype links, flow starts, export options, layout and square grids (hidden, as Sketch shows them only in its layout view). Instances of a stack laid out at their size and texts an override changed measured again on open. Warnings: noise fills, motion and zoom blurs (drawn as plain blurs), fonts embedded with their data (not used), Smart Layout (kept as laid out), locked overrides, scaled instances. Tested on fixtures built in code (`crates/formats/format/src/sketch/*_tests.rs`, fuzzed) and locally (`KITION_DESIGN_SKETCH_SAMPLES`) on one real Sketch 50 file and the public Apache-2.0 IBM Carbon Design Kit saved by Sketch 68 and 83 (70,000 layers open in ~3 s, debug build; its edition saved by Sketch 47 is refused, as it should be); compared artboard by artboard with Sketch 2026's own renders (`KITION_DESIGN_SKETCHTOOL`, black-box): the Sketch 50 file's 73 artboards differ in at most 2.3% of their pixels (0.8% on average) (text metrics, fonts Kition Design doesn't find, antialiasing), Carbon 11's 69 1.1% on average and at most 4.4% (text in IBM Plex, which the file embeds and Kition Design doesn't use yet), a Sketch 64 file's 65 artboards 2.1% on average, a Sketch 2026 showcase file's 16 frames 0.9% on average (at most 1.9%) | 5–10 (Smart Layout as auto layout, motion and zoom blurs, several borders of different widths; fonts embedded in the file once `.kdesign` documents can carry fonts: until then they would only last the session) |
| `.jam` (whiteboards), `.deck` (Slides) | native to other Figma products | no | no | `.jam`: not read or written (boards are Kition Design files, see [boards-parity](boards-parity.md)); `.deck` out of scope | — |
| REST API file JSON (`GET /v1/files/:key`) + local variables JSON + `images/` folder | the API's output | **yes** (`file.import`) | no | pages, layer tree, geometry, paints, strokes, effects, text runs, vectors, auto layout (sizing modes left out read as hug, as documented; grid track sizes from `gridColumnsSizing` / `gridRowsSizing`: px, fr, auto, `minmax(0, 1fr)`, `repeat()`), constraints, grids, components, sets, instances with overrides (sizes and resizing included), component properties, slots (`SLOT` layers bound to their slot property; an instance's slot keeps its content as the instance's own unless its property value is the main's default content; slot limits and preferred components are warnings), styles, prototyping (incl. conditionals and variable modes), variables with modes, aliases, scopes and bindings, image adjustments (the same tint sign as `.fig`); the import report after opening; warnings for what's skipped (video triggers and actions, modifier keys). Unit tests in `crates/formats/format/src/rest/*_tests.rs` | — |
| `.kdesign` (ours: zip of `document.json` + images, versions inside) | — | yes | yes | streamed JSON, 50k layers save ~0.09 s / load ~0.16 s (measured natively, `large_tests.rs`); in the kit the host reads and writes the file, and a file another program saved is read again with the difference recorded as one transaction ([`kdm.md`](../features/kdm.md), section 3); `kind: "board"` marks a board (left out for design files, so a file without it is a design file); board objects keep their `board` metadata on the layer; Dev Mode measurements are kept on the layer they start from (`measurements: [{to} | {side}]`, left out when there are none) | — |
| Local library files | team libraries in the cloud | yes (enable a `.kdesign` as a library) | yes (publish) | per-asset versions, updates with overrides kept | see [collaboration-parity](collaboration-parity.md) |

## Import (place / paste / drop)

| Format | Figma | Kition Design | Notes | Hours |
|---|---|---|---|---|
| SVG | editable vectors | **yes**, editable | corpus of 21 synthetic files with hand-computed expected pixels, structure and round-trip checks; gradients, patterns as image fills, clip paths, masks, shadow/blur filters, blend modes, text as text layers; other filters dropped with a warning; text on one path as text on a path (on several paths as outlines); fuzzed | 2–4 (filters) |
| PNG, JPG | yes | yes | embedded ICC profiles (PNG `iCCP`, JPG APP2) and PNG `sRGB` / `gAMA` / `cHRM` chunks convert the image into the file's colour space; no profile (or CMYK, damaged): taken as sRGB | — |
| GIF | yes (animated) | **yes**, animated | the file is kept as is; the canvas and PNG, JPG and PDF exports show the first frame (SVG exports embed the file, so viewers that animate GIFs animate it), the presenter and the preview play it (frames decoded on first play: at most 500 frames and 128 MB, past that the rest don't play; always loops) | — |
| WebP | yes | yes | embedded ICC profile honoured | — |
| HEIC | yes | **no** | shortcut: no Rust decoder; the usable ones (libheif, libde265) are C libraries, and HEVC carries patents. Revisit when a Rust decoder exists | 2–3 |
| TIFF | Safari only | **yes** | open, import, place, drop (the `image` crate's TIFF decoder; its embedded ICC profile is honoured); SVG exports embed it (and BMP) as PNG, which SVG viewers show | — |
| BMP | — | yes | | — |
| MP4, MOV, WebM (video fills, paid plans) | yes | **no** | shortcut: decoding H.264/HEVC/VP9 needs non-Rust libraries (FFmpeg or each platform's media framework); presenter playback would reuse the animated GIF clock | 4–8 |
| Images over 4096 px | downscaled to 4096 | kept | | — |
| Pasted content from other apps | images, SVG, text, Figma clipboard | images, SVG, text (newest clipboard content wins), **layers copied from another design app, best effort** | Cmd/Ctrl+V reads an image copied in another app. Copied design layers: the `.fig` canvas in the clipboard's HTML is read like a `.fig` file (with its warnings) and pasted as Paste pastes, in place or into the selected frame (`edit.pasteDesignLayers {html \| data}`, refused over 256 MB before decoding); text copied later pastes as text again. Tested with clipboard HTML made from our own `.fig` export (`design_paste_tests`), never with a real clipboard from other software | with `.fig` |

## Export

SVG and PDF refuse an export whose visible layer tree exceeds 256 descendant levels from any
exported root (the root is depth zero). The error identifies the root and offending layer and
suggests exporting an inner layer or reducing nesting. Hidden subtrees do not count; masks and
subtrees rendered into embedded images do. Refusal returns no partial SVG/PDF and leaves the
existing destination file intact. Selecting an inner root starts a new depth count. This is an
export limit, not a native document nesting limit.

| Format | Figma | Kition Design | Notes | Hours |
|---|---|---|---|---|
| PNG | yes, scales/suffixes | yes | export settings per layer, presets (`2x`, `512w`, `512h`), preview, "Export N layers"; ignore overlapping layers (on by default; off: every layer showing in the area) and include bounding box (the layer's own box; the Export section offers it for text layers, and `file.export` and `export.update` take it for any layer); slashes in layer names export into folders. The exported area is what the layer paints: shadows towards their offset, outside and centre strokes, unclipped children, a mask group's mask, nothing for hidden layers; slices export everything under them, except that with ignore overlapping layers on a slice inside a frame or group exports only that frame or group; sides round up to whole pixels | — |
| JPG | yes | yes | white where transparent; fixed quality (90) | — |
| SVG | yes (outline text option, ids) | yes | alpha/luminance masks kept; noise, texture, progressive and background blur, glass embedded as images; outline text (off: `<text>` with one `<tspan>` per line, one style per layer), unique id attributes from layer names, explicit depth-limit refusal, nested frames clipped by their parent's clip, simplified strokes (stroke attributes for centred strokes and for inside and outside strokes on boxes and ellipses) | — |
| PDF | yes | yes | one page per frame, real selectable text in subset fonts, links, vectors and gradients as vectors, effects as images; bleed, crop/bleed marks, page boxes, naive CMYK; checked by rendering the PDF back against PNG exports; explicit depth-limit refusal | CMYK output intent 2–4 |
| WebP | — | yes (ours) | | — |
| Android Vector Drawable (XML) | — | yes (ours) | an `.xml` drawable resource (`file.export {format: "xml"}`, export settings, File ▸ Export…), pretty-printed, sized in dp with the viewport in layer units, for AndroidX (VectorDrawableCompat: gradients and even-odd need API 24 without it). Shapes, booleans and text as path data with transforms baked in; solid fills and linear, radial and sweep gradients (`aapt:attr`, exact for any handles: radial and sweep in a `<group>` that maps the unit circle); centred strokes as stroke attributes where Android draws them the same (uniform scale, one cap, no dashes or variable width), other strokes as filled outlines; clipping frames and masks as `<group>` with `<clip-path>`; `android:name` from layer names with the id attribute option. What the format can't hold goes by the compatibility mode (`vdCompat`): best effort (default) applies layer opacity to each shape, rewrites even-odd fills and clip outlines as non-zero, draws diamond gradients as radial and alpha masks as outline clips, each with a warning; exact leaves out what it can't draw exactly, with a warning; ignore writes what maps without warnings: layer opacity over overlapping shapes is dropped (drawn opaque), even-odd fills keep `fillType="evenOdd"`, even-odd clip outlines clip as non-zero, diamond gradients are left out and alpha masks clip to their outline. Effects, image and pattern fills and blend modes are always left out (warned except in ignore); an export that draws nothing writes one empty path (Android refuses a drawable without a path), with a warning. Checked by parsing the XML, against Kition Design's renderer (gradients pixel by pixel), with `aapt2 compile` and `link`, and by rendering samples (linear, skewed radial and sweep gradients, a sweep with mirrored handles, even-odd booleans, centred and inside strokes, rotated text, a clipping frame, an alpha mask, a translucent group, each compatibility mode) with Android 13's (API 33) VectorDrawable on an emulator, next to Kition Design's PNG export: they match, including where the sweep starts and which way it turns, except what the compatibility mode and its warnings say (best effort: overlapping shapes in a translucent group look denser; exact and ignore leave out or draw opaque what they list). Not checked on API 21–23 (VectorDrawableCompat) | — |
| Copy as CSS / SVG / PNG / text / link | yes | yes | Copy as PNG: the whole selection at 2x | — |
| Dev Mode code (CSS, iOS SwiftUI, Android Compose) | yes | yes, plus HTML, React, Leptos and Vue | see [dev-mode-parity](dev-mode-parity.md) | — |
| `.fig` ("Save local copy") | yes | **experimental** | the whole file; see [Export to `.fig`](#export-to-fig-experimental). Round trips through our own importer; written files checked by opening them in other software: layers, fills, gradients, strokes, shadows, vectors, components and text draw as in Kition Design; resized and scaled instances keep their size and lay their layers out for it, variables with their modes and colour bindings resolve there (a frame set to another mode shows that mode's colours, also inside instances), colour, text, effect and grid styles are listed, and variable-width strokes that are the same on both sides of the path draw with their width | 5–15 (non-colour bindings as bindings, checks of effects, images and prototypes, and against real files) |

### Export to `.fig` (experimental)

`file.export {format: "fig"}` (or a `path` ending in `.fig`) and File ▸ Export… ▸ .fig (experimental)
write the whole document. In the engine, `file.save` / `file.saveAs` with a `.fig` path write it as a
copy too (the document keeps its `.kdesign` path, unsaved changes and version preview, and a file
saved under a `.fig` name is never overwritten by Save, `save_format_tests`); the kit
leaves saving to the host. The writer (`crates/formats/format/src/fig/export.rs`) works from the same public format
notes as the import and inverts its mapping: the zip holds `canvas.fig` (the fig-kiwi header, then a
raw-deflate binary schema and a raw-deflate message), `meta.json`, `thumbnail.png` and
`images/<hash>`; `meta.json` names the file and gives the page colour and the thumbnail size. The
schema written into the file describes only the fields written, under the names, kinds (struct or
message), field ids and enum values the format gives them. The document and each page say they are
visible and opaque (a page also its background opacity): a reader takes what is left out as off, and
draws nothing below a hidden page. The message
lists one node change per layer, parents first, each with its GUID, its parent and a position
string that orders it among its siblings; vector networks and outlines go in blobs, written once
each.

- **Written:** pages (name, background); frames, groups, sections, rectangles (as rounded
  rectangles, with their four radii), ellipses (arcs), polygons, stars, lines, vectors (editable
  network blobs with per-point corner radius, handle mirroring, caps and joins, plus the outline of
  their filled areas), text, boolean groups, slices; the fill outline of frames and shapes and the
  stroke outline of plain strokes (a ring around boxes and whole ellipses, the widened path of other
  centred strokes);
  solid, gradient (handles as the paint transform) and image paints (the image bytes, named by their SHA-1, scale mode,
  rotation, tile scale, crop); strokes (weight, align, join, caps, dashes, miter limit, per-side
  weights); shadows, layer and background blurs (progressive too), noise and texture; corners and
  smoothing, constraints, clip content, masks with their type; auto layout (direction, wrap, gaps, padding,
  alignment, hug, strokes in layout, stacking order, grids with their tracks, their sizes (fixed, hug, fill) and cells) and how
  layers sit in it (fill, absolute, min/max sizes, an axis without a maximum as infinite); layout grids; export settings; text style
  (font, with styles under the names font families give their faces: "SemiBold", "ExtraBold"…, and
  "Semi Bold" in Inter; size; line height, a share of
  the font size as a raw multiplier; letter spacing, case, decoration, links), alignment, auto resize,
  truncation, paragraph spacing and indent, list types and styled runs with their fills (indexed
  by UTF-16 unit); components (description, boolean, text and instance swap properties and the
  layers bound to them, with their values also as variable data), component sets (the set holds
  the definitions, variant properties included with their values in order; each variant has its
  own copy of each definition and its variant values, and its layers and instances name that
  copy), instances (their main, property values, what the instance
  itself changes from its main, such as its size, name and paints, as an override addressed by the
  main's id, the fields its layers change, by id path, through nested instances and swaps, and what
  follows from those as derived data: each layer's size, place and outline and the colours its
  variables give in the instance's mode; an instance scaled as a whole says its scale factor);
  variable collections with their modes and variables (colour, number, string and boolean values
  per mode, aliases, descriptions, scopes) as nodes on a hidden page, the variables bound to
  paints, gradient stops, effect and layout grid fields and to layer and text fields (size, gaps,
  padding, radii, stroke weights, opacity, visibility, characters, font and paragraph settings), and
  the modes frames and pages set; shared paint, text, effect and grid styles as nodes on that page
  with the links from the layers that use them (unused styles too); variable-width strokes (each
  point's reach on either side of the path as a share of the stroke weight, half of it for the
  full weight; a stroke wider on one side, on a shape that shows nothing else, as a vector filled
  with the area it paints); prototype
  interactions (clicks, hovers, presses, drags, mouse triggers, delays, keys with modifiers and
  controller buttons; navigate, overlays with their position, outside-click setting and
  background, swap, change to, scroll to, back, close, open link; transitions and easing, custom
  curves included), scrolling and fixed layers, flow starting points.
- **Left out, with a warning each:** variable bindings the format has no field for (component
  properties, arc fields, font weight: written as the values they give), text styles on part of a
  text (written as values), variables' publishing, code syntax and inheritance settings, extended
  collections (written as collections of their own), comments, Dev Mode annotations, statuses,
  resources, measurements and code links, plugin data, ruler
  guides, prototype device settings, slot properties and slot content, brush
  strokes (written plain), strokes wider on one side of their path (written as filled outlines, or
  with the width on both sides when the shape has a fill too), pattern fills, WebP export settings (written as PNG),
  image adjustments, glass settings other than frost, springs other than Gentle (written as ease
  out), set-variable, set-variable-mode and conditional actions, scroll-to offsets, text decoration
  looks, variable font axes, text on a path, nested list levels, list spacing, hanging punctuation,
  leading trim, OpenType features, per-layer operations inside boolean groups, different stroke ends
  on lines. Not written at all: stroke outlines of dashed and per-side strokes and of inside or
  outside strokes on polygons, stars and vectors, glyph outlines of text (a reader lays text out
  with its own fonts), and saved versions (the host's, never a file's).
- **Checked by opening written files in other software:** the example documents and small test
  files open there, and layers, fills, gradients, strokes, shadows, vectors, components and text draw
  as in Kition Design. A file saved again there and read back keeps the layer tree and properties.
  Font styles are written under their standard names, a line height that is a share of the font
  size is written as a raw multiplier (percent units count from the font's own line height), and
  the import reads per-side stroke weights that leave the sides without a border out. Resized and
  scaled instances keep their size and lay their layers out for it, variables with their modes and
  colour bindings resolve there (a frame set to another mode shows that mode's colours, also inside
  instances), colour, text, effect and grid styles are listed, variable-width strokes that are the
  same on both sides of the path draw with their width, and a nested instance swapped for another
  variant shows that variant at its size.
  Not working or unchecked there: number, string and boolean bindings and bindings on effects,
  layout guides and gradient stops come across as the values they give but do not show as bound
  there (their entries name the layer field they set, not yet checked there), and strokes wider on
  one side of the path are written as filled outlines, not editable strokes there (the stroke
  itself is kept as plugin data and our own import puts it back; not yet checked there); effects
  such as glass and noise, images and prototype interactions are unchecked. Text boxes sized to
  their content can come out a pixel wider or narrower there.
  Synthetic files imported into the incumbent (Drafts ▸ Import) come in with their images (named
  by their SHA-1), component sets list every property (the set holds the definitions and each
  variant its own copies, the layout of a file the incumbent saves itself), the variants' layers
  are bound to them and instances keep their values; frames with only a maximum width keep their
  size, and text in a family that spells compound weights without a space (Libre Franklin Semi
  Bold) is found and drawn. Variables with their modes, shared styles and their links, colour
  bindings and the modes frames set come across.
- **Round trips through our own importer:** 60 generated documents (every layer
  kind above, components, nested instances with overrides, a grid inside a component,
  interactions) are exported, read back and compared layer by layer: structure, geometry within
  32-bit float tolerance, paints, text and runs, auto layout, components and prototypes. The written
  file decodes with the generic kiwi decoder and lists every layer after its parent; the encoder
  agrees with the decoder on random messages and refuses values the schema can't hold; junk
  documents never panic (`crates/formats/format/src/fig/export_tests.rs`,
  `crates/engine/engine/src/fig_export_tests.rs`). What other readers need (visible document and pages,
  outlines, text settings, schema numbering, font style names, line height units, mask types) has
  its own tests (`crates/formats/format/src/fig/interop_tests.rs`). Variables, modes, every kind of binding,
  styles, resized, scaled and swapped instances and variable-width strokes are checked field by
  field as written and compared after reading back, files with scrambled values and junk kept strokes never panic
  (`crates/formats/format/src/fig/assets_tests.rs`), and every example document keeps its variables, styles
  and links through `file.export` and `file.open` (`crates/engine/engine/src/fig_export_assets_tests.rs`).

## Fonts

| | Figma | Kition Design |
|---|---|---|
| Local fonts | yes (font helper / desktop app) | no: the kit lays out text with the faces built into it and the font files added with `addFont`, never the machine's fonts (`bindings/wasm/src/lib.rs`) |
| Bundled/shared fonts | Google Fonts in the cloud | optional `KITION_FONTS_DIR` build input (KitionAI/kition-fonts) |
| Variable fonts | yes | yes (axes, named instances, per-range axis values) |
| Missing-font replacement on open | yes | listed, not replaced: the editor's Missing fonts dialog lists them (`query('missingFonts')`, `web/src/dialogs/FileDialogs.tsx`), and their text draws in a fallback font |

## Revision history

| Date | Change | Summary |
|---|---|---|
| 2026-11-17 | major | First public version |
