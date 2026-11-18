//! Every command's source-scan table and parameter `on` fills (`KitCommand::targets`) match what
//! the engine's code scan reaches (task card
//! C4/C5/C6-design). The engine has no enabled predicate, so each `spec!` command's `run` function (or
//! closure) is followed through the engine's functions and the commands it runs (`execute("…")`)
//! to the accessors of the selection (and `params::targets`, which falls back to it when no ids are
//! given), of the current page, and of the text and vector edit modes and the tool (`mode`); and to
//! the helpers that read a parameter as a layer id. A command that reaches one and does not
//! list it, or lists what the scan never reaches, fails here. The commands the binding adds are
//! declared in `acts.rs` by hand. To see how the scan reached a reading, set
//! `ACTS_DEBUG=<command id>` or `ACTS_FN=<engine function>`; `ACTS_PRINT=1` prints the scan.
//! The independent handler reading (`declarations_independent.rs`, C8-design-read) gates the
//! published editor-state metadata: it corrects both missed calls and unrelated helpers the
//! name-based scan follows. Keeping this scan check separate preserves its original coverage.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use kition_design_kit::DesignDocument;
use kition_kit::KitDocument;

/// Code that reads the selection: the document's selection, and the helpers that default to it.
const SELECTION: &[&str] = &[".selection()", "ds.selection", "d.selection", "self.selection", "ds()?.selection", "targets(", "req_targets("];
/// Code that reads the current page.
const PART: &[&str] = &["s.page()", "self.page()", "k.page()", "ds.page", "d.page", "self.page", "ds()?.page"];
/// Code that reads the edit modes and the tool.
const MODE: &[&str] = &[
    ".text_edit",
    ".vector_edit",
    "tool.tool",
    "tool.dev_mode",
    ".clipboard",
    ".prefs",
    ".view",
    ".rotation_origin",
    ".moved_origin(",
    ".origin_point(",
    ".origin_handle(",
    "s.tool.",
    "self.tool.",
    ".last_command",
    ".last_duplicate",
    ".interaction",
    ".ui_dark",
    ".recent_colors",
];
/// The engine's plumbing every command passes through, which carries the selection and the page
/// along (to restore them on undo, to keep the selection on the page) without a command reading
/// them for what it does.
const PLUMBING: &[&str] = &[
    "execute",
    "execute_command",
    "execute_as",
    "after_command",
    "record",
    "push_undo",
    "sync_all_kdm",
    "set_selection",
    "clean_selection",
    "apply_view_prefs",
    "check_instance_structure",
    "check_preview",
    "check_live_access",
    "check_finite_geometry",
    "grouped",
    "end_preview",
    "mark_saved",
];

fn engine_src() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates/engine/engine/src")
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs")
            && !path.file_name().is_some_and(|n| n == "tests.rs" || n.to_string_lossy().ends_with("_tests.rs"))
        {
            out.push(path);
        }
    }
}

