# Design on the Kit Data Model: schema `kit.design` 1, the `.kdesign` replica, engine contract 2

> Status: implemented; the "Decisions taken" list (section 9) names each point and the precedent
> it follows. Normative text:
> kition-sdk `docs/platform/sets/model.md` (KitModel 1.1, rules M1 to M24), `kits/engine.md`
> (engine contract 2, E11 to E14a, P1 to P5), `reference/object-model.md` (KitObjectModel),
> `kits/levels.md` (L6) and Kition `docs/kits.md` section 5 item 2b and section 7
> ("Collaboration budgets"). Precedents, closest first: kition-vector (an upstream app's document
> bridged to the KDM: an engine document view plus diff writes), kition-layout (the replica in
> its own file, DE notes), kition-board and kition-photo.

## 1. Overview

The engine keeps its `Document` (`crates/engine/doc`) and every command as they are. The crate
`crates/kdm/kdm-model` (`kition-design-kdm`) bridges that document to a `kit.design` KDM
replica: it writes a whole document into a new replica, writes only what a command changed
(a diff of two `Document`s), reads a replica back into a `Document`, and repairs a merged state.
The engine's session (`crates/engine/engine/src/kdm.rs`) holds one replica per open document
once the binding turns the bridge on (`Session::enable_kdm`; the engine's own tests run
without it), writes each new history step as one transaction, and steps the replica on undo and
redo. The bindings answer engine contract 2 (`contractVersion` 2, the session members,
`openWithReplica`, the object model) and `kition-design` serves `model.*` headless.

## 2. Schema `kit.design` 1

Schema version 1 is the first release's (task schema-freeze, 2026-11-22): every collection a
person edits piece by piece is a container from the start (a text, a list of objects, a map),
since an object type can never gain a container field later. A replica of an earlier schema
(the development versions 1 to 4, which kept a text and the paint lists as registers) does not
open (no migration).

`crates/kdm/kdm-model/model/schema.json`:

