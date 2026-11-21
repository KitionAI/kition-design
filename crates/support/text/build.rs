//! Embeds the optional kition-fonts build input (`KITION_FONTS_DIR`): both `fonts/manifest.txt` and the
//! opt-in Latin presentation fonts in `fonts/latin-manifest.txt` (Inter, Roboto, …). Unset or missing,
//! `KITION_FONTS` is empty and native apps use system fonts. Required web releases enforce Inter
//! Regular/SemiBold and Noto Sans Georgian Regular after filtering. Recipe: kition-fonts `docs/integration.md`.
//! Noto Sans CJK only with `KITION_DESIGN_EMBED_CJK` set (the web build's only way to show CJK UI text).
use std::fmt::Write as _;
use std::path::PathBuf;

/// Font build script: emit `kition_fonts.rs` from `input_fonts`, passing the
/// `KITION_FONTS_DIR`, `KITION_FONTS_REQUIRED`, wasm target and `KITION_DESIGN_EMBED_CJK` inputs;
/// report generation and write failures with `cargo::error`.
fn main() {
    println!("cargo::rerun-if-env-changed=KITION_FONTS_DIR");
    println!("cargo::rerun-if-env-changed=KITION_FONTS_REQUIRED");
    println!("cargo::rerun-if-env-changed=KITION_DESIGN_EMBED_CJK");
    let mut src = String::from("pub static KITION_FONTS: &[KitionFont] = &[\n");
    let dir = std::env::var_os("KITION_FONTS_DIR").map(PathBuf::from);
    let required = std::env::var_os("KITION_FONTS_REQUIRED").is_some();
    let wasm = std::env::var("CARGO_CFG_TARGET_ARCH").is_ok_and(|a| a == "wasm32");
    let embed_cjk = std::env::var_os("KITION_DESIGN_EMBED_CJK").is_some();
    match input_fonts(dir.as_deref(), wasm, required, embed_cjk) {
        Ok(entries) => src.push_str(&entries),
        Err(e) => println!("cargo::error={e}"),
    }
    src.push_str("];\n");
    let out = PathBuf::from(std::env::var_os("OUT_DIR").unwrap_or_default()).join("kition_fonts.rs");
    if let Err(e) = std::fs::write(&out, src) {
        println!("cargo::error=writing {}: {e}", out.display());
    }
}

/// Collect the effective catalog and enforce the required web release faces after filtering.
fn input_fonts(dir: Option<&std::path::Path>, wasm: bool, required: bool, embed_cjk: bool) -> Result<String, String> {
    let Some(dir) = dir else {
        return if required { Err("KITION_FONTS_REQUIRED is set but KITION_FONTS_DIR is missing".into()) } else { Ok(String::new()) };
    };
    let mut out = String::new();
    let mut faces = Vec::new();
    for manifest in ["fonts/latin-manifest.txt", "fonts/manifest.txt"] {
        let path = dir.join(manifest);
        if manifest == "fonts/latin-manifest.txt" && !path.exists() {
            watch(&path);
            continue;
        }
        match kition_fonts(dir, manifest, wasm, embed_cjk) {
            Ok(catalog) => {
                out.push_str(&catalog.entries);
                faces.extend(catalog.faces);
            }
            Err(e) if required => return Err(format!("KITION_FONTS_DIR={}: {e}", dir.display())),
            Err(e) => println!("cargo::warning=kition-fonts {manifest} skipped: KITION_FONTS_DIR={}: {e}", dir.display()),
        }
    }
    if required && wasm {
        for (family, style) in [("Inter", "Regular"), ("Inter", "SemiBold"), ("Noto Sans Georgian", "Regular")] {
            if !faces.iter().any(|(f, s)| f == family && s == style) {
                return Err(format!("required web font missing: {family} {style}; KITION_FONTS_DIR must include both Latin and core release faces"));
            }
        }
    }
    Ok(out)
}