/// `source` with every comment, string and character literal blanked (to spaces), so brackets and
/// names are only the code's; same length, byte for byte.
fn code_only(source: &str) -> Vec<u8> {
    let b = source.as_bytes();
    let mut out = b.to_vec();
    let mut i = 0;
    let blank = |out: &mut Vec<u8>, from: usize, to: usize| {
        for byte in out.iter_mut().take(to).skip(from) {
            *byte = b' ';
        }
    };
    while i < b.len() {
        let ident_before = i > 0 && (b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_');
        if b[i..].starts_with(b"//") {
            let end = b[i..].iter().position(|&c| c == b'\n').map_or(b.len(), |n| i + n);
            blank(&mut out, i, end);
            i = end;
        } else if b[i..].starts_with(b"/*") {
            let end = source[i + 2..].find("*/").map_or(b.len(), |n| i + 2 + n + 2);
            blank(&mut out, i, end);
            i = end;
        } else if !ident_before && (b[i] == b'r' || b[i..].starts_with(b"br")) && {
            let start = if b[i] == b'b' { i + 2 } else { i + 1 };
            let hashes = b[start..].iter().take_while(|&&c| c == b'#').count();
            b.get(start + hashes) == Some(&b'"')
        } {
            let start = if b[i] == b'b' { i + 2 } else { i + 1 };
            let hashes = b[start..].iter().take_while(|&&c| c == b'#').count();
            let close = format!("\"{}", "#".repeat(hashes));
            let body = start + hashes + 1;
            let end = source[body..].find(&close).map_or(b.len(), |n| body + n + close.len());
            blank(&mut out, i, end);
            i = end;
        } else if b[i] == b'"' {
            let mut j = i + 1;
            while j < b.len() && b[j] != b'"' {
                j += if b[j] == b'\\' { 2 } else { 1 };
            }
            blank(&mut out, i, (j + 1).min(b.len()));
            i = j + 1;
        } else if b[i] == b'\'' {
            // A character literal ('a', '\n', '{', 'é'), not a lifetime ('a).
            let rest = &source[i + 1..];
            let len = if let Some(escaped) = rest.strip_prefix('\\') {
                escaped.find('\'').map(|n| n + 2)
            } else {
                rest.chars().next().map(char::len_utf8).filter(|&n| rest[n..].starts_with('\''))
            };
            match len {
                Some(len) => {
                    blank(&mut out, i, i + len + 2);
                    i += len + 2;
                }
                None => i += 1,
            }
        } else {
            i += 1;
        }
    }
    out
}

/// The position after the bracket matching the one at `open`.
fn matching(code: &[u8], open: usize) -> usize {
    let (left, right) = match code[open] {
        b'(' => (b'(', b')'),
        b'{' => (b'{', b'}'),
        _ => (b'[', b']'),
    };
    let mut depth = 0usize;
    for (at, &c) in code.iter().enumerate().skip(open) {
        if c == left {
            depth += 1;
        } else if c == right {
            depth -= 1;
            if depth == 0 {
                return at + 1;
            }
        }
    }
    code.len()
}

/// The source cut before its `#[cfg(test)]` module, the code of what is left, and its file.
struct File {
    source: String,
    code: Vec<u8>,
    name: String,
}

fn files() -> Vec<File> {
    let mut paths = Vec::new();
    rust_files(&engine_src(), &mut paths);
    paths.sort();
    paths
        .into_iter()
        .map(|path| {
            let mut source = std::fs::read_to_string(&path).unwrap();
            // An inline test module (`#[cfg(test)] mod tests { … }`) ends the shipped code; an
            // out-of-line one (`mod tests;`) is another file, left out with the tests.
            let inline = source
                .match_indices("#[cfg(test)]\nmod ")
                .map(|(at, _)| at)
                .find(|at| source[*at..].lines().nth(1).is_some_and(|line| line.trim_end().ends_with('{')));
            if let Some(at) = inline {
                source.truncate(at);
            }
            let code = code_only(&source);
            File { source, code, name: path.to_string_lossy().into_owned() }
        })
        .collect()
}

fn ident_at(code: &[u8], at: usize) -> &str {
    let end = code[at..].iter().position(|c| !(c.is_ascii_alphanumeric() || *c == b'_')).map_or(code.len(), |n| at + n);
    std::str::from_utf8(&code[at..end]).unwrap_or("")
}

/// What a piece of code reads directly, the parameters it reads as layer ids, the free functions
/// it calls, and the commands it runs.
#[derive(Default, Clone)]
struct Reach {
    reads: BTreeSet<&'static str>,
    ids: BTreeSet<String>,
    calls: BTreeSet<String>,
    runs: BTreeSet<String>,
}

/// The parameters `source` reads as layer ids: `req_id(_, p, "key")`, `opt_id(_, p, "key")`,
/// `id(p, "key")`, and `ids` and `id` through `targets`, `req_targets` and `targets_param`.
fn id_keys(source: &str, code: &[u8]) -> BTreeSet<String> {
    let mut keys = BTreeSet::new();
    let text = std::str::from_utf8(code).unwrap_or("");
    for helper in ["targets(", "req_targets(", "targets_param("] {
        if text.match_indices(helper).any(|(at, _)| at == 0 || !(code[at - 1].is_ascii_alphanumeric() || code[at - 1] == b'_')) {
            keys.insert("ids".to_owned());
            keys.insert("id".to_owned());
        }
    }
    for (at, _) in text.match_indices("id(") {
        let start = if text[..at].ends_with("req_") || text[..at].ends_with("opt_") { at - 4 } else { at };
        if start > 0 && (code[start - 1].is_ascii_alphanumeric() || code[start - 1] == b'_' || code[start - 1] == b'.') {
            continue;
        }
        let close = matching(code, at + 2);
        let args = &source[at + 3..close.saturating_sub(1).max(at + 3)];
        if let Some(key) = args.rsplit(',').next().map(str::trim).and_then(|last| last.strip_prefix('"')).and_then(|last| last.strip_suffix('"')) {
            keys.insert(key.to_owned());
        }
    }
    keys
}

fn reach(code: &[u8], source: &str) -> Reach {
    let text = std::str::from_utf8(code).unwrap_or("");
    let mut found = Reach::default();
    let has = |token: &str| {
        text.match_indices(token).any(|(at, _)| {
            let end = at + token.len();
            let ident = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
            if token.as_bytes().first().is_some_and(|c| ident(*c)) && at > 0 && ident(code[at - 1]) {
                return false;
            }
            let next = code.get(end).copied().unwrap_or(b' ');
            let whole = token.ends_with('(') || token.ends_with(')') || !(next.is_ascii_alphanumeric() || next == b'_');
            // Setting it (`ds.page = pg`, `ds.selection.clear()`, `ds.text_edit.take()`) is no read.
            let rest = text[end..].trim_start();
            let written = (rest.starts_with('=') && !rest.starts_with("==") && !rest.starts_with("=>"))
                || [".clear()", ".push(", ".retain(", ".extend(", ".truncate(", ".dedup(", ".take()"].iter().any(|write| rest.starts_with(write));
            whole && !written
        })
    };
    if SELECTION.iter().any(|token| has(token)) {
        found.reads.insert("selection");
    }
    if PART.iter().any(|token| has(token)) {
        found.reads.insert("part");
    }
    if MODE.iter().any(|token| has(token)) {
        found.reads.insert("mode");
    }
    found.ids = id_keys(source, code);
    for at in 0..code.len() {
        let starts =
            (code[at].is_ascii_alphabetic() || code[at] == b'_') && (at == 0 || !(code[at - 1].is_ascii_alphanumeric() || code[at - 1] == b'_'));
        if !starts {
            continue;
        }
        let name = ident_at(code, at);
        let after = at + name.len();
        let method = at > 0 && code[at - 1] == b'.';
        if code.get(after) == Some(&b'(') && !method && in_engine(code, at) {
            found.calls.insert(name.to_owned());
        }
        // `s.execute("arrange.align", …)`: the command runs another, and reads what it reads.
        if name == "execute" && code.get(after) == Some(&b'(') {
            let rest = source[after + 1..].trim_start();
            if let Some(stripped) = rest.strip_prefix('"')
                && let Some(end) = stripped.find('"')
            {
                found.runs.insert(stripped[..end].to_owned());
            }
        }
    }
    found
}

/// The engine's modules: its files' and folders' names.
fn modules() -> &'static BTreeSet<String> {
    static MODULES: std::sync::OnceLock<BTreeSet<String>> = std::sync::OnceLock::new();
    MODULES.get_or_init(|| {
        fn walk(dir: &Path, out: &mut BTreeSet<String>) {
            for entry in std::fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                if let Some(stem) = path.file_stem() {
                    out.insert(stem.to_string_lossy().into_owned());
                }
                if path.is_dir() {
                    walk(&path, out);
                }
            }
        }
        let mut out: BTreeSet<String> = ["crate", "super", "self", "Self"].into_iter().map(str::to_owned).collect();
        walk(&engine_src(), &mut out);
        out
    })
}

