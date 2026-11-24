# Real-time collaboration protocol (draft)

Status: this document describes the collaboration protocol the engine carries. **What this
repository holds:** the core
(`crates/engine/collab`: identifiers, the object/property model, change sets, ordering and
validation, client reconciliation, per-user undo and redo, offline queueing and reconnect, derived
data, join codes and the message encoding) and the engine's live-session state and `collab.*`
commands on top of it (`crates/engine/engine/src/sharing`). **What it does not hold:** a network
transport, a session host or a server. The engine reaches the network only through a `LiveLink`,
and nothing here installs one, so the commands answer that live sessions aren't available; the
kit's bindings also leave the `collab.*` commands out (`bindings/wasm/src/commands.rs`), and the
editor's Share button is disabled. In Kition, people co-edit a design through its KDM replica,
which the host relays ([`kdm.md`](kdm.md)). The sections below say what is built; where a message
needs a server to answer it, only the message types and the engine's side exist here; the transport, hosting, the server and
their user interface are outside this repository.

Goal of the protocol: several people edit the same `.kdesign` file at the same time and see each
other's edits, cursors and selections live, without a central cloud service being required.

## Design in one paragraph

Clients talk to a **session server** over **WebSockets** using small **binary messages**. The
server is the single authority for ordering: it assigns every accepted change a sequence number and
rebroadcasts it. The document syncs as **state, not operations**: each change says "property P of
object O is now V" (plus create, delete and move), and conflicting writes to the same property
resolve **last-writer-wins in server order**. Clients apply their own edits immediately
(optimistically) and reconcile when the server echoes or overrides them. Cursors, selections and
viewports are **ephemeral presence** messages that are never stored.

## Why this shape

Public engineering write-ups about multiplayer design tools converge on the same choices, and they
fit Kition Design's architecture:

- **WebSockets**: one long-lived, bidirectional, ordered connection; available natively and in
  browsers, and it passes through proxies and TLS (`wss://`) like ordinary HTTPS. Plain HTTP
  polling adds latency and overhead; WebRTC peer-to-peer complicates NAT traversal and gives up a single ordering authority.
- **A central authority instead of OT or a full CRDT**: a design document is a tree of objects with
  properties, not a text buffer. Operational transformation is complex and fragile for that shape;
  general CRDTs carry metadata and tombstones we don't need when a server orders everything. With
  per-property last-writer-wins plus a few tree rules (below), every client converges to the
  server's state, and the server can reject invalid changes (cycles, edits to deleted objects).
- **State-based changes instead of replaying commands**: Kition Design's commands are deterministic
  locally, but replaying them on other clients would require identical documents, fonts and
  floating point everywhere. Instead a command runs once, on the client that issued it, and the
  resulting document difference is what gets sent. Kition Design's document is a persistent map of
  `Arc<Node>` with dirty tracking, so computing the exact changed nodes and properties after a
  command is cheap.
- **Binary encoding**: cursor traffic is frequent and documents are large; a compact, schema-based
  binary format keeps messages small and fast to decode. JSON stays available for debugging and
  for agents (the same messages have a JSON form).

## Identifiers

*Built.*

- **Client id**: a 32-bit id the server assigns to each connection on join. The server starts
  above every client id space already used by node ids in the file, so ids from earlier sessions
  are never handed out again. The counter is part of the session state a server saves
  (`Server::state` / `Server::restore`), so a restarted server doesn't hand ids out twice either;
  a client that reconnects keeps its id.
- **Node ids** are unique across clients without coordination: with a client id set
  (`Document::set_client_id`), `Document::alloc_id` returns `client_id << 32 | counter`,
  continuing after the highest id of that space already in the document. Ids in existing files
  stay valid (they occupy client id 0), and a document without a client id allocates exactly as
  before. The counter is saved as the file's `next_id`; the client id is never saved, and neither
  is part of document equality.
- An id is never *allocated* twice. A deleted node can come back with its id only by being
  created again whole (what undo does): the server accepts creating an id that doesn't exist only
  if it was deleted before or lies in the sender's own id space.