/// Rebuild when `manifest` changes or, while it is missing, when it is added to its folder.
///
/// Cargo takes a watched path that doesn't exist as changed on every build, and then rebuilds this
/// crate and every crate that depends on it each time. So a missing manifest is never watched
/// itself: its folder is, when there is one (a kition-fonts checkout without the Latin manifest).
/// With no kition-fonts at all there is nothing to watch, so the build can't notice it arriving:
/// rebuild this crate after adding a previously absent checkout.
fn watch(manifest: &std::path::Path) {
    let watched = if manifest.exists() { Some(manifest) } else { manifest.parent().filter(|folder| folder.is_dir()) };
    if let Some(path) = watched {
        println!("cargo::rerun-if-changed={}", path.display());
    }
}

struct Catalog {
    entries: String,
    faces: Vec<(String, String)>,
}

/// Parse a font manifest of `family | style | file | scripts` rows into `KitionFont`
/// entries: skip Adobe-authored families, skip CJK unless requested, keep the web catalog
/// to its budgeted families, canonicalize and watch each face, and emit `include_bytes!`
/// items.
fn kition_fonts(dir: &std::path::Path, manifest_rel: &str, wasm: bool, embed_cjk: bool) -> Result<Catalog, String> {
    let manifest = dir.join(manifest_rel);
    watch(&manifest);
    let text = std::fs::read_to_string(&manifest).map_err(|e| format!("{}: {e}", manifest.display()))?;
    let mut out = String::new();
    let mut faces = Vec::new();
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
        let f: Vec<&str> = line.split(" | ").map(str::trim).collect();
        let [family, style, file, scripts, ..] = f.as_slice() else {
            return Err(format!("malformed manifest line: {line}"));
        };
        // Kition Design's asset policy excludes Adobe-authored visual assets on every target.
        if matches!(*family, "Source Sans 3" | "Source Serif 4") {
            continue;
        }
        // CJK fonts are large; the desktop app finds system CJK fonts instead unless asked.
        let cjk = scripts.contains("Hans");
        if cjk && !embed_cjk {
            continue;
        }
        // Web builds have a size budget: Inter, Noto Sans Georgian (small, for the Georgian UI), only the
        // SemiBold wordmark, and optionally Noto Sans CJK (every CJK script, for CJK UI text).
        let wordmark = *family == "Archivo" && *style == "SemiBold";
        if wasm && !matches!(*family, "Inter" | "Noto Sans Georgian") && !wordmark && !cjk {
            continue;
        }
        let path = dir.join(file).canonicalize().map_err(|e| format!("{file}: {e}"))?;
        println!("cargo::rerun-if-changed={}", path.display());
        if undrawable_colour_font(&std::fs::read(&path).map_err(|e| format!("{file}: {e}"))?) {
            continue;
        }
        faces.push(((*family).to_owned(), (*style).to_owned()));
        let _ = writeln!(out, "    KitionFont {{ family: {family:?}, style: {style:?}, bytes: include_bytes!({:?}) }},", path.display().to_string());
    }
    Ok(Catalog { entries: out, faces })
}