/// Whether the call at `at` is to the engine's own code: unqualified, or through `crate`, `super`,
/// `self`, `Self` or one of its modules; not `Arc::new`, `std::fs::rename` or another crate's.
fn in_engine(code: &[u8], at: usize) -> bool {
    let mut start = at;
    while start >= 2 && &code[start - 2..start] == b"::" {
        let mut from = start - 2;
        while from > 0 && (code[from - 1].is_ascii_alphanumeric() || code[from - 1] == b'_') {
            from -= 1;
        }
        start = from;
    }
    start == at || modules().contains(ident_at(code, start))
}

/// Every `fn` body, by name (a name defined in several files keeps each, by file).
fn functions(files: &[File]) -> BTreeMap<String, Vec<(String, Reach)>> {
    let mut out: BTreeMap<String, Vec<(String, Reach)>> = BTreeMap::new();
    for file in files {
        let code = &file.code;
        let text = std::str::from_utf8(code).unwrap_or("");
        for (at, _) in text.match_indices("fn ") {
            if at > 0 && (code[at - 1].is_ascii_alphanumeric() || code[at - 1] == b'_') {
                continue;
            }
            let name = ident_at(code, at + 3).to_owned();
            if name.is_empty() {
                continue;
            }
            let Some(open) = code[at..].iter().position(|&c| c == b'(').map(|n| at + n) else { continue };
            let params_end = matching(code, open);
            let Some(brace) = code[params_end..].iter().position(|&c| c == b'{' || c == b';').map(|n| params_end + n) else { continue };
            if code[brace] == b';' {
                continue;
            }
            let end = matching(code, brace);
            let body = if PLUMBING.contains(&name.as_str()) { Reach::default() } else { reach(&code[brace..end], &file.source[brace..end]) };
            out.entry(name).or_default().push((file.name.clone(), body));
        }
    }
    out
}