- **Document-level ids** (`kition_design_doc::ids`): styles, variables, variable collections, modes
  and comment threads created during a session take a number from the same allocator as node ids
  (`client_id << 32 | counter`, never handed out twice): `S:<n>`, `VariableID:<n>`,
  `VariableCollectionId:<n>`, `<collection>/mode<n>` and comment id `n`. So two people creating a
  style (or a variable, a comment…) at the same moment get different ids and both survive. Outside
  a session ids are made as single-user editing makes them (`S:<k>` with the first free `k`, one
  above the highest for variables, collections and comments, the first free `mode<k>`). The
  number's high 32 bits are its id space; small numbers (ids made outside a session) and ids of
  other forms (library copies `library:key`, files placed into the document)
  are in space 0. Server client ids start above every space used by any of these, not only by
  layers.
- The server refuses a batch that creates a document-level entry (by `Put` or by replacing a whole
  list) whose id lies in the space of another client this session gave an id to, unless the entry
  was removed earlier in the session (undo puts entries back, whoever made them). Ids in space 0 or
  in spaces this session never handed out (earlier sessions, other files) are accepted: they can't
  collide with what a connected client makes, because a client's numbers continue above every id of
  its space already in the document. Removed entries are part of the saved session state
  (`SessionState::removed`).
- Objects inside an entry are not separate objects: modes are part of their collection, a
  variable's values per mode part of the variable and replies part of their comment thread. Two
  people adding a mode to the same collection, editing different modes of one variable, or replying
  to the same thread at the same moment still resolve last-writer-wins on the whole entry (mode ids
  no longer collide, so nothing is mixed up; one of the two additions is lost).

## The document as objects and properties

*Built* (`kition_design_collab::{Change, Prop, DocProp, Entry, Replica}`).

Every synced thing is an **object** with an id, a type and a set of **properties**:

- Nodes: every field of `Node` that is saved in the file is a property (name, geometry, fills,
  strokes, effects, auto layout, text content and runs, component data, bindings, prototype
  interactions…). Large or list-valued fields (fills, text runs, vector networks) are a single
  property each: replaced whole, last writer wins.
- **Tree structure** is two properties on the child: `parent` (an object id) and `position` (a
  **fractional index** string, e.g. between `"a0"` and `"a1"` sits `"a0V"`). A parent's children
  are its children sorted by `position`, so two clients inserting into the same frame never
  conflict, and a move is one property write. The server rejects a `parent` write that would
  create a cycle or point at a deleted object, and replies with the current value.
- Document-level data (pages order, styles, variables, collections, modes, published library
  metadata, comments, versions) are objects with properties in the same way.

As built:

- **Node properties** are `parent`, `position` and one property per saved field of `Node` (name,
  kind with its per-type data, visibility, lock, opacity, blend, transform, size, fills, strokes,
  stroke settings, effects, corners, constraints, auto layout, child layout, clip, masks, grids,
  guides, exports, interactions, overflow, scroll position, style links, variable bindings and
  modes, instance source, overrides, property references, plugin data, handoff status and note,
  annotations). `children` is not a property: it follows from the children's `parent` and
  `position`. The build fails when `Node` gains a field that isn't listed.
- **Positions** are fractional keys (`kition_design_collab::frac`: base-62 digits after a length-coded
  integer part, so appending keeps keys short; deterministic; never ending in `0`). A replica
  orders every parent's children, and the pages, by `(key, id)`; equal keys from simultaneous
  inserts fall back to id order. Existing documents get keys in their current order on first sync.
  A diff gives new keys only to the children that need them (the longest run of old keys that is
  still increasing keeps its keys) and re-spaces a parent whose keys would grow past 48 characters.
- **Document-level objects covered now**: the file name, the current page, document colours, the
  colour profile, image presets, annotation categories and library data (whole values); styles,
  comment threads, variable collections and variables (one entry per id plus the order of ids;
  the whole list when ids aren't unique); dev-mode snapshots (per layer); images (per content
  hash, the encoded bytes as base64); a board's timer and vote (whole values, `DocProp::Timer` and
  `DocProp::Vote`) and the vote's ballots (`Entry::Ballot(voter)`, one per voter, so people voting
  at once write different entries). The timer travels as its end time (or, paused, the time left),
  never as ticks: each client counts down on its own clock. The page order is the pages'
  positions.
