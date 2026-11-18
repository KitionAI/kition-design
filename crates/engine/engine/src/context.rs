//! The editor's state besides the document, for an explicit call context (Kit Process Protocol
//! section 7a; task card C4/C5/C6-design): what a command may read besides its parameters (the
//! current page, the selection on it, the view, the text and vector edit modes, the tool), taken
//! and put back without an edit, and set for one call. None of it moves the revision, marks the
//! document dirty or joins an undo history.

use crate::cmd::edit::Duplicated;
use crate::rotation_origin::RotationOrigin;
use crate::vectoredit::VectorEdit;
use crate::{Clipboard, EngineError, Interaction, Prefs, Result, Session, TextEdit, ViewState};
use kition_design_doc::NodeId;

/// A snapshot of the editor's state ([`Session::editor_state`]).
#[derive(Clone, Debug)]
pub struct EditorState {
    /// The current page.
    pub page: NodeId,
    /// The selected layers (on the current page).
    pub selection: Vec<NodeId>,
    /// The canvas view: centre, zoom, viewport and the View menu's toggles.
    pub view: ViewState,
    /// The text being edited, and the vector in vector edit mode.
    pub text_edit: Option<TextEdit>,
    pub vector_edit: Option<VectorEdit>,
    /// The hovered layer, the Dev Mode focus and the ruler guide clicked last.
    pub hover: Option<NodeId>,
    pub focus: Option<NodeId>,
    pub guide: Option<(NodeId, usize)>,
    /// The tool and Dev Mode's tool state.
    pub tool: crate::tools::ToolState,
    pub clipboard: Clipboard,
    pub prefs: Prefs,
    pub ui_dark: Option<bool>,
    pub last_command: Option<(String, serde_json::Value)>,
    pub recent_colors: Vec<kition_design_color::Color>,
    pub interaction: Option<Interaction>,
    view_from_prefs: bool,
    rotation_origin: Option<RotationOrigin>,
    last_duplicate: Option<Duplicated>,
}

impl EditorState {
    /// Checks that every active part, target and mode of the state names something `doc` holds:
    /// the current page, each selected layer on that page, the layers of the text and vector edit
    /// modes, the Dev Mode focus and the clicked ruler guide's holder. A state taken on another
    /// copy of the design (a freshly read file, a planning copy) is restored only when this
    /// answers `Ok`: what is gone is refused with its reason, never replaced by a default. The
    /// hovered layer is pointer feedback, not a target, and is left to
    /// [`Session::restore_editor_state`]. A gesture in progress holds a document of its own and
    /// never moves to another copy.
    pub fn resolves_on(&self, doc: &kition_design_doc::Document) -> std::result::Result<(), String> {
        if !doc.pages.contains(&self.page) {
            return Err(format!("deleted active page {}", self.page));
        }
        let on_page = |id: NodeId| doc.contains(id) && doc.page_of(id) == Some(self.page) && id != self.page;
        for id in &self.selection {
            if !doc.contains(*id) {
                return Err(format!("deleted active target {id}"));
            }
            if !on_page(*id) {
                return Err(format!("relocated active target {id}: it is no longer on page {}", self.page));
            }
        }
        if let Some(edit) = &self.text_edit
            && !doc.contains(edit.id)
        {
            return Err(format!("deleted active text edit target {}", edit.id));
        }
        if let Some(edit) = &self.vector_edit {
            if !doc.contains(edit.id) {
                return Err(format!("deleted active vector edit target {}", edit.id));
            }
            if !on_page(edit.id) {
                return Err(format!("relocated active vector edit target {}: it is no longer on page {}", edit.id, self.page));
            }
        }
        if let Some(focus) = self.focus
            && !doc.contains(focus)
        {
            return Err(format!("deleted active Dev Mode focus {focus}"));
        }
        if let Some((holder, _)) = self.guide
            && !doc.contains(holder)
        {
            return Err(format!("deleted active guide holder {holder}"));
        }
        if let Some(interaction) = &self.interaction {
            return Err(format!("the gesture `{}` is in progress on the previous copy of the design", interaction.label));
        }
        Ok(())
    }
}

impl Session {
    /// The editor's state now; `None` without a document.
    pub fn editor_state(&self) -> Option<EditorState> {
        let ds = self.ds().ok()?;
        Some(EditorState {
            page: ds.page,
            selection: ds.selection.clone(),
            view: ds.view.clone(),
            text_edit: ds.text_edit.clone(),
            vector_edit: ds.vector_edit.clone(),
            hover: ds.hover,
            focus: ds.focus,
            guide: ds.guide,
            tool: self.tool.clone(),
            clipboard: self.clipboard.clone(),
            prefs: self.prefs.clone(),
            ui_dark: self.ui_dark,
            last_command: self.last_command.clone(),
            recent_colors: self.recent_colors.clone(),
            interaction: ds.interaction.clone(),
            view_from_prefs: ds.view_from_prefs,
            rotation_origin: ds.rotation_origin.clone(),
            last_duplicate: ds.last_duplicate.clone(),
        })
    }

    /// Puts `state` back. What the document no longer has is dropped: a deleted page makes the
    /// first page current, deleted layers leave the selection, an edit mode on a deleted layer
    /// ends. No edit: the revision, the dirty state and the undo history stay as they are.
    pub fn restore_editor_state(&mut self, state: &EditorState) -> Result<()> {
        let ds = self.ds_mut()?;
        let doc = ds.doc.clone();
        let page = if doc.pages.contains(&state.page) { state.page } else { doc.first_page().ok_or(EngineError::NoDocument)? };
        let alive = |id: &NodeId| doc.contains(*id);
        ds.page = page;
        ds.selection = state.selection.iter().copied().filter(|id| alive(id) && doc.page_of(*id) == Some(page) && *id != page).collect();
        ds.view = state.view.clone();
        ds.text_edit = state.text_edit.clone().filter(|edit| alive(&edit.id));
        ds.vector_edit = state.vector_edit.clone().filter(|edit| alive(&edit.id) && doc.page_of(edit.id) == Some(page));
        ds.hover = state.hover.filter(alive);
        ds.focus = state.focus.filter(alive);
        ds.guide = state.guide.filter(|(holder, _)| alive(holder));
        ds.rotation_origin = state.rotation_origin.clone();
        ds.last_duplicate = state.last_duplicate.clone();
        ds.interaction = state.interaction.clone();
        ds.view_from_prefs = state.view_from_prefs;
        self.tool = state.tool.clone();
        self.clipboard = state.clipboard.clone();
        self.prefs = state.prefs.clone();
        self.ui_dark = state.ui_dark;
        self.last_command = state.last_command.clone();
        self.recent_colors = state.recent_colors.clone();
        Ok(())
    }

    /// Makes `page` current for one call, and `selection` (layers on it) the selection; `None`
    /// leaves each as it is. The caller checks the ids first. A page change ends the edit modes
    /// and the hover of the page left, as going to a page does. No edit.
    pub fn set_call_context(&mut self, page: Option<NodeId>, selection: Option<Vec<NodeId>>) -> Result<()> {
        let ds = self.ds_mut()?;
        if let Some(page) = page
            && page != ds.page
        {
            if !ds.doc.pages.contains(&page) {
                return Err(EngineError::NoNode(page));
            }
            ds.page = page;
            ds.selection.clear();
            ds.text_edit = None;
            ds.vector_edit = None;
            ds.hover = None;
            ds.focus = None;
            ds.rotation_origin = None;
        }
        if let Some(selection) = selection {
            ds.selection = selection;
            ds.rotation_origin = None;
        }
        Ok(())
    }
}