/// One `spec!(…)` entry: its id, its `run` code, and its file.
struct Spec {
    id: String,
    run: Reach,
    file: String,
}

fn split_args(code: &[u8], from: usize, to: usize) -> Vec<(usize, usize)> {
    let mut args = Vec::new();
    let (mut start, mut at) = (from, from);
    while at < to {
        match code[at] {
            b'(' | b'[' | b'{' => at = matching(code, at),
            b',' => {
                args.push((start, at));
                at += 1;
                start = at;
            }
            _ => at += 1,
        }
    }
    if code[start..to].iter().any(|c| !c.is_ascii_whitespace()) {
        args.push((start, to));
    }
    args
}

fn specs(files: &[File]) -> Vec<Spec> {
    let mut out = Vec::new();
    for file in files {
        let code = &file.code;
        let text = std::str::from_utf8(code).unwrap_or("");
        for (at, _) in text.match_indices("spec!(") {
            if at > 0 && (code[at - 1].is_ascii_alphanumeric() || code[at - 1] == b'_') {
                continue;
            }
            let open = at + 5;
            let end = matching(code, open);
            let args = split_args(code, open + 1, end - 1);
            if args.len() < 5 {
                continue;
            }
            let id_source = &file.source[args[0].0..args[0].1];
            let Some(id) = id_source.split('"').nth(1) else { continue };
            // A closure's `|s, p|` holds a comma: the run is everything after `history`.
            let (run_from, run_to) = (args[4].0, args[args.len() - 1].1);
            let mut run = reach(&code[run_from..run_to], &file.source[run_from..run_to]);
            let name = std::str::from_utf8(&code[run_from..run_to]).unwrap_or("").trim();
            if !name.is_empty() && name.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b':') {
                run.calls.insert(name.rsplit("::").next().unwrap_or(name).to_owned());
            }
            out.push(Spec { id: id.to_owned(), run, file: file.name.clone() });
        }
    }
    out
}

