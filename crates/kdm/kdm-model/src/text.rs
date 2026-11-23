//! A text layer's characters as one rich `text` field (`Node.text`, `docs/features/kdm.md`
//! section 2.4), after kition-layout's stories.
//!
//! - **Characters.** The field holds the engine's characters plus one final `\n`: every paragraph,
//!   the last included, ends with a break that carries its paragraph mark. The bridge adds the
//!   final break when the layer becomes a text and never writes over it: its edits are computed on
//!   the engine's characters, so every index stays before it. A field without it (an agent's
//!   batch deleted it) reads whole as the characters, the last paragraph with no mark.
//! - **Character marks** ([`CHAR_MARKS`]): each character carries every set field of its style as
//!   the engine's serde form has it, one mark per field, all `expand: "after"`, so two people
//!   formatting overlapping ranges with different fields both win. The values are absolute (never
//!   relative to the layer's base style), so a character reads the same whatever happens to the
//!   base: a field the engine leaves unset (no fills of its own, no link) is no mark.
//! - **Paragraph marks:** a paragraph's list settings (`ParaList`) other than the plain ones are
//!   the `list` mark on its break, `expand: "none"`: typing never extends it.
//! - The layer's base style (`TextData.style`, what an empty text and new text take), its default
//!   list style and its box settings stay in `Node.kind`.

use std::collections::BTreeMap;

use kition_design_doc::{ParaList, StyleRun, TextData, TextStyle};
use kition_kit::session::TextRun;
use serde_json::{Map, Value};

use crate::{KdmResult, kdm_err};

/// The `TextStyle` fields (serde keys) and the character mark each one is.
pub const CHAR_MARKS: &[(&str, &str)] = &[
    ("font_family", "fontFamily"),
    ("font_style", "fontStyle"),
    ("font_size", "fontSize"),
    ("line_height", "lineHeight"),
    ("letter_spacing", "letterSpacing"),
    ("decoration", "decoration"),
    ("decoration_look", "decorationLook"),
    ("case", "case"),
    ("fills", "fills"),
    ("link", "link"),
    ("variations", "variations"),
    ("style_id", "styleId"),
];

/// The paragraph mark: a paragraph's list settings, on its break.
pub const LIST: &str = "list";

/// The keys of a text's serde form (`TextData`) the `text` field holds; the rest is `kind`'s.
pub const TEXT_KEYS: &[&str] = &["characters", "runs", "paragraphs"];

type Marks = BTreeMap<String, Value>;

/// A style's character marks: every field its serde form sets (null is unset).
pub fn char_marks(style: &TextStyle) -> KdmResult<Marks> {
    let Value::Object(map) = serde_json::to_value(style).map_err(kdm_err)? else {
        return Err(kdm_err("a text style is not an object"));
    };
    let mut out = Marks::new();
    for (key, mark) in CHAR_MARKS {
        if let Some(v) = map.get(*key).filter(|v| !v.is_null()) {
            out.insert((*mark).to_owned(), v.clone());
        }
    }
    Ok(out)
}

/// The style a character's marks give (other keys are ignored). A field whose mark does not read
/// (a peer wrote a value of another shape) is left at its default, the others kept: the result
/// depends on the marks alone.
pub fn style_from(marks: &Marks) -> TextStyle {
    let mut map = Map::new();
    for (key, mark) in CHAR_MARKS {
        if let Some(v) = marks.get(*mark).filter(|v| !v.is_null()) {
            map.insert((*key).to_owned(), v.clone());
        }
    }
    if let Ok(style) = serde_json::from_value(Value::Object(map.clone())) {
        return style;
    }
    let mut kept = Map::new();
    for (key, _) in CHAR_MARKS {
        let Some(v) = map.get(*key) else { continue };
        kept.insert((*key).to_owned(), v.clone());
        if serde_json::from_value::<TextStyle>(Value::Object(kept.clone())).is_err() {
            kept.remove(*key);
        }
    }
    serde_json::from_value(Value::Object(kept)).unwrap_or_default()
}

/// A paragraph's mark: its list settings unless they are the plain ones.
fn para_mark(para: ParaList) -> KdmResult<Option<Value>> {
    if para == ParaList::default() { Ok(None) } else { serde_json::to_value(para).map(Some).map_err(kdm_err) }
}

/// Builds runs, merging adjacent text with the same marks.
#[derive(Default)]
struct Runs {
    out: Vec<TextRun>,
}

impl Runs {
    fn push(&mut self, text: &str, attributes: &Marks) {
        if text.is_empty() {
            return;
        }
        if let Some(last) = self.out.last_mut()
            && last.attributes == *attributes
        {
            last.insert.push_str(text);
            return;
        }
        self.out.push(TextRun { insert: text.to_owned(), attributes: attributes.clone() });
    }
}