/// Whether a font file's glyphs are colour the renderer does not draw: a `COLR` table without
/// version 0 layers (only COLR v1 paint graphs) and no bitmap strikes (`sbix`, `CBDT`, `EBDT`).
/// kition-fonts' Noto Color Emoji is such a font: its outlines are empty, so embedded it drew
/// nothing, and it took the place of the system's own Noto Color Emoji (CBDT strikes on Linux),
/// which the system font scan does not load beside an embedded face of the same name and style.
/// A file that is not a single OpenType font is kept.
fn undrawable_colour_font(bytes: &[u8]) -> bool {
    let u16_at = |at: usize| bytes.get(at..at + 2).map(|b| u16::from_be_bytes([b[0], b[1]]));
    let u32_at = |at: usize| bytes.get(at..at + 4).map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]));
    if bytes.get(..4) == Some(b"ttcf") {
        return false;
    }
    let Some(count) = u16_at(4) else { return false };
    let mut colr = None;
    for record in (0..usize::from(count)).map(|i| 12 + 16 * i) {
        let Some(tag) = bytes.get(record..record + 4) else { return false };
        match tag {
            b"sbix" | b"CBDT" | b"EBDT" => return false,
            b"COLR" => colr = u32_at(record + 8),
            _ => {}
        }
    }
    // COLR: version (u16), then numBaseGlyphRecords (u16), the version 0 layers' count.
    colr.and_then(|at| u16_at(at as usize + 2)).is_some_and(|base_glyphs| base_glyphs == 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(0);

    struct Fixture(PathBuf);

    impl Fixture {
        /// Test fixture: create a unique temporary directory, write a synthetic face file and
        /// install the default Georgian manifest.
        fn new() -> Self {
            let dir = std::env::temp_dir().join(format!("design-font-input-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
            std::fs::create_dir_all(dir.join("fonts")).unwrap();
            // The build script validates manifest/file presence; font bytes are checked centrally.
            std::fs::write(dir.join("fonts/face.ttf"), b"synthetic build-input fixture").unwrap();
            let fixture = Self(dir);
            fixture.manifest("manifest.txt", &[("Noto Sans Georgian", "Regular")]);
            fixture
        }

        /// Test fixture: write manifest rows that all point at the fixture's `fonts/face.ttf` face.
        fn manifest(&self, name: &str, faces: &[(&str, &str)]) {
            let text = faces.iter().map(|(family, style)| format!("{family} | {style} | fonts/face.ttf | Latn\n")).collect::<String>();
            std::fs::write(self.0.join("fonts").join(name), text).unwrap();
        }
    }

    impl Drop for Fixture {
        /// Remove the fixture's temporary directory, ignoring errors.
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// Asserts the native Latin catalog keeps its 43 permitted presentation faces, the web
    /// catalog keeps only the four Inter faces, Adobe-authored families are absent, and
    /// permitted native faces remain.
    #[test]
    fn restored_latin_catalog_preserves_permitted_native_faces_and_web_budget() {
        let f = Fixture::new();
        std::fs::write(f.0.join("fonts/latin-manifest.txt"), include_str!("tests/fixtures/latin-families.txt")).unwrap();
        let native = kition_fonts(&f.0, "fonts/latin-manifest.txt", false, false).unwrap();
        assert_eq!(native.faces.len(), 43, "all prior permitted presentation faces remain available");
        let web = kition_fonts(&f.0, "fonts/latin-manifest.txt", true, false).unwrap();
        assert_eq!(web.faces.len(), 4, "the web catalog keeps only the four Inter faces");
        for catalog in [&native, &web] {
            assert!(!catalog.entries.contains("Source Sans 3"));
            assert!(!catalog.entries.contains("Source Serif 4"));
        }
        assert!(native.faces.iter().any(|(family, _)| family == "Carlito"));
        assert!(native.faces.iter().any(|(family, _)| family == "Liberation Serif"));
        assert!(input_fonts(Some(&f.0), true, true, false).is_ok());
    }

    /// Asserts required web input rejects a catalog missing Inter Regular or Inter SemiBold.
    #[test]
    fn required_web_rejects_core_only_and_partial_latin_catalogs() {
        let f = Fixture::new();
        assert!(input_fonts(Some(&f.0), true, true, false).unwrap_err().contains("Inter Regular"));
        f.manifest("latin-manifest.txt", &[("Inter", "Regular"), ("Unrelated", "SemiBold")]);
        assert!(input_fonts(Some(&f.0), true, true, false).unwrap_err().contains("Inter SemiBold"));
    }

    /// Asserts required web input embeds the Inter and Georgian catalogs (three entries) and
    /// rejects a manifest without Noto Sans Georgian Regular.
    #[test]
    fn required_web_embeds_both_catalogs_and_requires_georgian() {
        let f = Fixture::new();
        f.manifest("latin-manifest.txt", &[("Inter", "Regular"), ("Inter", "SemiBold")]);
        let entries = input_fonts(Some(&f.0), true, true, false).unwrap();
        assert_eq!(entries.matches("KitionFont {").count(), 3);
        assert!(entries.contains("family: \"Inter\""));
        assert!(entries.contains("family: \"Noto Sans Georgian\""));
        f.manifest("manifest.txt", &[("Other", "Regular")]);
        assert!(input_fonts(Some(&f.0), true, true, false).unwrap_err().contains("Noto Sans Georgian Regular"));
    }

    /// Asserts absent font input is empty when optional and an error when required, while present
    /// input includes Noto Sans Georgian.
    #[test]
    fn optional_and_native_inputs_keep_their_existing_policy() {
        let f = Fixture::new();
        assert!(input_fonts(None, true, false, false).unwrap().is_empty());
        assert!(input_fonts(None, true, true, false).unwrap_err().contains("KITION_FONTS_DIR is missing"));
        assert!(input_fonts(Some(&f.0), true, false, false).unwrap().contains("Noto Sans Georgian"));
        assert!(input_fonts(Some(&f.0), false, true, false).unwrap().contains("Noto Sans Georgian"));
    }

    /// A synthetic OpenType file holding `tables`, each with the given bytes.
    fn font_with(tables: &[(&[u8; 4], &[u8])]) -> Vec<u8> {
        let mut out = vec![0, 1, 0, 0];
        out.extend((tables.len() as u16).to_be_bytes());
        out.extend([0; 6]);
        let mut at = 12 + 16 * tables.len();
        for (tag, data) in tables {
            out.extend(tag.iter());
            out.extend([0; 4]);
            out.extend((at as u32).to_be_bytes());
            out.extend((data.len() as u32).to_be_bytes());
            at += data.len();
        }
        for (_, data) in tables {
            out.extend(data.iter());
        }
        out
    }

    /// Asserts a colour font whose only colour is COLR v1 paint graphs (kition-fonts' Noto Color
    /// Emoji) is not embedded, while COLR v0 layers, bitmap strikes and plain outlines are.
    #[test]
    fn colour_fonts_the_renderer_does_not_draw_are_not_embedded() {
        let f = Fixture::new();
        f.manifest("manifest.txt", &[("Noto Sans Georgian", "Regular"), ("Noto Color Emoji", "Regular")]);
        let colr_v1 = [0u8, 1, 0, 0];
        let colr_v0 = [0u8, 0, 0, 3];
        for (tables, embedded) in [
            (vec![(b"glyf", &[0u8; 4][..]), (b"COLR", &colr_v1[..])], false),
            (vec![(b"glyf", &[0u8; 4][..]), (b"COLR", &colr_v0[..])], true),
            (vec![(b"CBDT", &[0u8; 4][..]), (b"COLR", &colr_v1[..])], true),
            (vec![(b"glyf", &[0u8; 4][..])], true),
        ] {
            std::fs::write(f.0.join("fonts/face.ttf"), font_with(&tables)).unwrap();
            let native = kition_fonts(&f.0, "fonts/manifest.txt", false, false).unwrap();
            assert_eq!(native.faces.len(), if embedded { 2 } else { 0 }, "{tables:?}");
        }
    }

    /// Asserts required web input rejects a manifest whose face file is missing, even when every
    /// face row is listed.
    #[test]
    fn required_web_rejects_missing_asset_even_with_all_face_rows() {
        let f = Fixture::new();
        f.manifest("latin-manifest.txt", &[("Inter", "Regular"), ("Inter", "SemiBold")]);
        std::fs::remove_file(f.0.join("fonts/face.ttf")).unwrap();
        assert!(input_fonts(Some(&f.0), true, true, false).unwrap_err().contains("fonts/face.ttf"));
    }
}