/// What a function reaches, to a fixed point over the calls.
type Reached = (BTreeSet<&'static str>, BTreeSet<String>);

/// What every command reads and the parameters it reads as layer ids, through the functions and
/// commands it reaches.
fn derived() -> BTreeMap<String, Reached> {
    let files = files();
    let functions = functions(&files);
    let specs = specs(&files);
    let mut reached: BTreeMap<(String, String), Reached> = BTreeMap::new();
    for (name, defs) in &functions {
        for (file, body) in defs {
            reached.insert((name.clone(), file.clone()), (body.reads.clone(), body.ids.clone()));
        }
    }
    // A name defined in several files is the caller's own file's when it has one, else every one.
    let resolve = |name: &str, file: &str, reached: &BTreeMap<(String, String), Reached>| -> Reached {
        let Some(defs) = functions.get(name) else { return Reached::default() };
        let own: Vec<_> = defs.iter().filter(|(f, _)| f == file).collect();
        let chosen: Vec<_> = if own.is_empty() { defs.iter().collect() } else { own };
        let mut out = Reached::default();
        for (f, _) in chosen {
            if let Some((reads, ids)) = reached.get(&(name.to_owned(), f.clone())) {
                out.0.extend(reads.iter().copied());
                out.1.extend(ids.iter().cloned());
            }
        }
        out
    };
    loop {
        let mut changed = false;
        for (name, defs) in &functions {
            for (file, body) in defs {
                let mut now = reached.get(&(name.clone(), file.clone())).cloned().unwrap_or_default();
                for call in body.calls.iter().filter(|call| *call != name) {
                    let (reads, ids) = resolve(call, file, &reached);
                    now.0.extend(reads);
                    // Both normalize functions take documents or a key-name string, never
                    // command parameters. Their helper names cannot identify this call's ids.
                    if name != "normalize" {
                        now.1.extend(ids);
                    }
                }
                if reached.get(&(name.clone(), file.clone())) != Some(&now) {
                    reached.insert((name.clone(), file.clone()), now);
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    if let Ok(wanted) = std::env::var("ACTS_FN") {
        for (file, body) in functions.get(&wanted).into_iter().flatten() {
            eprintln!("fn {wanted} in {file}: direct {:?} {:?}", body.reads, body.ids);
            for call in &body.calls {
                eprintln!("  calls {call}: {:?}", resolve(call, file, &reached));
            }
        }
    }
    let mut commands: BTreeMap<String, Reached> = BTreeMap::new();
    for spec in &specs {
        let mut now: Reached = (spec.run.reads.clone(), spec.run.ids.clone());
        for call in &spec.run.calls {
            let (reads, ids) = resolve(call, &spec.file, &reached);
            now.0.extend(reads);
            now.1.extend(ids);
        }
        if std::env::var("ACTS_DEBUG").is_ok_and(|id| id == spec.id) {
            for call in &spec.run.calls {
                eprintln!("{} calls {call}: {:?}", spec.id, resolve(call, &spec.file, &reached));
            }
        }
        // A command defined twice (a board and a design variant) reads what either reads.
        let entry = commands.entry(spec.id.clone()).or_default();
        entry.0.extend(now.0);
        entry.1.extend(now.1);
    }
    // Commands that run other commands read what those read (not their parameters).
    loop {
        let mut changed = false;
        for spec in &specs {
            let mut now = commands[&spec.id].clone();
            for other in &spec.run.runs {
                now.0.extend(commands.get(other).map(|(reads, _)| reads.clone()).unwrap_or_default());
            }
            if commands[&spec.id] != now {
                commands.insert(spec.id.clone(), now);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    commands
}

#[test]
fn every_command_declares_the_editor_state_its_code_reads_and_its_target() {
    let derived = derived();
    let doc = DesignDocument::create("Design.kdesign", 400, 300).unwrap();
    let commands = doc.commands();
    if std::env::var("ACTS_PRINT").is_ok() {
        for command in &commands {
            if let Some((reads, ids)) = derived.get(&command.id) {
                let schema = command.schema.as_ref();
                let target = ["ids", "id"]
                    .into_iter()
                    .find(|key| ids.contains(*key) && schema.is_some_and(|schema| kition_kit::schema::check_targets(schema, key).is_ok()));
                println!("ACTS {} {:?} {:?}", command.id, reads, target);
                println!("IDPATHS {} {:?}", command.id, ids);
            }
        }
    }
    let mut wrong = Vec::new();
    for command in &commands {
        let declared: BTreeSet<&'static str> =
            if derived.contains_key(&command.id) { kition_design_kit::acts::scanned_reads(&command.id) } else { command.reads }
                .iter()
                .map(|state| state.as_str())
                .collect();
        let (expected, target): (BTreeSet<&'static str>, Option<&str>) = match derived.get(&command.id) {
            Some((reads, ids)) => {
                let schema = command.schema.as_ref();
                let target = ["ids", "id"]
                    .into_iter()
                    .find(|key| ids.contains(*key) && schema.is_some_and(|schema| kition_kit::schema::check_targets(schema, key).is_ok()));
                (reads.clone(), target)
            }
            None => {
                let Some((_, state, target, _)) = kition_design_kit::acts::BINDING_COMMANDS.iter().find(|(id, ..)| *id == command.id) else {
                    wrong.push(format!("{}: neither an engine command the scan found nor one acts::BINDING_COMMANDS declares", command.id));
                    continue;
                };
                (state.iter().map(|state| state.as_str()).collect(), *target)
            }
        };
        if declared != expected {
            wrong.push(format!("{}: scanned table {declared:?}, its code scan reaches {expected:?}", command.id));
        }
        let independently_reviewed = kition_design_kit::acts::REVIEWED.iter().any(|(id, _)| *id == command.id)
            || kition_design_kit::acts::READING.iter().any(|(id, _)| *id == command.id);
        if !independently_reviewed {
            let published: BTreeSet<_> = command.reads.iter().map(|state| state.as_str()).collect();
            if published != expected {
                wrong.push(format!("{}: publishes {published:?}, its code scan reaches {expected:?}", command.id));
            }
        }
        if command.targets != target {
            wrong.push(format!("{}: declares targets {:?}, its code reads {target:?} as layer ids", command.id, command.targets));
        }
    }
    assert!(wrong.is_empty(), "{} commands declare other editor state or targets than their code:\n{}", wrong.len(), wrong.join("\n"));
    let listed: BTreeSet<&str> = commands.iter().map(|command| command.id.as_str()).collect();
    for (id, ..) in kition_design_kit::acts::BINDING_COMMANDS {
        assert!(listed.contains(id), "acts::BINDING_COMMANDS names {id}, which the kit does not list");
    }
}

/// The parameter names an agent's `on` fills are what the host's step checker takes (kition-sdk's
/// `check_commands` checks `targets` against each schema), and the counts are as the guide says.
#[test]
fn the_published_commands_pass_the_sdks_checks() {
    let doc = DesignDocument::create("Design.kdesign", 400, 300).unwrap();
    let commands = doc.commands();
    kition_kit::testing::check_commands(&commands, &[]).unwrap_or_else(|problems| panic!("{}", problems.join("\n")));
    assert!(commands.iter().any(|c| c.targets == Some("ids")), "commands taking `ids` take `on`");
    assert!(commands.iter().any(|c| c.reads.contains(&kition_kit::EditorState::Selection) && c.targets.is_some()));
}