/// The engine's characters of `t` as runs, without the final break ([`final_mark`] is its mark).
pub fn runs_of(t: &TextData) -> KdmResult<Vec<TextRun>> {
    let text = &t.characters;
    let bytes: Vec<usize> = text.char_indices().map(|(b, _)| b).chain(std::iter::once(text.len())).collect();
    let mut runs = Runs::default();
    let mut para = 0usize;
    let mut cache: Vec<(TextStyle, Marks)> = Vec::new();
    for (start, end, style) in t.spans() {
        let marks = match cache.iter().find(|(s, _)| s == style) {
            Some((_, m)) => m.clone(),
            None => {
                let m = char_marks(style)?;
                cache.push((style.clone(), m.clone()));
                m
            }
        };
        let (Some(&from), Some(&to)) = (bytes.get(start), bytes.get(end)) else { continue };
        let Some(segment) = text.get(from..to) else { continue };
        for piece in segment.split_inclusive('\n') {
            match piece.strip_suffix('\n') {
                Some(body) => {
                    runs.push(body, &marks);
                    let mut at_break = marks.clone();
                    if let Some(v) = para_mark(t.para_list(para))? {
                        at_break.insert(LIST.to_owned(), v);
                    }
                    runs.push("\n", &at_break);
                    para += 1;
                }
                None => runs.push(piece, &marks),
            }
        }
    }
    Ok(runs.out)
}

/// The final break's marks: the last paragraph's list settings, if they are not the plain ones.
pub fn final_mark(t: &TextData) -> KdmResult<Option<Value>> {
    para_mark(t.para_list(t.paragraph_count().saturating_sub(1)))
}

/// The whole field as `t` makes it: [`runs_of`] and the final break with [`final_mark`].
pub fn field_runs(t: &TextData) -> KdmResult<Vec<TextRun>> {
    let mut runs = runs_of(t)?;
    let mut last = TextRun::new("\n");
    if let Some(v) = final_mark(t)? {
        last.attributes.insert(LIST.to_owned(), v);
    }
    runs.push(last);
    Ok(runs)
}

/// Puts a `text` field's runs into `t` (whose base style and list style came from `kind`): its
/// characters, the style runs that differ from the base, and the paragraphs' list settings (none
/// when every paragraph has the default list at level 0, as the engine keeps them).
pub fn apply_runs(t: &mut TextData, runs: &[TextRun]) {
    let full_len: usize = runs.iter().map(|r| r.insert.chars().count()).sum();
    let has_final = runs.last().is_some_and(|r| r.insert.ends_with('\n'));
    let body_len = if has_final { full_len - 1 } else { full_len };
    let mut characters = String::new();
    let mut style_runs: Vec<StyleRun> = Vec::new();
    let mut paras: Vec<ParaList> = Vec::new();
    let mut at = 0usize;
    let mut cache: Vec<(Marks, TextStyle)> = Vec::new();
    for run in runs {
        let style = match cache.iter().find(|(m, _)| *m == run.attributes) {
            Some((_, s)) => s.clone(),
            None => {
                let s = style_from(&run.attributes);
                cache.push((run.attributes.clone(), s.clone()));
                s
            }
        };
        let breaks = run.insert.matches('\n').count();
        if breaks > 0 {
            let para = run.attributes.get(LIST).and_then(|v| serde_json::from_value::<ParaList>(v.clone()).ok()).unwrap_or_default();
            paras.extend(std::iter::repeat_n(para, breaks));
        }
        let len = run.insert.chars().count();
        let take = len.min(body_len.saturating_sub(at));
        if take > 0 {
            characters.extend(run.insert.chars().take(take));
            if style != t.style {
                match style_runs.last_mut() {
                    Some(last) if last.end == at && last.style == style => last.end = at + take,
                    _ => style_runs.push(StyleRun { start: at, end: at + take, style }),
                }
            }
        }
        at += len;
    }
    t.characters = characters;
    t.runs = style_runs;
    let count = t.paragraph_count();
    paras.resize(count, ParaList::default());
    let plain = ParaList { list: t.list, ..ParaList::default() };
    t.paragraphs = if paras.iter().all(|p| *p == plain) { Vec::new() } else { paras };
}

/// `t` as the replica holds it: the same characters, styles and paragraphs in the bridge's one form
/// (runs only where the style differs from the base, merged; paragraphs spelled out unless all
/// are plain), for comparing a view with the engine's document.
pub fn canonical(t: &TextData) -> TextData {
    let mut out = t.clone();
    if let Ok(runs) = field_runs(t) {
        apply_runs(&mut out, &runs);
    }
    out
}

/// Whether two texts give the same field (characters, styles, paragraphs).
pub fn same_field(a: &TextData, b: &TextData) -> bool {
    a.characters == b.characters && a.style == b.style && a.runs == b.runs && a.paragraphs == b.paragraphs && a.list == b.list
}

#[cfg(test)]
mod tests {
    use super::*;
    use kition_design_doc::ListStyle;