- **Next**: this file's published library (it embeds a snapshot document; it moves with an asset
  channel, together with images, which then stop travelling inside batches); finer entries for modes,
  per-mode variable values and comment replies (see Identifiers).
- **Changes** (`Change`): `Create {id, kind, parent, position, props}` (only the properties that
  differ from a new node of that kind), `Set {id, prop, value}`, `Delete {id}` (the subtree; a
  delete of something already gone does nothing), and for document-level data
  `SetDoc {prop, value}`, `Put {entry, value}` and `Remove {entry}`. Values are JSON.
- **Diff** (`kition_design_collab::diff(replica, edited_document)`): only nodes whose `Arc` differs are
  visited (the persistent map skips shared structure), so the cost follows the size of the edit. A
  batch lists field writes, creations (parents first), moves, deletions (one per subtree, after
  the moves out of it), then document-level changes. `Replica::apply` applies a batch strictly
  (stop at the first invalid change, then check the result: the server's mode) or leniently (skip
  what no longer applies: a client replaying its pending edits). For random edits made by real
  engine commands, `apply(diff(a, b), a) == b`, positions included (tested).

## Messages

*Built* (`kition_design_collab::message`): the types below and both encodings. Protocol version 2 (the
`Hello`'s `protocol`): the welcome says what the client may do, and a connection can manage a
server's files or use a file's team library instead of joining one; a client of another version is
refused with the reason. (The library messages came later within version 2: a server from before
them can't read them and refuses the connection.)

**Signals (cursor chat, spotlight)** came later within version 2 too, without breaking older clients
in either direction. A client that reads them says so with the word `signals` in its hello's
`client_version` (`"0.3.1 signals"`); the server sends `Signal` messages only to those clients, so
an older client never gets a message it can't read. A server that takes them answers such a
client, right after its welcome, with a `ready` signal about the client itself; a client sends
signals only after that, so an older server never gets one either. A signal is a kind and a text, so kinds added later are skipped by clients and
dropped by servers that don't know them.

**Open hosting and reports** came later within version 2 the same way. A connection that opens
with `OpenHosting` asks a server about its hosting, publishes without the key when the server is
open to the public, or manages one file with its owner token; a server older than that can't read
the message and refuses the connection.
A client that reads `Hosting` and `Reported` says so with the word `hosting` in its hello's
`client_version` (`"0.3.1 signals hosting"`); only those clients get a `Hosting` after their
welcome, and a client sends `Report` only to a server that sent it one. New variants only ever go
at the end of an enum, and existing messages keep their fields, so nothing an older peer reads
changes.

**Join codes** came later within version 2 the same way: a connection that opens with `Resolve`
asks a server which file a code opens, and `Admin code` asks it for a link's code. Both are new
variants at the end of their enums, so everything an older peer reads is unchanged. A server from
before join codes can't read either message and refuses the connection ("that message couldn't be
read"). An older client never sends them and joins by link as before.

**Owned files** came later within version 2 the same way: `Admin owned` on an open hosting
connection asks what became of the files a list of `(file id, owner token)` pairs owns, in one
request. It is a new variant at the end of `Admin`, and its answer one at the end of the replies,
so everything an older peer reads is unchanged. A server from before it can't read the request and
refuses the connection ("that message couldn't be read"). An older client never sends it.

**Encoding (decided):** a binary message is a 1-byte wire version followed by **postcard** (serde),
which is compact and needs no schema files. Property values inside are compact JSON text: the
document types use tagged and flattened serde forms that only self-describing formats can read
back, and JSON keeps them exact (floats round-trip). The **JSON form** of every message is the same
types through `serde_json`, externally tagged (`{"changes": {"base_seq": …}}`; postcard can't read
internally tagged enums). Measured: a typical edit (one layer moved: its transform) is **43 bytes
binary vs 139 JSON**; a cursor update is **23 bytes vs 93**. Decoders take a size limit, refuse
trailing bytes and never allocate much beyond the message's size; JSON nesting is capped by the
parser; both are fuzzed.

Client → server:

| Message | Meaning |
|---|---|
| `Hello {protocol, file, token, name, color, client_version, resume?}` | Join a file's session; `resume {session, client_id, seq}` when coming back after a dropped connection. |
| `Changes {base_seq, batch_id, changes: [Change]}` | One user action's changes (one undo step). |
| `Presence {cursor?, selection?, viewport?, editing_text? {node, caret, selection?}}` | Ephemeral; throttled to ~30 Hz, latest wins. |
| `Ack {seq}` | Highest sequence applied (lets the server trim per-client buffers). |
| `Ping` | Keep-alive. |
| `Manage {protocol, key, client_version}` | Instead of `Hello`: manage a server's files with its key. |
| `Admin {publish {name, snapshot} \| files \| links {file} \| createLink {file, access, label} \| revoke {file, link} \| remove {file}}` | A request on a managing connection; each gets one `Admin` reply, in order. |
| `OpenLibrary {protocol, file, token, client_version}` | Instead of `Hello`: use a file's team library with one of the file's share links. |
| `Library {fetch {have?, usage?} \| publish {archive} \| usage}` | A request on a library connection; each gets one `Library` reply, in order. `usage` is `{file, components: [(id, instances)]}`. |
| `Signal {kind, text}` | Ephemeral presence, only after the server's `ready`: `chat` (the cursor chat message; empty: closed) or `spotlight` (`"on"`, or empty: stopped). |
| `OpenPlugins {protocol, key, client_version}` | Instead of `Hello`: use the server's plugin directory; `key` empty to browse, the server key to publish too. |
| `OpenHosting {protocol, client_version, file, owner}` | Instead of `Hello`: ask about the server's hosting; publish without the key (`Admin publish`) when it is open to the public; with a file's id and owner token, manage that file (`Admin links \| createLink \| revoke \| rename \| remove` of it). Answered with `Hosting` or `Refused`. |
| `Admin {rename {file, name} \| listing \| reports \| block {ip} \| unblock {ip} \| blocked}` | Later managing requests: rename a file on the server, the files with what the operator moderates by, the reports, blocking addresses (an IPv6 address: its /64). Owner tokens may only rename their file. |
| `Report {reason}` | On a joined connection, only to a server that sent `Hosting`: report the file to the operator (at most 500 characters kept; ten an hour per address). |
| `Plugins {list {query} \| fetch {id, version?} \| publish {bundle} \| unpublish {id, version?}}` | A request on a plugin directory connection; each gets one `Plugins` reply, in order. `bundle` is a plugin bundle's bytes (`kition_design_plugin::bundle`). |
| `Resolve {protocol, client_version, code}` | Instead of `Hello`: ask which file a join code opens (the code's nine symbols). Answered with `Resolved`, then the server closes the connection; a code it doesn't know, too many wrong ones from the address, or a session's own host (not a server) is `Refused` with the reason. |
| `Admin {code {file, token}}` | A later managing request (with the server key, or the file's owner token on an open hosting connection): a new join code for the link of `file` whose token this is. A link has one code; asking again replaces it. |
| `Admin {owned {files: [[file, owner]]}}` | A later request, on any open hosting connection (also one that named no file when it opened): what became of the files these `(file id, owner token)` pairs own, at most 200 a request. Answered with `Admin owned`. |

Server → client:

| Message | Meaning |
|---|---|
| `Welcome {session, client_id, seq, catch_up: Snapshot or Delta, processed, peers, access}` | Join accepted: the full document, or for a reconnecting client the batches it missed; what the server did with that client's batches (`processed {last_batch, refused}`); `access` is `edit` or `view` (a view link: every batch is refused). |
| `Changes {seq, author, batch_id, changes}` | Accepted changes, in order, to everyone (the author gets its own back as confirmation). |
| `Reject {batch_id, reason, fixes: [Change]}` | A batch (or part of it) was invalid: the current values to roll back to. |
| `Presence {client_id, …}` / `PeerJoined` / `PeerLeft` | Other people's cursors, selections and arrivals. |
| `Refused {reason}` | The connection is refused (protocol version, token, session full, a message out of place). |
| `Pong` | Keep-alive. |
| `Admin {ready \| published {file, edit, view} \| files \| links \| link \| done \| failed {reason}}` | A managing connection's answer: the key is right, a published file with its first two links (tokens shown once), listings, a new link, done, or why not. |
| `Library {ready {access} \| library {version, archive} \| unchanged {version} \| empty \| published {version} \| usage [{usage, updated}] \| failed {reason}}` | A library connection's answer: the link is right (with its access), the library (a `.kdesign` archive holding it), no newer version, no library yet, kept, the usage reports, or why not. |
| `Signal {client_id, signal: {kind, text}}` | Someone's cursor chat or spotlight, cleaned by the server; `ready` (about the client itself, after its welcome): signals may be sent. Only to clients whose hello announced `signals`. |
| `Hosting {open, operator, terms, idle_secs, max_file_mb, expires?}` | The answer to `OpenHosting`, and after a welcome to clients whose hello announced `hosting`: anyone may publish, how to reach the operator, the terms (a URL), how long files published without the key are kept unopened, the largest file, and this file's expiry (seconds since 1970). |
| `Admin {hosted {file, edit, view, owner, expires?} \| listing [row] \| reports [report] \| blocked [ip]}` | A file published without the key with its links and owner token (shown once), and the answers to the later managing requests. |
| `Reported {error?}` | The answer to `Report`: kept, or why not. |
| `Plugins {ready {publisher} \| list [entry] \| bundle {entry, bundle} \| published entry \| removed [version] \| failed {reason}}` | A plugin directory connection's answer: opened (with the key or not), the latest version of each plugin with the versions kept, a bundle and its entry (its SHA-256 among them), kept, unpublished, or why not. |
| `Resolved {file, token}` | The answer to `Resolve`: the file the code opens and its link's token, which the client then joins with in an ordinary `Hello` on a new connection (so reconnects, access and revocation are the link's). |
| `Admin {code {code}}` | The answer to `Admin code`: the link's new join code, shown this once (the server keeps only what it needs to recognise it). |
| `Admin {owned [{name, links, expires?} \| null]}` | The answer to `Admin owned`, one entry per pair asked about, in order: the file's name on the server, the ids of its share links (the owner's own and the ones made for others) and its expiry; `null` when that token manages no file of that id (removed, expired, never there, or a wrong token: they read the same). |

`Change` is one of: `Create {id, type, props}`, `Set {id, prop, value}`, `Delete {id}` (deleting a
node deletes its subtree; ids are never allocated twice), plus the document-level `SetDoc`, `Put`
and `Remove` (see above). A snapshot is the list of changes that builds the document from nothing;
`Welcome` carries one, or the missed batches when a client reconnects (see Offline and
reconnect). A whole batch is accepted or rejected.

## Ordering, optimism and convergence

*Built* (`kition_design_collab::{Server, Client}`: pure state machines; the transport only moves
messages).

1. A user action runs locally as today (one command or gesture = one undo step). The client diffs
   the document before and after, sends the `Changes` batch and shows the result immediately.
2. The client keeps its **unconfirmed batches** in order. Changes from others are applied to the
   confirmed state; the client's unconfirmed property values are then re-applied on top, so a
   user never sees their own pending edit flicker away.
3. When the server echoes a batch, it moves from unconfirmed to confirmed. On `Reject`, the client
   drops the rejected values and applies the server's `fixes`.
4. Result: every client's document equals the server's once everyone has caught up.

As built: the server validates each batch as a whole by applying it to a copy (limits on changes
per batch and layers per file; values of the right type; finite, in-range sizes, transforms and
opacity; known nodes; parents that exist and can hold layers; pages only at the top; no cycles; at
least one page; new layer ids from the sender's space; new styles, variables, collections and
comments not in another client's space), then either commits it with the next sequence
number or rejects it with the current values of what it touched as fixes. The client keeps the
confirmed state, applies its own batches at once, and on someone else's batch or a rejection
rebuilds its view as the confirmed state with its pending batches replayed leniently (the fixes
are for clients that keep no confirmed copy). `kition_design_collab::sim` runs a server and N clients
over in-memory links that deliver in order per connection after random delays, through the binary
or the JSON encoding. Tests drive it with synthetic edits and, in the engine, with random real
commands (creating, moving and reparenting layers including cycle attempts, deleting, grouping,
auto layout, components and instances, styles, variables, text, pages, comments): after quiescence
every client's state equals the server's, for every seed (more seeds run as an ignored test).
The same runs with undo, redo and dropped connections are described below.

### Rejections

*Built.* When the server refuses a batch (a layer it changes was deleted by someone else, a move
would make a cycle, a value is out of range…), the `Reject` carries a short reason (a deleted
layer reads "a layer it changes was deleted by someone else") and the current values of what the
batch touched. The client drops the batch, applies the fixes to its confirmed copy (a no-op there:
every batch ordered before the rejection arrived first; they matter to clients that keep no
confirmed copy), rebuilds its view from the confirmed state and its remaining batches, and takes
the batch's step out of the undo history, together with any undo or redo of it still waiting to be
sent. It then queues a notice (`Client::take_notices`, `Notice::message`); the engine shows it as
the UI's toast (`Session::message`).

### Derived data

*Built.* Decided per property:

- **Synced with the authoring change** (the client making an edit runs the engine's normalize pass
  before diffing, so its batch carries consistent values, which receivers apply as they are): auto
  layout positions and sizes of children and hugging frames, text box auto-size (it depends on the
  fonts installed, so it is computed once, never per machine), group and boolean bounds, instance
  contents (copies of their mains with their own ids, overrides respected), values written by
  variable bindings and style links, ready-for-dev "changed" flags and snapshots, comment pins
  following their layers.
- **Repaired after concurrent edits:** when a client's batch is confirmed after someone else's
  batch was ordered before it, that client (only the author) runs the recompute hook over the
  nodes its batch touched, on the confirmed state, and sends any difference as a follow-up batch
  (leaving out properties its own later pending batches write). The engine's hook is its normalize
  pass (`kition_design_engine::collab::EngineRecompute`). Because only the author recomputes, clients
  never fight over a value, even with different fonts; a test checks that a derived invariant
  broken by concurrent edits is restored.
- **Not synced:** each person's selection, viewport, tool and text caret (presence), the id
  counter and client id, and (for now) this file's published library.

## Undo and redo

*Built* (`Client::{undo, redo}`; in the engine, `edit.undo` / `edit.redo` and the UI's undo route
there while the document is in a live session). Outside a live session the engine's snapshot undo
works exactly as before; a document entering or leaving a session starts with an empty history.

Undo stays per user and undoes only **your own** actions: an undo turns your last batch into a new
batch that writes back the values your batch replaced, for the properties nobody has changed since
(a property someone else changed afterwards is left alone). Redo is the inverse. Remote changes
never enter your undo history.

Decisions, as built:

- **A step per batch.** Every undoable batch of the user (one command, one gesture, one
  `command.batch` / `Session::grouped` group) becomes a step: per target (a node property, a
  document-level property or entry) the value before and after, plus the nodes it created and
  deleted, each as the change that creates it as it was. Batches that aren't commands' edits
  (derived-data follow-ups) never become steps.
- **"Nobody has changed it since"** is decided by server order: a batch from someone else that the
  server ordered after the step's last confirmed batch takes the targets it writes away from the
  step for good (a write to a list as a whole takes every entry of that list; creating a node again
  takes all its properties; a taken parent takes the position too). Batches ordered before it are
  what the step replaced: the step learns their values when its batch is confirmed or replayed, so
  an undo restores the value that was there in server order, not the one the user happened to see.
- **Creates become deletes, deletes create the subtree again** with the same ids, positions and
  values (the server accepts re-creating deleted ids). A created layer someone else has since put
  layers into is not deleted (and if nothing else of the step is left, the step is skipped); a
  layer whose parent is gone is not brought back. Changes are checked in order against the current
  state, so an undo is never refused for what the client can see.
- **An undo or redo waits for what came before it.** It shows at once, but goes out only when
  every earlier batch of the client is confirmed or refused, and is then made again from the
  confirmed state. So it never rests on an edit the server refused (the step and its undo are
  dropped with the edit) or on values the client hadn't seen yet. Undoing right after an edit
  costs one round trip before it reaches the others; edits made meanwhile queue behind it.
- **Derived data** that the restored values imply (auto layout, groups…) is recomputed with the
  recompute hook and sent as a follow-up batch, like the repairs after concurrent edits.
- **A step that no longer changes anything is skipped** (the next undo goes one step further).
- Concurrent edits the undoing client couldn't have seen (made while its undo was on the way, or
  while it was offline) are ordered last-writer-wins like any other edits.

Tests: the in-memory simulation checks every undo and redo the server accepts against the log of
accepted batches: a value it changes was last written by its author (or after the client's last
seen sequence number), and every layer it deletes was created by its author or moved into the
deleted layer after that. Random sessions of synthetic edits and of real engine commands, with
undo, redo and dropped connections, converge with no violation (an ignored test runs thousands
of seeds). The engine's tests (its live session with a scripted
link) also cover a gesture staying one step while others edit, a group of commands syncing as one
step, an undo right after an edit waiting for it, a refused edit's toast, a dropped connection
resuming with its offline edits, and leaving the session going back to snapshot undo.

## Presence

*Built* in the engine (`crates/engine/engine/src/sharing/live`): the presence state and the
commands. Nothing in this repository sends it over a network, and the kit's editor draws none of
it.

- Cursor position in page coordinates plus the page id; selection ids; viewport rectangle; whether
  the user is typing in a text layer, with their caret and selection inside it
  (`editing_text {node, caret, selection}`, in characters). The message is the core's `Presence`; the engine builds yours from the active document (page, selection, view,
  edited text) and the pointer over the canvas, and sends it only when it changed.
