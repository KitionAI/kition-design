//! The parameters of this kit's commands that name files or folders on disk (KHP A28), declared
//! as typed files in the commands' schemas (`kition_kit::files`): a host lends them, and a
//! headless kit keeps those commands from agents (the host opens, saves and exports documents).

use kition_kit::files::FileParam;

/// Every file or folder parameter of the kit's commands.
pub const FILES: &[FileParam] = &[
    FileParam::read("file.import", "path", &[]),
    // A `.kdesign` published as a library; a share link is not a file and goes through the host.
    FileParam::read("library.add", "path", &["kdesign"]),
];

#[cfg(test)]
mod tests {
    use kition_kit::KitDocument;

    use super::*;

    /// Parameters whose names read like files but are not: ids, property paths, directions,
    /// folders of the kit's own panels, in-memory data.
    const NOT_FILES: &[(&str, &str)] = &[
        // A node path inside the prototype (where the action sits), not a file.
        ("proto.action.set", "path"),
        // The position of an action in an interaction.
        ("proto.action.remove", "path"),
        ("proto.action.move", "path"),
        ("proto.action.add", "path"),
        ("proto.conditional.addBlock", "path"),
        ("proto.conditional.setCondition", "path"),
        ("proto.conditional.removeBlock", "path"),
        // SVG path data.
        ("node.create", "path"),
        ("vector.create", "path"),
        ("board.ink", "path"),
        // The vector or shape layer the text follows.
        ("text.onPath", "path"),
        // A style folder's name ("Brand/Blog").
        ("style.move", "folder"),
        ("style.renameFolder", "folder"),
        // A direction.
        ("comment.next", "dir"),
        ("data.fill", "dir"),
    ];

    #[test]
    fn every_file_parameter_is_declared() {
        let doc = crate::DesignDocument::create("Untitled.kdesign", 64, 64).unwrap_or_else(|error| panic!("{error}"));
        let problems = kition_kit::files::check(&doc.commands(), FILES, NOT_FILES);
        assert!(problems.is_empty(), "{problems:#?}");
    }
}