| Type | Fields (kind) | From the engine |
| --- | --- | --- |
| `Design` (root, id `design`) | `settings` (map), `nodes` (tree of `Node`), `styles` (list of `Style`), `collections` (list of `Collection`), `variables` (list of `Variable`), `images` (list of `Image`), `comments` (list of `CommentThread`), `libraries` (list of `Library`), `libraryStore` (tree of `Node`), `libraryCopies` (list of `LibraryCopy`) | `Document` |
| `Node` | `key` (integer, read-only and optional: the layer's tool id, below), `name` (string), `visible`, `locked` (boolean), `opacity` (number), `kind` (json), `frame` (json), `text` (text with marks, section 2.4), `fills`, `strokes` (lists of `Paint`), `effects` (list of `Effect`), `stroke`, `corners`, `layout`, `childLayout`, `constraints`, `prototype`, `bindings`, `instance`, `dev`, `props` (json), `mirrors` (integer; all registers but `kind` optional) | `Node` |
| `Paint`, `Effect` | `paint` / `effect` (json: the engine's paint or effect), `bound` (map: the variable bound to a field inside it, by that field: `color`, `stops/1/color`, `radius`) | `Paint`, `Effect` and the `fills/<i>/…`, `strokes/<i>/…`, `effects/<i>/…` keys of `Node.bound` |
| `Style` | `key`, `name` (string), `value` (json), `props` (json, optional) | `Style` |
| `Collection` | `key`, `name` (string), `modes` (list of `Mode`), `props` (json, optional) | `Collection` |
| `Mode` | `key`, `name` (string) | `Mode` |
| `Variable` | `key`, `name`, `collection`, `type` (string), `values` (map), `props` (json, optional) | `Variable` |
| `Library` | `key` (string: the library's id), `name`, `path` (string), `version` (integer; all but `key` optional) | `LibraryLink` (`Libraries.enabled`) |
| `LibraryCopy` | `kind` (string: `component`, `style`, `collection` or `variable`), `local` (string: the copy's root layer id or local id), `record` (json: the engine's record) | `ImportedComponent`, `ImportedRecord` (`Libraries.components`, `styles`, `collections`, `variables`) |
| `Image` | `key` (string: the SHA-256), `blob` (blob), `size` (json) | `ImageData` |
| `CommentThread`, `Comment` | exactly `kition_kdm::comment`'s types (M23); `Comment` adds the optional registers `edited` (integer) and `reactions` (json) | `Comment`, `Message` |

All container fields are declared on their types (M5: a container field cannot be added to an
existing type later).

### 2.1 The node tree

- **One document-level tree.** Every layer of every page is a `Node` object named by one node of
  `Design.nodes`; a page is a root. The tree's parent is the layer's parent and the siblings' order
  is the layers' order (bottom first, as `Node.children`). A move to another frame, another page,
  a group, an ungroup and a reorder are each one tree move: concurrent moves never duplicate a
  layer or form a cycle. Pages are the roots in order.
- **The library store's own tree (F16).** The layers of the library store (the hidden
  page-kind root `Libraries.store` names, never listed in `pages`, that holds the read-only copies
  of library components) are the nodes of `Design.libraryStore`, whose roots are the store's
  children in order. The store root itself is no object: its id is the `libraries` setting's
  `store` and its registers are the `libraryStore` setting. A layer never moves between the two
  trees (a command that moved one would make it again in the other tree, with a new object).
- **Registers by coupling (M3).**

  | Register | Holds | Why one register |
  | --- | --- | --- |
  | `frame` | `transform`, `width`, `height` | a rotation, a position and a size are one placement; a split would give a position nobody chose |
  | `kind` | the node type and its data (a vector's network, a text's box settings, base style and default list style, a component's property definitions, an instance's main component and property values, a page's background, flows, prototype settings and guides, a shape's arcs and counts) | the engine's own unit: a type change replaces it whole |
  | `stroke` | weight, align, caps, joins, dashes, brushes, width profile | one stroke style |
  | `corners` | radius per corner and smoothing | |
  | `layout` | the frame's auto layout (direction, gaps, padding, alignment, grid tracks) | gaps and padding of another direction are meaningless |
  | `childLayout` | how the layer sits in its parent's auto layout (sizing, absolute position, grid cell) | |
  | `constraints` | horizontal and vertical constraints | |
  | `prototype` | `interactions`, `overflow`, `scroll_position` | the prototype panel's unit |
  | `bindings` | style links (`styles`), variable bindings of the layer's own fields (`bound`: `width`, `fill`, `characters`…; a paint's or an effect's are its object's) and chosen modes (`var_modes`) | |
  | `instance` | `overrides` and `prop_refs` | which fields an instance overrides and which component properties drive them |
  | `mirrors` | `source`: inside an instance, the main component's layer this one mirrors | the one register V12 rewrites, so its repair never takes a person's edit of another register (F18) |
  | `dev` | Dev Mode status, note, annotations, measurements, code links, resources, the ready-for-dev snapshot | the Dev Mode panel's unit |
  | `props` | the rest (blend, clip, mask, grids, guides, exports, boolean op, constrain proportions, `exposed`, plugin data, board items, and keys a newer engine writes) | rarely edited at once |

  `name`, `visible`, `locked` and `opacity` stand alone: any combination is valid. The engine's
  serde form of a node is the source: a key the bridge does not know goes into `props`, so
  nothing is lost.
- **Paints and effects are objects** (`crates/kdm/kdm-model/src/items.rs`). `Node.fills` and
  `Node.strokes` are lists of `Paint` objects and `Node.effects` a list of `Effect` objects, one
  per item, in the engine's order (bottom first). A variable bound to a field of an item (the
  engine's `fills/<i>/color`, `fills/<i>/stops/<j>/color`, `effects/<i>/radius`) is a key of the
  item's `bound` map, so it follows its paint: one person recolouring a paint, another adding,
  removing or reordering paints, and a third binding a paint's colour all keep their edits. The
  engine's paints have no ids: the writer tells which object a paint of the new list is by an
  equal paint first, else the paint at the same place (a recolour keeps its object, an added
  paint is a new object, a reorder moves objects). A paint or effect object no live layer lists
  is deleted by the repair (V13).
- **Ballots are keys.** Each voter's ballot is its own settings key, `votes/<voter>`, so people
  voting at once all count.

### 2.2 Identity: engine ids are data

- Every object has a KDM id the KDM generates (UUID v7, M8); the root is `design`.
- **The engine's ids are stored as data** (`Node.key`, `Style.key`, `Collection.key`,
  `Mode.key`, `Variable.key`, the comment id in a thread's anchor): every replica reads the same
  `NodeId`, `StyleId`, `VariableId`, `CollectionId` and `ModeId` for the same object. So every
  reference inside a register (a prototype link's destination, an instance's main component, an
  instance swap property's default and preferred components, a measurement's other layer, a
  board connector's ends, a pattern paint's source layer, a flow's start, a style link, a variable
  binding or alias, a chosen mode, a comment's page and anchor, the library maps) means the same
  thing on every replica with no translation layer.
- **New ids are made in this session's own id space.** The engine's ids are made for
  collaboration: `Document::set_client_id` hands out node ids as `client << 32 | counter`, and
  style, variable, collection, mode and comment ids from the same allocator (`ids.rs`). When the
  bridge makes or opens a replica it gives the document a client id drawn from the replica's
  random actor id, in `1..2^21` (so every id stays below 2^53, M4b), avoiding the id spaces the
  document already uses. Two sessions that edit at the same time pick the same space with odds
  of 1 in 2^21 per pair; the repair (section 5, V3) makes even that deterministic.
- **`Node.key` is the layer's tool id** (KitObjectModel OM9: `"toolId": { "field": "key" }`, no
  prefix): the object model names every layer and page by the numeric id the kit's inspect lists
  and its commands take (`12`), in query and batch answers, receipts, `others` and `conflicts`. Only
  the kit writes it; a layer a model batch creates gets one from the repair (V3).
- **Resource tool ids:** `Style`, `Collection`, `Mode`, `Variable`, `Library`
  and `Image` expose their string `key`; `CommentThread` exposes its integer `key` (also held
  in its anchor). Keys are read-only. Command reference declarations name the resolved type;
  raw integer parameters are not implicitly object ids. Thread/node number collisions are
  repaired deterministically before a first import and during replica validation. Individual
  messages remain nested in thread bodies; category/preset records are held by settings selectors.
- **Premises:** `Node.frame` is positional. `~bounds` and `~hierarchy` are derived facts,
  computed from the same document/picker as the engine. The binding's `place.rs` narrows
  property, transform, index, connector, component and point-swap calls; unhandled families
  retain SDK defaults. The current limits and exact families are in `agent/guide.md`.
- **Saved records:** root derived `history` holds version metadata;
  `snapshots` holds SHA-256 fingerprints of version documents and image bytes; `branches`
  holds saved branch records. `assets` holds publishable identities and `published` the
  current publication manifest and content fingerprint. These are guard facts, not new persisted formats.

#### Guard coverage

The implementation reads below cover handlers and their shared mutators, not all combinations
of all commands. `Yes` does not certify that every dependency is declared. Tests in
`bindings/headless/tests/outside/review.rs` use a second document instance to rewrite the actual
file between observation and the agent's write; each repaired scenario includes an unrelated
writer control. `outside.rs::fixed_seed_mixed_table` adds 200 independent writes.

| Family | Implementation read | Holds / finding |
|---|---|---|
| Creation, property and paint/effect/grid lists | Yes: `cmd/create`, `edit/props`, `props`, `lists` | Typed parent/style/variable references; recognised property registers or indexed item/bindings; other setters retain conservative target defaults. |
| Transforms and constraints | Yes: `cmd/arrange`, `props`, `derived` | World transforms, hierarchy, layout geometry and resized descendants. Plain-frame constraint inputs are held. Root `frame` position holds remain broader than transforms because of the SDK pin. |
| Auto layout | Yes: `cmd/layout`, `structure`, `props`, `derived` | Layout operations hold consumed subtree geometry and membership; fixed-parent setter control is covered too. No proof for every normalisation combination. |
| Boolean, flatten, outline, groups and sections | Yes: `cmd/structure`, booleans, `cmd/section` | Consumed subtrees and ancestor transforms added; section visibility/status retains target defaults. |
| Components, variants, instances | Yes: `cmd/component` and its `instance`, `props`, `variants` modules | Dependency graph of mains/sets/defaults, supplied preferred/property references and target subtrees. Deleted stored defaults retain identity premises even when their subtrees are gone. Property rename/delete holds affected users' property values/bindings. Other propagation paths remain incompletely scenario-tested. |
| Variables and modes | Yes: `cmd/styles`, `variables` | Declared definitions, collection/default-mode order, copied-mode values and alias chains. Implicit mode-copy inputs and transitive aliases are held; metadata edits can still hold more than their written field. |
| Styles | Yes: `cmd/styles` and style settling | Typed references, source node, written fields, folder membership replay. `style.move` declares the names it takes. Complete propagation/user-set coverage is not proven. |
| Text and ranges | Yes: `cmd/text`, text targets and `edit/props` | Traversed text records, membership, bindings and sizing. Search/replace holds searched page text/names; range edits conservatively hold whole text records. |
| Prototype interactions/actions/flows | Yes: `cmd/proto`, `proto_actions` | Nested destinations/aliases/expressions, replay of old names, page flow/settings records. Implicit conditions hold the engine's derived `defaultCondition`. Action-path schemas are index arrays. |
| Pages and order | Yes: `cmd/page`, page targets | Duplicate/delete consume subtrees; move holds the source-to-destination window and its occupants. Destination reorder is held. |
| Comments, reactions, categories | Yes: `cmd/comment` | Thread identity/body/order for message indexes; category id/name selectors. Thread/node numeric collisions are repaired. Unrelated messages can still conflict at thread granularity. |
| Images/presets, export settings | Yes: `cmd/image`, export setters | Preset name selectors added; node export settings retain target defaults, no object ids inside those settings. Output rendering is outside this write table. |
| Library placement | Yes: `cmd/library`, import and drop helpers | Shared component resolution and drop replay, including omitted coordinates and reused local mains. External library bytes are outside the current file's revision. |
| Library publish/refresh/swap/prune | Yes: handlers, library mutators and `library_swap` | Publish holds identities/definitions and previous publication content. Holds local copy records, copy/instance dependency graphs, styles/variables, and root `~libraryUsage` for reference membership including added/deleted users. Conservative across libraries; external bytes remain a separate limit. |
| Saved versions and branches | Yes: `cmd/file/version`, `sharing/branches` and merge/diff | Version metadata separate from snapshot fingerprints; restore holds the design/resources it replaces. Branch merge/update holds main resources/settings, and branch base/head/history fingerprints include image bytes. Holds are conservative across the main file. |
| Clipboard/paste, file lifecycle and import | Yes: `cmd/edit` and `edit/clipboard`, `cmd/file` and `file/open`, `create::place_image`, library `adopt_pasted`/`merge_store`. `merge_components`, `sync_instance`, `duplicate_into`, the SVG importer and `file/load`/`file/export` skimmed | paste holds copied mains and instance mains, clipboard style/variable ids, library links/copies/store and images, and the paste parent; a repeated `edit.duplicate` without `dx`/`dy` holds the last sources/copies. `file.import` holds root resources and library records, but commands naming files are not offered to the agent. The clipboard is session state: a headless run cannot carry it across another program's save, so paste and repeat-duplicate are checked through `under` only. `image.place` `target` is a typed object parameter (held whole by default). `file.revert` is not offered. |
| Templates, data fill, text fit, variable import/export | Yes: `content/templates`, `content/datafill`, `content/textfit`, `cmd/variables_io` | `data.fill` holds the source subtree and its component graph (layers are found by name); `variable.import` holds root collections/variables/library records (matched by name, absent names cannot be named by id). `text.fit` is held by the text family; `variable.export` and `file.newFromTemplate` do not write the open document. |
| Dev status, code links/resources, measurements and playground | Yes: `dev/annotations`, `code_connect`, `dev_measurements`, `cmd/section`, `playground`; `dev_resources`, `devinspect`, `props` dev status and `derived::mark_dev_changes` in part; codegen grepped | marking ready-for-dev and `dev.resolveChange` hold the whole subtree they bless; `codeConnect.set/remove` hold the owning set/main and its graph; `codeConnect.import` holds every component/set and root tree membership; measurements hold `from` and, without `to`, its parent. Inspection, codegen, compare/focus and playground do not write the file. |
| Board/tool mutations and facilitation | Yes: `boards/board` and its `items/connectors/edit`, `board_table`, `board_templates`, `cmd/facilitate`, `tools/draw`, `tool.place`; `board/tools` grepped | `board.tableEdit` holds the whole table (rows/columns by index); `board.vote`/`board.timer` hold `settings.vote/votes` and `settings.timer`. `tool.place` is a write and holds its parent pick and the geometry of the siblings a frame or section encloses; `board.reattach` by label holds the connector's routing record and endpoint geometry. Remaining: `board.sticky/shape` coordinates do not hold the parent's layout. |
| Advanced vector editing and shape builder | Yes: `canvas/vectoredit` (most), `vectoredit/transform`, `vectoredit/width` specs, `shapebuilder`, structure vector handlers; `ops::*`/`pen_click` skimmed | every writing `vector.edit.*`, `vector.deleteHeal` and edit-mode `vector.join/smoothJoin` holds the session's edited layers whole (vertex/segment/handle/width-point indexes) plus world transforms and parent hierarchy; `shapeBuilder.merge/remove` hold targets whole, their world transforms and parent layout/hierarchy. `vector.edit.begin` and `vector.edit.end` are writes (`EDITS_WITHOUT_HISTORY`). Eligible group children of shape builder targets are held only as part of the target subtree. |
| Widgets, plugins and network | Widgets: yes (`extensions/widgets`); plugins: `DENIED`, `refusal`, `EngineHost`, `run`, `call_plugin`, `event`; sharing/plugin directory: ids only | widget writes hold the widget subtree and every stored root field (plugin code may read and write any part of the file within its manifest). `plugin.*`, `plugins.*`, `collab.*`, `online.*`, `invite.*` are not offered to the agent (`EXCLUDED_PREFIXES`); their handlers are not audited. |
| Editor tools, view, fonts and saved defaults | No complete family audit | View/preference/tool-state commands do not write the file. Tool-driven pointer mutation beyond `tool.place` and the board tools, and saved-default dependencies, are not audited. |

Propagation: `review2.rs::propagated_writes_hold_sources_and_report_targets`
checks five propagating writes (`style.update` to users, `variable.set` to bound layers, a
main's layer to instance copies, an auto layout child's width to siblings, a frame's width to a
`scale`-constrained child). A colleague's change of the source (the style/variable value, the
main's layer, the parent's spacing, the child's constraints) refuses; a colleague's edit of a
propagation target and an unrelated edit apply, and the receipt's `changed` names every
propagated layer. The reference-place `under` names the definitions it reaches as objects,
matching `covered_references`, so an unrelated layer's rename does not refuse
`variable.*`/`proto.*` writes.

External library bytes: at the pinned SDK the guard weighs only change sets of
the open file. A change of a linked library file alone produces none, so a library-consuming
write after it applies with the current bytes (`review2.rs::pinned_sdk_library_only_change_is_invisible_to_the_guard`:
the agent observed no updates, `library.acceptUpdates` then applied version 2). A kit-side
`unknown` is not expressible without an SDK hook for external inputs; the guide says so and
tells agents to read updates in the same run. This is a known silent-write gap.

All **63 allow-list entries** in `bindings/wasm/tests/object_params.rs` are checked against
their handlers. None of the 46 unused `id` members is consumed as an id by its handler.
The scanner alone does not establish this; it only checks declarations/schema membership.
The exhaustive grouping below lists every exception once (brace groups expand to
individual commands). All verdicts are valid exceptions; saved records need premises rather
than Node typing, and remote references belong to another file's namespace.

| Exceptions | Count | Implementation / verdict |
|---|---:|---|
| `edit.{pasteOver,findReplace,pickColor}.id` | 3 | `paste_over`, `find_replace`, tool setter: selection/clipboard, page search, tool state; extra `id` unused. |
| `select.{children,parent,next,previous,layersAt}.id` | 5 | Selection helpers or point picker; extra `id` unused. |
| `dev.unfocus.id`; `guide.{move,clear}.id` | 3 | Clears focus; guides resolve owner and index, never that `id`. |
| `text.edit.{selectWord,selectLine,end}.id` | 3 | Active text editor and character index; `id` unused. |
| `paint.documentColors.id`; `style.{move,renameFolder}.id` | 3 | Page; `styles`/`before`; folder/name. Extra `id` unused. |
| `variable.{createCollection,delete,duplicate}.id`; `image.presets.id`; `instance.place.id` | 5 | Allocated collection id; explicit `variable`; list presets; explicit `component` and point. Extra `id` unused. |
| `proto.{flow.rename,present,preview,closePreview}.id`; `comment.zoom.id` | 5 | `start` or closes preview; zoom uses `ids`. Extra scalar `id` unused. |
| `vector.edit.{tool,selectAll,position,delete,strokeCap,strokeJoin,bounds}.id` | 7 | Active vector editor, selected vertices/segments and numeric options; `id` unused. |
| `devMode.removeMeasurement.id`; `widget.list.id` | 2 | Explicit `from`; enumerates widgets. Extra `id` unused. |
| `board.{sticky,shape,stamp}.id`; `tool.{sticky,shape,stamp}.id` | 6 | Allocated objects or tool options; extra `id` unused. |
| `board.{voteResults,template,templates}.id`; `file.render.id` | 4 | Voter, template name/list, explicit render target. Extra `id` unused. |
| `library.{place,swapAt,import}.component` | 3 | Remote published component id/name through the shared library resolver. |
| `library.{importVariable,bindVariable}.variable`; `library.aliasVariable.alias` | 3 | Remote published variable lookup. The separate local variable/link references remain local. |
| `library.setVariableMode.{collection,mode}`; `library.extendCollection.collection` | 3 | Remote collection/mode lookup. |
| `version.{restore,rename,delete,preview}.id` | 4 | Saved history records; metadata/snapshot holds, string ids. |
| `annotation.{editCategory,removeCategory}.id` | 2 | Category settings token, held through settings. |
| `branch.archive.id`; `widget.click.target` | 2 | Saved branch record; widget callback token. Neither is a layer id. |

The mixed table's necessity rules:
id rename holds A's name/existence; name-addressed description holds Accent's name assignment
and description; point swap holds picked instance/source/parent placement; structural reparent
holds A's existence/placement and source/destination parent placement. Seed
`0x9e3779b97f4a7c15`, 50 shuffled rounds, four fresh fixtures per round. Each cell has five
attempts; values are **applied / necessary / false / wrong / failed at kit**.

| Colleague edit | Id | Name | Point | Structural |
|---|---|---|---|---|
| B name | 5/0/0/0/0 | 5/0/0/0/0 | 5/0/0/0/0 | 5/0/0/0/0 |
| A opacity | 5/0/0/0/0 | 5/0/0/0/0 | 5/0/0/0/0 | 5/0/0/0/0 |
| A name | 0/5/0/0/0 | 5/0/0/0/0 | 5/0/0/0/0 | 5/0/0/0/0 |
| A deletion | 0/5/0/0/0 | 5/0/0/0/0 | 5/0/0/0/0 | 0/5/0/0/0 |
| A move | 5/0/0/0/0 | 5/0/0/0/0 | 5/0/0/0/0 | 0/5/0/0/0 |
| Parent move | 5/0/0/0/0 | 5/0/0/0/0 | 0/5/0/0/0 | 0/5/0/0/0 |
| Style name | 5/0/0/0/0 | 0/5/0/0/0 | 5/0/0/0/0 | 5/0/0/0/0 |
| Picked instance move | 5/0/0/0/0 | 5/0/0/0/0 | 0/5/0/0/0 | 5/0/0/0/0 |
| Style description | 5/0/0/0/0 | 0/5/0/0/0 | 5/0/0/0/0 | 5/0/0/0/0 |
| Destination move | 5/0/0/0/0 | 5/0/0/0/0 | 5/0/0/0/0 | 0/5/0/0/0 |
| Total | 40/10/0/0/0 | 40/10/0/0/0 | 40/10/0/0/0 | 30/20/0/0/0 |
- **Environment words:** named resource types and nodes label by `name`, messages by `body`.
  Opacity has bounds 0 to 1. Discriminators, reference tokens and timestamps are excluded from
  random scalar edits, and library copy records are read-only. Pixels and Unix timestamps
  have no timecode `display` annotation.

A seed-7 environment run over all 20 conformance `.kdesign` files and five example fixtures,
requesting 100 tasks, produces 35 set-field, 30 replace-text, 23 delete and 12 move tasks (67
forward, 33 inverse), with no skipped sources, give-ups or `receipt-differs` discards. All 30
resampled candidates are creates refused by the kit: SDK `generate::build(Create)` copies only
scalar/text fields and omits the required JSON `Node.kind`. Minimal reproduction: batch `create
Node` in `root.nodes` with `fields: {name: "Sampled"}` fails `invalid-value (op 0): field kind of
Node is required`; adding `kind: {type: "frame"}` succeeds. The test
`bindings/wasm/tests/land11_env.rs` keeps this reproduction. This needs a sampler fix in the SDK;
making `Node.kind` optional would misstate the model. All 100 generated tasks earn reward 1 in
`kit-env trace`; no model is called.
- A reference to a deleted object dangles in the register and is a validation state: the engine
  already treats a missing main component, prototype destination or measured layer as missing
  (`document.check` reports the first).

### 2.3 Styles, variables, modes, images, settings, comments

- **Styles** are objects in `Design.styles` (`value`: the style's type and data; `props`: its
  description and bound variables). A rename and a use both survive: layers link by key.
- **Variables**: `Collection` objects hold their modes as a list of `Mode` objects (two people
  adding a mode each keep theirs); a `Variable`'s values are a map keyed by mode id, so values of
  different modes merge per key. Aliases name variable keys.
- **Images** are content-addressed blobs (`Image.blob`, SHA-256, the same hash the engine and
  `.kdesign` already use for `images/<sha256>`), one object per image hash.
- **Settings** (`Design.settings`, one key each, last writer wins per key): `name`, `kind`,
  `color_profile`, `document_colors`, `libraries` (the id of the library store's root; not the
  links or the copies' records, below), `libraryStore` (the store root's registers, as a node's),
  `image_presets`,
  `annotation_categories`, `timer`, `vote`, one key per voter's ballot (`votes/<voter>`), and any
  document-level key a newer engine writes.
- **Library links**: each library the design uses (the engine's enabled libraries) is a
  `Library` object in `Design.libraries`, in the order the Libraries view lists them: `key` is the
  library's id, `path` the library file as the document names it, a workspace path relative to
  the document's folder (or from the workspace root with a leading `/`, never a computer's path:
  `library::lent_path`), so the design keeps its libraries wherever the workspace is. Two people
  adding two libraries at once both keep theirs; one removing a library while another accepts its
  updates keeps both changes; the same library added twice at once is kept once (V9). The kit
  reads the files through its host's lent files (`bindings/wasm/src/libraries.rs`).
- **Library copies**: each record of a copy taken from a library (a component copy's root,
  asset, version and layer ids; a style, collection or variable copy's local id, asset and
  version) is a `LibraryCopy` object in `Design.libraryCopies`, named by its kind and local id, in
  the engine's order (components, styles, collections, variables). Two people placing library
  components at once both keep their copies (in the one store) and their records; two copies of
  one component placed at once become one copy with one record (V12), which every instance of
  either uses with its overrides. A component's record keeps `merged` (each merged layer's former
  id → the copy's layer) so a reference to a merged copy that arrives later follows it. The
  engine counts an update once per component, however many copies a design holds
  (`library::pending`), and updates every copy (`library::import`). Style, collection and variable
  copies have ids derived from the asset (`<library>:<key>`, the engine's), so the same asset copied
  at once is one copy (V11).
- **Comments** follow M23: each design thread is a `CommentThread` whose `anchor` names the anchor
  layer's (or the page's) object and adds design's keys (`id`, `page`, `x`, `y`, `node`);
  its first message and replies are `Comment`s (`author`, `body`, `created` as ISO 8601 UTC, and
  design's `edited` and `reactions`). Concurrent replies to one thread are both kept. A message
  keeps its `Comment` object by identity, its author and `created` time (among messages of one
  identity, by order), never by its place in the thread: deleting a reply deletes that reply's
  object, so a concurrent edit or reaction on another reply is kept.
- **Not replicated** (M2 and file-level state): the selection, the current page and the view
  (`current_page` is the editing person's), the id counter (`next_id`), the undo history, the
  file's version history, branches, published library and thumbnail (kept by the file, section 3),
  text layout caches, and who has read each comment thread (each person's own: the view keeps its
  read state through merges, and a thread that gets a message is unread for everyone but its
  author, as the engine marks it).

### 2.4 Text

A text layer's characters are its `Node.text`, a KDM `text` with marks
(`crates/kdm/kdm-model/src/text.rs`, after kition-layout's stories):

- The field holds the engine's characters and one final `\n`: every paragraph, the last
  included, ends with a break that carries its paragraph mark. The bridge writes the final break
  when the layer becomes a text and computes its edits on the engine's characters, so they never
  touch it. A field without it (an agent's batch deleted it) reads whole as the characters.
- **Character marks**: each character carries every set field of its style, one mark per
  `TextStyle` field (`fontFamily`, `fontStyle`, `fontSize`, `lineHeight`, `letterSpacing`,
  `decoration`, `decorationLook`, `case`, `fills`, `link`, `variations`, `styleId`), all
  `expand: "after"` as the engine's typing takes the style before the caret. The values are
  absolute, never relative to the layer's base style, so a character reads the same whatever
  happens to the base; a field the engine leaves unset (no fills of its own, no link) is no mark.
- **Paragraph marks**: a paragraph's list settings other than the plain ones (`ParaList`: list
  style, level, start) are the `list` mark on its break, `expand: "none"`.
- `kind` keeps the box settings (resize, alignment, spacing, truncation, features, text on a
  path), the base style new and empty text takes, and the default list style.

Two people typing in one text layer at once keep both texts; formatting overlapping ranges with
different fields both apply; a paragraph split or merge concurrent with typing keeps the typing.

## 3. The `.kdesign` file under the KDM

A `.kdesign` holds the design's **content**, not its history (kition-sdk data model 7.3, rule
M16b; task schema-freeze section 4): the replica's sealed content snapshot is the one authority
for what the design holds. The history (every change, deleted content, who did what) is the
replica the host stores (`.kition/docs/<docId>/`, the Kit UI protocol's `host/replicaRead` and
`host/replicaWrite`), never the file. Inside its zip:

| Entry | Content |
| --- | --- |
| `kdm/meta.json` | `{ "format": "kition.design-content", "formatVersion": 1, "kit": "kit.design", "kdm": 1, "schemaVersion": 1, "docId" }`: what identifies the content, nothing else |
| `kdm/content.bin` | the replica's sealed content snapshot (`Replica::sealed_content_snapshot`): its state, and of its history one `system` change labelled `content-seal` |
| `images/<sha256>` | the blobs the content's `Image` objects name (and those the branches and the published library use) |
| `thumbnail.png` | a picture of the first page, rendered on save (left as it was; the owner has not decided whether files keep one) |
| `branches/` | the file's branches as document JSON (left as they were; the owner has not decided whether branches stay) |
| `library/` | the published library (manifest, snapshot, images), when the file was published |

There is no `document.json`, no change log, no compaction and no `versions/`: saved versions are
the session's (`DocState::versions`) until the host offers kits a store for labelled checkpoints
(section 10).

- **Open**: load `content.bin` (refused as damaged when it is missing or `meta.json` names
  another docId), repair once, read the document from it. The engine's bridge opens the same
  content as the document's replica: the same docId and object ids, of the history only the seal.
- **Save**: hosts call `prepareSave` first (engine contract E4): the session's committed edits are
  written and the replica is sealed, a `system` change that is no undo step (an undo after it takes
  back the last person's or agent's step). The save writes the replica's sealed content snapshot,
  the same bytes while the design is at the file's version. One fallback, until the kit editor's
  release that calls `prepareSave` is in `web/pnpm-lock.yaml`: a save nobody sealed for writes the
  content of a copy sealed under an actor derived from the version, and `markSaved` imports that
  seal (`kition_design_kdm::file::save`). A document the bridge never held (the engine's own tests,
  `kition_design_format::save_bytes`) is written as a new replica of it, sealed.
- `openWithReplica(name, bytes, snapshot, blobs)`: a `.kdesign` of the same document as the
  host's stored replica is taken into it (`kition_kit::session::take_file`): nothing when the
  replica has the file's version, the file's changes when its history starts at a version the
  replica has, else its state as one `system` transaction keyed by object id. The stored history
  stays. A `.kdesign` of another document opens with its own content; a file that does not open
  gives way to the stored replica; any other file opens on the stored replica, recorded on top as
  one `import` transaction; empty `bytes` open the replica alone (a sync room's snapshot).
- **Identity and forks** (Kit Process Protocol 5.6, KitModel M7a; `kition_design_kdm::file::embedded_replica`
  and `fork_entries`, `kition_design_format::replica_entries` and `replace_replica`, served as
  `document.identity` and `document.fork`): a byte copy of a file is the same document; its
  identity is the content's docId. A fork writes the copy's content under a new docId, no
  `branches/`, only the images the copy's state and the published library use; the thumbnail, the
  published library and unknown entries are copied as they are.
- **Another program's edit**: `kition_design_format::save_edited` writes a `.kdesign` read and
  edited as a document as the same document, its replica taking the edit as one `import`
  transaction (`kition_design_kdm::file::rewrite`).
- **Read again after another program saved it** (kition-sdk `reconcile_reread`): a clean design
  keeps its replica, its undo histories and its editor state. What the file holds differently is
  written as one `import` transaction (`file reread`), outside both undo scopes, so each undo step
  still takes back only its own change; branches and the published library are the file's, the
  saved versions the session's. The editor state saved before the reread must resolve on the file as read again: a
  deleted page, selected layer, edit-mode layer, Dev Mode focus or guide holder is refused with
  its reason (`deleted active target <id>`) and the previous session stays.

## 4. The bridge: an engine document view plus diff writes (E11 to E13)

- **Writes (E12).** Commands, tools and keys run on the engine's `Document`. At the end
  of every top-level call (a command, a committed interaction, a group, a tool event the binding
  forwards) the bridge writes what the committed document changed since the replica last saw it:
  one transaction per history step pushed since, in order, using each step's snapshot as the
  state between them; a change that pushed no step goes to the step on top. A diff walks only the
  nodes whose `Arc` changed (the engine's documents share every untouched node), so a one-layer
  edit writes one object's changed registers. The tree is reconciled per parent whose children
  changed. A KDM tree move puts a node last among its siblings, so the children that stay are
  the longest start of the new order that keeps the old order (a subsequence of the old
  children); every other child is moved (or created) after them, in order. Bringing the bottom
  layer of any number to the front is one move, and a concurrent reorder of other layers keeps
  its moves; sending the top layer to the back still moves every other one (a longest increasing
  subsequence would need a move to a position, which the KDM tree does not have).
- **Reads (E11).** After an import, an undo or a redo the view is patched: the tree is re-read
  (one pass), only changed objects' registers are re-read, and every other node keeps its `Arc`
  (caches and all). The view is never normalised after a read.
- **Reads never fail on a value.** Registers and settings are `json` and the KDM takes any value
  from any peer (a headless agent's `model.batch`, another build). A layer whose registers do not
  read as the engine's node reads with an unreadable `kind` as a frame (it holds any children) and
  every other register that does not read (with those before it, in schema order) unset; a
  setting that does not read (with the keys before it, in key order) is left out. Nothing is
  rewritten: the value stays in the replica until someone edits that field, and every replica
  reads the same view (`fields::node_from_lenient`, `read::settings_from`).
- **Imports (E13).** `.fig`, `.sketch` and REST JSON imports, templates and new files are written
  in the replica's creating transaction (`create_with`, P2), never as undo steps.
- A replica that refuses a write or an undo is left (`kdmResets()`), as kition-vector does: the
  document keeps working on its own history and the next edit makes a fresh replica.

### 4.1 Derived edits only in local commands

The engine's `normalize` (after every command) keeps derived values consistent: text boxes
resize to their content, groups and boolean operations wrap their children, instances follow
their main components (and record overrides), auto layout re-flows, variable bindings and style
links feed node values, ready-for-dev frames record changes. **These run only inside a local
command's transaction, never after a merge, an undo or a redo**: two replicas re-flowing or
re-propagating the same merge would each write the result, and derived writes made after a merge
would be attributed to whoever merged. A merged state that is not settled (A changed a frame's
padding while B added a child) is shown as merged; the next local command touching it settles it
in its own transaction. Instance propagation is written by the command that edited the main
component, so peers receive the propagated instance layers as ordinary changes.

## 5. Validation and repair (E14)

Run after open, every import, undo and redo, in one `system` transaction labelled `repair`,
only when something is wrong. Every rule depends on the state alone. A repair drops, clears,
renames or moves; it inserts only under an object id derived from the state (V8, V12), so
replicas that repair one state concurrently make the same object, which the KDM keeps once (and
V7 its tree node), rather than one copy each, and it sets a setting only to values derived from
the state (V10). The tree rules (V3, V4, V5, V7) hold for both trees, `Design.nodes` and
`Design.libraryStore`.

| # | State | Repair |
| --- | --- | --- |
| V1 | A list entry naming no live object (styles, collections, variables, images, comments, a collection's modes, a thread's comments) | drop the entry |
| V2 | The same object twice in one list | keep the first |
| V3 | Two objects of one type with the same engine key, a layer with the library store root's key (the `libraries` setting's `store`, else V10's), or a layer without a key (a model batch made it) | the one whose object id sorts later (every such layer, for the store's key) gets a key derived from its object id that no live object of the type has, nor one this repair gave before (a layer without a key gets one first, in object id order; nodes and comment ids: a number in the reserved id space `2^21 - 1`, the hash of the object id, else of `<id>#1`, `#2`, …, the store root's key counted as taken; others: the key plus `~` and the object id's first 8 characters, else plus `~1`, `~2`, …), in key order |
| V4 | A layer whose parent is not a container (a concurrent type change), or a root of `Design.nodes` that is no page (a layer with no container ancestor, or one a peer wrote there) | move it to the nearest container ancestor, last; a root that is no page (after V8) moves into the first page that is not a divider (else the first page), last, in tree order |
| V5 | A `Node` object no tree node names (created under a page deleted concurrently) | delete it (delete wins over edit) |
| V6 | A `CommentThread` with no comments | delete it |
| V7 | One layer named by two tree nodes (a redo made the layer a new node while another replica's concurrent move kept the old one alive) | the KDM's `tree_dedupe` (kition-sdk KitModel M10b), run first: the least node by engine id among those with no ancestor naming the layer stays, the others' children move under it, the others are deleted |
| V8 | No page left that is not a divider (two people deleted the last two pages at once; the engine refuses deleting the last one locally) | add an empty page (the engine's new page, "Page 1") as the last root, object id `page-` plus a 64-bit FNV-1a hash of the sorted object ids of the tree's roots (the deleted pages included), key in the repair space (probed past live keys, as V3); nothing when that id is taken |
| V9 | Two `Library` objects with one key (two people added the same library file at once) | the object whose id sorts first stays; the others are deleted and their list entries dropped |
| V10 | Layers in `Design.libraryStore` but no store root in the `libraries` setting (the person who made the store undid it after another added a copy to it) | the setting names the root under the key `2^21 - 1` (the repair space) `<< 32` plus the 32-bit FNV-1a hash of `libraryStore`, and a missing `libraryStore` setting gets the engine's new store's registers; the reader reads the same store before the repair |
| V11 | Copies of one library asset taken at once: two `LibraryCopy` objects of one kind and local id, or two `Style`, `Collection` or `Variable` objects of one key that a `LibraryCopy` names (run before V1 and V3) | records: the one of the lowest library version stays (then the object id that sorts first), so an update is offered when either copy is older; objects: the one whose id sorts first stays (a collection with its modes); the others are deleted and their list entries dropped |
| V12 | Copies of one library component placed at once: two `LibraryCopy` objects of kind `component` with one library and asset whose root layers are in the store (run after V11, before V1, V3 and V10) | one new copy replaces them (F17): the copy of the lowest library version (then the record whose object id sorts first) is made again under object ids derived from the merged records' sorted ids (`copy-` plus a 64-bit FNV-1a hash; a layer's adds its old object id), with keys in the repair space (probed past live keys and the store root's), as the last root of `Design.libraryStore`, and one record under the derived id at the first merged record's place (`merged` names every merged layer and the aliases the merged records held); the merged copies' layers and records are deleted. Every layer that names a merged copy's layer (main, mirrored layer, instance swap value, preferred component, prototype destination, measurement, pattern source) names the new copy's layer for the same library layer instead, rewriting only those registers (a mirrored layer's is `mirrors`, F18); overrides and the rest stay. A copy left alone points late references to the layers its `merged` names (layers no live layer holds) at its own. When the record's derived id was an object's before (M8 refuses it again), the merge is left out |
| V13 | Paint and effect objects of a layer deleted at once, or list entries of a layer's `fills`, `strokes` or `effects` naming a deleted object or an object listed before (run after V12) | the entries are dropped (an object listed twice keeps its first entry, in tree order); `Paint` and `Effect` objects no live layer lists are deleted |

Not repaired: references to deleted objects inside registers (section 2.2), a missing blob
(it may arrive late; the image reads as missing until it does), and a register or setting the
engine cannot read (section 4: it reads as unset; the repair never rewrites it, so a value a
newer build writes survives older replicas).

Repairs that create objects under derived ids (V8, V12) share one limit of the KDM: replicas
that make the same object concurrently keep one replica's object whole, so an edit of a
repair-made object made on a replica before it receives another replica's identical repair can
be lost. For V12 that is accepting a library update in that moment; the update is then offered
again. As KitModel M12 has it, a repair's write survives the undo of an earlier step; V12 writes
a mirrored layer's pointer only in its own register (`mirrors`, F18), so undoing an edit of that
layer made before the merge (a blend mode) still takes it back.

## 6. Undo scopes (E14a, M12a)

- Every history step remembers its undo scope (`person` or `agent`, from the origin the command
  ran as: `executeAs` with an `agent` origin is the agent's) and how many replica undo steps it
  took.
- `edit.undo` and `edit.redo` take an optional `scope` (`"person"` or `"agent"`, read with
  `kition_kit::session::history_scope`); without it they work in the scope of the origin they
  run as. They take back the newest step **of that scope**, wherever it is in the history: the
  replica steps back that many transactions in that scope (`undo_in`), and the document becomes
  the replica's result, which keeps every later change of the other scope and of other replicas
  (M12). When the step was the newest of all and the replica holds exactly its snapshot, the
  snapshot is the document (the engine's own undo, ids, caches and saved state unchanged).
- `undo()`, `redo()`, `canUndo()`, `canRedo()` and `inspect()`'s `canUndo`/`canRedo` are the
  person's. A new step clears only its own scope's redo steps.
- Undo groups (`grouped`, interactions) stay one step of the scope that made them.

## 7. Object model and agent

`kit.json` has `engine.contract: 2`, the capability `kdm`, `model` (the schema and its
version), `collaboration` (`merge: ["structured", "rich-text", "binary"]`, no leases, no server
validation) and `objectModel: { "namespace": "Design" }`; `agent.objectModel` is `true` and
`kition-design` runs `kition_headless::run_manifest_model` with the features `model.open`,
`model.import`, `model.export`, `model.save`, `model.describe` and `model.batch`
(`ReplicaPort`: the object model over the document's change exchange, as kition-vector).
`design_query` reads objects by type and field; `design_batch` edits registers or runs commands. In the
editor the kit answers `model/*` itself (`export_kit_session!`), so a batch is one undo step of
the agent. Headless, `kition-design` answers the object model over the change exchange (platform rule
OM6): its register edits arrive as a collaborator's changes and are not undo steps, while the
commands a batch runs are the agent's undo steps.
Every writable type declares `"batchOps": { "*": { "direct": true } }`: a raw register, list or
tree operation reads only the object, field, reference and insertion window it names (the bridge
runs no auto layout, text fit or propagation after a merge, section 4.1), so the guard's ordinary
holds describe it. `LibraryCopy` is read-only and declares none.

## 8. Convergence scenarios (L6)

`bindings/wasm/tests/convergence.rs` drives two to five `DesignDocument`s through
kition-sdk's `kition-convergence` (`KitAdapter`): a document of two frames with shapes and a
text layer; `SetField` sets a probe rectangle's `opacity` and `name` (two fields of one object);
`Create` adds a rectangle; `Delete`, `Duplicate`; `Reorder` sends a layer backward or forward;
`Reparent` moves one frame into the other (and the other way for `which` 1, so the tree must
refuse the cycle); `InsertBlob` places an image; random edits pick from create, move, resize,
recolour, rename, hide, lock, group, ungroup, auto layout, delete, duplicate, text change, style
creation and variable creation, adding and removing library links (every replica is lent
the same two synthetic library files) and placing an instance of an enabled library's
component (`library.place`, which makes the store and copies). `bindings/wasm/tests/libraries.rs`
holds the deterministic two-replica regressions: two people placing the same library component
and binding the same library variable at once in a design without copies (one store, no page,
one component copy and record, one style, collection and variable copy, and one acceptance of an
update reaching every instance), the store's maker undoing it after another person added a copy
(V10), and V12's: one copy for a component placed at once, with an instance override kept and
the update counted once; either person undoing (and redoing) their placement after the merge; a
reference to a merged copy made before hearing of the merge (a duplicated instance); and an edit
of a mirrored layer crossing the merge, then undone. The text scripts (`InsertText`,
`DeleteText`, `SplitParagraph`, `MergeParagraph`) type into the text layer `Title` through
`text.set`, whose characters the bridge writes as text edits. Every scenario must pass, the four
text scenarios included; `level6()` must hold, the report is saved with `save_if_requested()`, and the
WebAssembly parity check runs when `KITION_WASM_ENGINE` is set. `bindings/wasm/tests/perf.rs`
measures the collaboration budgets on the largest example design
(`examples/ridgeline-studio-dashboard.kdesign`), a keystroke being one edit and the repaint of a
1440 × 900 view.

Measured on an Apple M-series machine (release build, other builds running):

| Objects, snapshot | Open | 1,000 remote changes | Keystroke p50 / p95 | Memory per replica |
| --- | --- | --- | --- | --- |
| 285, 160 KB | 9.2 ms (budget 50) | 2.4 s (budget 10 s) | 4.4 / 6.0 ms (budget p95 50) | 3.4 MB (budget 16) |

The suite passes L6 with the four text scenarios (schema 1); before schema 1 (13 scenarios
passing, the text scenarios skipping) it also passed long fuzz runs:
fifteen runs of 100 seeds × 200 steps on 2, 3 and 5 replicas
(`KITION_CONVERGENCE_SEEDS=100 KITION_CONVERGENCE_STEPS=200`, `KITION_CONVERGENCE_SEED_BASE` 0 to
1400 in steps of 100), with library links and `library.place` among the random edits (a run
merges copies about 200 times), all pass, and so does the WebAssembly parity check. V7 is what
keeps a layer from being in the tree twice in those runs. The engine's ids come from each
session's random id space, so the layers a seed picks differ from run to run.

## 9. Decisions taken

| # | Decision | Precedent |
| --- | --- | --- |
| F1 | One document-level node tree, pages as roots (the library store in its own tree, F16) | kition-vector `Drawing.nodes`, kition-layout `Layout.items` |
| F2 | Registers by coupling as in section 2.1, unknown keys into `props` | kition-vector `Node` registers, design note M3 table |
| F3 | A text layer's characters are `Node.text`, a KDM `text` with one mark per style field (absolute values) and the list mark on each paragraph's break, the final break included (section 2.4); `kind` keeps the box settings. Superseded: the characters, runs and paragraphs in the `kind` register (two people typing kept one text) | kition-layout (`Story.text` with marks, the final break) |
| F4 | Engine ids are data, minted in a per-session id space; no translation of references | deviation from kition-vector (which translates ids); the engine's own collaboration ids (`set_client_id`) make it safe, and a missed reference site cannot point at the wrong layer on another replica |
| F5 | Comments on M23's reserved types | M23, kition-layout (comments declared up front) |
| F6 | The `.kdesign` file holds the replica's sealed content snapshot under `kdm/` (no `document.json`, no history), sharing `images/` as blobs; the history is the host's stored replica. Superseded: the full snapshot and a change log beside `document.json` | kition-board `.kboard` (content file, 4a37af5), kition-sdk data model 7.3 (M16b) |
| F7 | Derived edits (auto layout, text fit, group fit, instance propagation, bindings) only in local commands | design note 4.4 (autoflow never after a merge) |
| F8 | Undo steps carry their scope; an undo of one scope keeps the other's later work | M12a |
| F9 | The bridge is off in the engine's own tests and on in the bindings | kition-vector (replica made by the binding's calls) |
| F10 | Leave the replica on a refused write (`kdmResets`) | kition-vector |
| F11 | `collaboration.merge` `structured`, `rich-text` (comment bodies), `binary`; no leases | kition-vector, kition-layout |
| F12 | kition-sdk is pinned by one commit for all its crates (it has `Tx::tree_dedupe`, KitModel M10b, which V7 needs, undo scopes, `history_scope`, `kition-convergence` and `run_manifest_model`) | Cargo.toml |
| F13 | `contractVersion()` stays 1: kition-sdk's export macro answers `CONTRACT_VERSION` 1 and `@kition/sdk/editor` and Kition refuse a higher number; the kit is contract 2 by its manifest and its session members, as platform `kits/engine.md` allows | kition-vector, kition-layout |
| F14 | When concurrent deletes leave no page, the repair adds one empty page (V8) under an object id derived from the state, instead of leaving a design the engine cannot show | KitModel M22 (conflict objects under ids derived from what they are about, kept once when recorded concurrently) and M8 (ids a host chooses) |
| F15 | Library links are `Library` objects keyed by library id; a library file is named by a workspace path relative to the document (KitLinks HC-P2, E5) and read through lent files | kition-film (clips as relative lent paths), M5 (a container field means a new schema version) |
| F16 | The schema declares the library store's layers as their own tree (`Design.libraryStore`) and each copy's record as a `LibraryCopy` object (`Design.libraryCopies`). The store root is no object (its id and registers are settings both writers write alike), so concurrent placements never make two roots, and a root of `Design.nodes` is always a page. Rejected: a store root under an id derived from the state (as V8's page): the KDM refuses to create an object again under the id of one deleted earlier, so placing a component after undoing the first placement would fail; and a store root per person merged by a repair would drop the other copies when its maker undid it | M5 (container fields up front, a new schema version), M8 and M22 (deterministic ids, considered), kition-layout (comments declared up front) |
| F17 | Copies of one component placed at once become one copy the repair makes under ids derived from the merged records (V12), with no schema change (the record's `merged` aliases are engine data in its `record` json). Rejected: keeping one of the copies (the lowest object id): each copy is one person's placement, and its maker's undo would delete the kept copy and leave the other person's instances without a main (the repair's own objects are no one's undo step, M12); and keeping both with an update count per component only, which leaves two copies in the store | M22 and M8 (objects under ids derived from what they are about, kept once when made concurrently), V8 |
| F19 | Paints and effects are `Paint` and `Effect` objects in their layer's lists, each with its value and the bindings of its own fields (`bound`, a map); the writer pairs the engine's paints with objects by equality, then place. Superseded: the lists as `json` registers and bindings by index in `Node.bound` (a concurrent reorder moved a binding to another paint) | kition-sheet (cells as objects), kition-slide (`TextBody` lists) |
| F20 | Who has read a comment thread (`readBy`) is each person's own and never replicated; each voter's ballot is its own settings key (`votes/<voter>`) | M2 (private state stays out of the replica) |
| F18 | The schema keeps a mirrored layer's pointer (`source`, the main component's layer it mirrors) in its own register `Node.mirrors`, not in `props`, so V12 rewrites only that register and the undo of a person's or agent's earlier `props` edit (a blend mode set before the merge) still reverts it. Rejected: keeping `source` in `props` and accepting the M12 limit (an undo that silently does nothing), and writing the repair's value as a split of `props` keys (registers are last-writer-wins as a whole) | M5 (a register field on an existing type under a new schema version), M12 (a repair's write survives the undo of an earlier step), F2 (registers by coupling) |

## 10. Known gaps

- A paint's value (and an effect's) is one register: two people changing one paint's colour and
  its opacity at once keep one of the two (the paint is a property bag, split later without a
  container).
- Saved versions (`file.saveVersion`, autosaves) live in the session only: the file holds no
  history, and the host's history store (`history.json` in `.kition/docs/<docId>/`) records its
  own unlabelled checkpoints on `host/replicaWrite` but offers a kit no way to record a labelled
  checkpoint or to list and read them. Until it does, a design's versions are gone once it closes.
- `current_page` (the page the editor shows) is not in the file either: a reopened design opens on
  its first page (the host has no place for a kit's view state yet).
- The blob store keeps every image added in the session (no collection of blobs only old undo
  steps need, unlike kition-vector's `blob_gc`).
- The engine's own live session (`sharing/live`, its server protocol) is separate from the KDM and
  stays off in Kition.
- An edit of an object a repair made under a derived id (V8's page, V12's copy) made before the
  replica receives another replica's identical repair can be lost (section 5).