- **Following.** `collab.follow {client}` follows that person's viewport; `collab.follow` without
  a client stops.
- **Removing someone.** `collab.removePerson {client}` asks the session's host to remove a person
  (`Admin::RemovePeer {file, client}`).
- **Two people in one text layer.** Both edits sync when each person finishes, as whole values:
  the one the server orders last wins, as for any property. `collab.peers` reports who types where
  (`editing {node, layer, caret, selection}` per person, and yours).
- Never stored; dropped when a client disconnects.
- Presence lines up with your canvas while the session's document is in front
  (`Session::live_shared`).
- **Cursor chat.** `collab.cursorChat {text}` sends a `chat` signal (at most 100 characters;
  control characters become spaces); an empty text closes it.
- **Spotlight.** `collab.spotlight {on?}` sends a `spotlight` signal: everyone else follows your
  view and page as with a follow, and someone who moves their own view stops following. A newer
  spotlight takes over. With no accounts, anyone in a session can spotlight.
- **Signals are presence**: never in the document, the log or a saved state; the server keeps only
  who spotlights (for newcomers).

## Sessions in the engine

*Built* in the engine; not reachable in this repository without a `LiveLink`.

- **Commands:** `collab.share {via?: "online" | "local", access?, …}`,
  `collab.host {port?, bind?: "local" | "lan" | <ip>}`, `collab.stop`, `collab.join {code | link}`,
  `collab.leave`, `collab.publish`, `collab.peers`, `collab.follow {client?}`,
  `collab.cursor {x?, y?}`, `collab.cursorChat {text?}`, `collab.spotlight {on?}`,
  `collab.removePerson {client}`, `collab.files`, `collab.openFile`, `collab.forgetFile`,
  `collab.recoverFile`, `collab.manageFile`, `collab.serverInfo`, `collab.report`.
