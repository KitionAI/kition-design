# Collaboration parity

> **Last reviewed:** 2026-11-17 · **Last updated:** 2026-11-27 · **Target:** Figma Design (desktop app 126.10.7)

Figma is multiplayer first; Kition Design is local first. **Ready ~13% (estimated):** comments and saved
versions (the host's checkpoints) work inside one file; the engine has a real-time collaboration core (sync model,
server ordering, client reconciliation, convergence tests), and the engine keeps the live-session
state and `collab.*` commands built on it ([`collab-protocol.md`](../features/collab-protocol.md)). This
repository has no network transport, session host or server for those sessions, the kit's bindings
leave the `collab.*` commands out and the editor's Share button is disabled: people co-edit a
design through Kition, which relays the document's KDM replica ([`kdm.md`](../features/kdm.md)). Remaining: 46–92 h
(multiplayer 42–84 h, shared libraries 4–8 h). Branches are cut (owner, 2026-11-23): a person saves a copy of the file instead.

| Feature | Figma | Kition Design | Hours |
|---|---|---|---|
| Real-time co-editing | yes | **core built, no networking** (`crates/engine/collab`): client-scoped ids, per-property change sets with fractional positions, server ordering and validation, optimistic clients, binary messages; randomized multi-client convergence tests over real commands. The engine's live sessions sit on it (`crates/engine/engine/src/sharing/live`, `collab.*` commands): sessions are per document, revert to saved is refused until the document leaves its live session, undo is per person (only your own steps, others' later writes left alone), refused edits roll back, and a reconnect resumes as the same client with what it missed (or a snapshot) and sends what was made offline. Styles, variables, collections, modes and comments made in a session take ids from their creator's space (the server checks it), so concurrent creations all survive; modes, per-mode values and replies inside one entry still resolve last-writer-wins. Nothing in this repository carries those sessions over a network (no transport, session host or server), and the kit's bindings leave `collab.*` out; in Kition a design is co-edited through its KDM replica, which the host relays ([`kdm.md`](../features/kdm.md)) | 25–55 |
| Live cursors, selections, viewports; follow / observation mode; spotlight | yes | **engine state only, not in the kit**: the engine's live sessions keep everyone's cursor, selection, viewport and text caret as presence, follow a person (`collab.follow`) and spotlight (`collab.spotlight`: everyone follows your view and page, people who join later too, anyone who moves their view stops following, a newer spotlight takes over); the kit's bindings leave `collab.*` out and the editor draws none of it ([collab-protocol](../features/collab-protocol.md#presence)). No accounts, so anyone in a session can spotlight | 2–4 |
| Cursor chat, audio, high-fives | yes | **engine state only, not in the kit**: `collab.cursorChat {text}` sends a cursor chat message (100 characters) as ephemeral presence, never stored; the kit leaves it out and the editor shows no chat. No audio, high-fives or emote reactions | 2–5 |
| Share links, permissions (view / edit), teams and projects | yes | **not in the kit**: the engine keeps the join-code and share-link commands (`collab.share`, `collab.join`, `collab.manageFile`, `collab.removePerson`; view-only access refuses edits), but this repository has no session host or server to answer them, the kit's bindings leave `collab.*` out, and the editor's Share button is disabled with a note that sharing happens in Kition. No accounts, teams or projects | 8–17 |
| Comments: pins on frames and layers, threads, reactions, @mentions, resolve, unread, search and filters, links, in the presenter | yes (shared) | **done, local**: stored in the file; a behaviour suite from the documentation (pinning and following layers, threads, own edits and deletes, mentions, reactions, C and Shift+C, stepping, filters, saving, copy and paste); restoring a version keeps them. In Kition, comment threads are part of the design's KDM replica (`CommentThread` and `Comment`, [`kdm.md`](../features/kdm.md)), so concurrent replies to one thread are both kept | (shared once multiplayer lands) |
| Version history: named versions, autosave checkpoints, view before restore, restore, duplicate | yes (cloud) | **done through the host**: saved versions are the host's checkpoints (Kit UI protocol 7m), never in the `.kdesign` or the replica; the panel saves, lists (newest first, a page at a time), renames, deletes, views and restores them (a restore is one undoable change everyone gets); the host's own unlabelled checkpoints of its saves stand for autosaves. A host without checkpoints keeps none (the panel says so). Escape closes the panel once the canvas has nothing to escape (`web/tests/hand/keys.mjs`). No duplicate of a version into a new file and no comparing two versions (Dev Mode compares a frame with a saved version) | — |
| Branching and merging | yes (paid plans) | **cut** (owner, 2026-11-23): save a copy of the file to try changes apart | — |
| Team / organization libraries | yes | **partial**: a design uses the components, styles and variables another `.kdesign` file of the workspace publishes; the host lends the library files to the engine (`bindings/wasm/src/libraries.rs`), updates are accepted with overrides kept, and a library can be swapped for another by names ([components-parity](components-parity.md)). No team-wide library list, accounts or analytics over time | 4–8 |
| Notifications, activity | yes | no | with sharing |

## Revision history

| Date | Change | Summary |
|---|---|---|
| 2026-11-17 | major | First public version |