    fn text(chars: &str) -> TextData {
        TextData::new(chars)
    }

    #[test]
    fn every_style_field_is_one_mark() {
        let mut s =
            TextStyle { fills: Some(Vec::new()), link: Some("https://example.com".into()), style_id: Some("S:1".into()), ..TextStyle::default() };
        s.variations.insert("wght".into(), 550.0);
        s.set_look(|l| l.skip_ink = !l.skip_ink);
        let Value::Object(map) = serde_json::to_value(&s).unwrap() else { panic!() };
        let keys: Vec<&str> = map.keys().map(String::as_str).collect();
        let mut declared: Vec<&str> = CHAR_MARKS.iter().map(|(k, _)| *k).collect();
        declared.sort_unstable();
        let mut keys = keys;
        keys.sort_unstable();
        assert_eq!(keys, declared, "every TextStyle field has a mark");
        assert_eq!(style_from(&char_marks(&s).unwrap()), s);
    }

    #[test]
    fn the_schema_declares_every_mark() {
        let schema: Value = serde_json::from_str(crate::SCHEMA_JSON).unwrap();
        let marks = schema["types"]["Node"]["fields"]["text"]["marks"].as_object().cloned().unwrap();
        for (_, mark) in CHAR_MARKS {
            assert_eq!(marks.get(*mark).and_then(|m| m["expand"].as_str()), Some("after"), "{mark}");
        }
        assert_eq!(marks.get(LIST).and_then(|m| m["expand"].as_str()), Some("none"));
        assert_eq!(marks.len(), CHAR_MARKS.len() + 1);
    }

    #[test]
    fn a_text_reads_back_with_its_runs_and_paragraphs() {
        let mut t = text("Hello world\nsecond\n\nlast");
        t.restyle(6, 11, |s| s.font_style = "Bold".into());
        t.restyle(12, 14, |s| s.font_size = 30.0);
        t.set_para_lists(12, 12, |p| {
            p.list = ListStyle::Bulleted;
            p.indent = 1;
        });
        let runs = field_runs(&t).unwrap();
        assert!(runs.last().unwrap().insert.ends_with('\n'));
        let mut back = TextData { characters: String::new(), runs: Vec::new(), paragraphs: Vec::new(), ..t.clone() };
        apply_runs(&mut back, &runs);
        assert_eq!(back.characters, t.characters);
        assert_eq!(back.runs, t.runs);
        assert_eq!((0..4).map(|p| back.para_list(p)).collect::<Vec<_>>(), (0..4).map(|p| t.para_list(p)).collect::<Vec<_>>());
    }

    #[test]
    fn the_last_paragraph_keeps_its_list_on_the_final_break() {
        let mut t = text("a\nb");
        t.set_para_lists(2, 2, |p| p.list = ListStyle::Numbered);
        let runs = field_runs(&t).unwrap();
        let last = runs.last().unwrap();
        assert_eq!(last.insert, "\n");
        assert!(last.attributes.contains_key(LIST));
        let mut back = TextData { characters: String::new(), paragraphs: Vec::new(), ..t.clone() };
        apply_runs(&mut back, &runs);
        assert_eq!(back.para_list(1).list, ListStyle::Numbered);
        assert_eq!(back.para_list(0).list, ListStyle::None);
    }

    #[test]
    fn an_empty_text_is_one_break() {
        let t = text("");
        let runs = field_runs(&t).unwrap();
        assert_eq!(runs, vec![TextRun::new("\n")]);
        let mut back = t.clone();
        apply_runs(&mut back, &runs);
        assert_eq!(back, t);
    }

    #[test]
    fn a_text_without_its_final_break_reads_whole() {
        let mut t = text("");
        apply_runs(&mut t, &[TextRun::new("ab\ncd")]);
        assert_eq!(t.characters, "ab\ncd");
        assert!(t.runs.is_empty());
    }

    #[test]
    fn marks_are_absolute_so_a_base_change_leaves_marked_characters_alone() {
        // The base style is `kind`'s: characters carry their whole style, never a difference.
        let t = text("ab");
        let runs = runs_of(&t).unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].attributes.get("fontSize"), Some(&serde_json::json!(12.0)));
        let mut bigger = t.clone();
        bigger.style.font_size = 20.0;
        apply_runs(&mut bigger, &runs);
        assert_eq!(bigger.style_at(0).font_size, 12.0);
        assert_eq!(bigger.runs.len(), 1, "the characters keep their size; the base is 20");
    }

    #[test]
    fn canonical_form_is_stable() {
        let mut t = text("abc\ndef");
        t.restyle(1, 2, |s| s.font_size = 40.0);
        let c = canonical(&t);
        assert_eq!(canonical(&c), c);
        assert_eq!((0..7).map(|i| c.style_at(i).clone()).collect::<Vec<_>>(), (0..7).map(|i| t.style_at(i).clone()).collect::<Vec<_>>());
    }
}