- **Sessions are per document.** In the engine a session is a `Live` in `Session::live` (`Lives`),
  found by its document's uid (`Session::live()` for the active document, `live_of(uid)`); one
  `LiveLink` carries them all, each under the `SessionId` the engine gave it, and every event it
  hands back names its session, so nothing crosses over. Closing a document ends its session only.
- **Join codes** (`kition_design_collab::code`). Both kinds use Crockford's base32 alphabet (digits
  and capitals without `I`, `L`, `O`, `U`); reading one ignores case, spaces and dashes and takes
  `O` as `0` and `I` or `L` as `1`. An online code is nine symbols (45 random bits, shown
  `K7M-2QX-9TD`): a name a server gives one share link. A local network code is 20 symbols (shown
  in fives): the host's IPv4 address, its port, the session's 44-bit secret and 8 check bits that
  catch a mistyped code before anything connects.
- **Online files** (`crates/engine/engine/src/sharing/online.rs`). A file put on a server without
  the server's key has an owner token; the engine records such files with the preferences, never
  in a document, and no command result carries the token (`collab.files` says
  `hasOwnerToken: true`).
- **Document sync.** The engine syncs one open document per session (`Live::doc_uid`). Every command, group of
  commands or gesture on it is diffed against the client's view into one batch
  (`ClientMsg::Changes`) that is one step of the user's undo history (`Session::record`); a gesture
  (a drag, a resize) is sent once, when it ends. Batches the server orders (`Changes`, `Reject`) go through
  `Client::receive` with the engine's recompute hook, and the resulting document is installed with
  `Session::apply_remote`, which keeps your selection, page and view and never adds an undo step.
  Batches that arrive during a gesture wait for it to end. New layers take ids in the client's
  space (`client_id << 32 | counter`). Closing the session's document leaves the session. Revert to
  saved (`file.revert`) is refused while the document belongs to a session; leave the session
  first to revert the local file.

## Offline and reconnect

*Built* (`Client::{disconnect, reconnect}`, the server's log, the engine's live session). On a dropped connection the client keeps
editing locally, queues batches, and on reconnect sends `Hello` with its last confirmed `seq`; the
server replies with the missed changes, then the queued batches are sent (they may be partly
rejected if their objects were deleted meanwhile).

Decisions, as built:

- **Same identity.** `Hello.resume` carries the session id (random per server, kept in its saved
  state), the client id and the last sequence number applied. A client this session gave its id to
  keeps it (a stale connection with that id is taken over); anyone else joins as new. A client
  welcomed as new after all (the server restarted without its state) starts over from the
  snapshot and is told how many unsent changes couldn't be kept.
- **Catch-up.** The server keeps a log of accepted batches (`Limits::max_log_batches`, default
  10 000, and `max_log_changes`, default 1 000 000). If every batch after the client's sequence
  number is still in it, the welcome carries them (`CatchUp::Delta`); otherwise the whole document
  (`CatchUp::Snapshot`).
- **What happened to the batches in flight.** The server remembers per client the last batch id it
  processed and the latest refused ids (at most 1 024). Batches up to that id that the catch-up
  doesn't confirm were refused (the user is told); the others are sent again, in order, keeping
  the sequence number they were made on. Batches the server confirmed but whose echo was lost are
  confirmed by the catch-up and never sent twice.
- **Undo across a gap.** With a delta, the missed batches mark the steps they conflict with exactly
  as they would have live. With a snapshot, which batches came from others isn't known: a step
  keeps a value only if it is still the one the step wrote (someone else setting exactly the same
  value can't be told apart). An undo or redo lost in flight is made again from the caught-up
  state, treating what others wrote meanwhile as later than the step.
- **Session state.** `Server::state` returns what a server needs to carry on after a restart
  (`SessionState`: snapshot, sequence number, session id, client-id counter, the clients that can
  reconnect and what was done with their batches, deleted ids, the log), as one serde value, and
  `Server::journaling` / `Server::replay` give a journal of the changes since.

Tests: random sessions where clients drop and come back mid-session converge, with the default log
and with logs short enough that long absences fall back to snapshots; targeted tests cover a lost
echo, a batch refused while offline, a long absence, an unknown client and a restart from the
saved state.

## Agents

Everything stays a command: the `collab.*` commands are ordinary engine commands. Without a
network link they answer that live sessions aren't available, which is the case everywhere in this
repository: the headless kit executable (`bindings/headless`) and the editor's engine
(`bindings/wasm`) install none and leave the `collab.*` commands out of the kit's command list.

## Security

The core's server validates every change (types, ranges, the same caps the engine applies to
commands, tree rules, message size limits) and never trusts client-computed data. Message decoders
take a size limit and are fuzzed. A transport, a host and their threat model are outside this
repository.

## What is built

1. **Core (no networking):** client-scoped ids, the object/property model over `Document`, diff
   after a command → `Changes`, apply `Changes`, fractional positions, server-side validation and
   ordering as a pure library (`crates/engine/collab`, layer 2: it depends on the document model only;
   the engine depends on it for the recompute hook and `collab.diff`, which shows the batch an
   edit syncs as), and a randomized N-client convergence test with an in-memory transport.
   **Built.**
2. **The engine's live session:** the `collab.*` commands, the live state and the sync client,
   reaching the network only through a `LiveLink`. **Built** in the engine; no transport or host
   in this repository.
3. **Per-user undo**, reconnect and offline queueing, rejection handling, presence for text
   editing. **Built** (the core's client and server, the engine's live session), with
   client-scoped ids for document-level objects.
4. **Accounts**: people signing in, per-person permissions, teams and projects. Not built.
