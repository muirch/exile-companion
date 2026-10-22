//! `poebo-pob` - import a Path of Building build for the poebo PoE1 leveling assistant.
//!
//! A build reaches us as an **import code** (pasted from PoB / pobb.in / pastebin, shaped
//! `url-safe-base64( zlib( xml ) )`), a **local file** (raw exported XML, or a saved code), or a
//! **build link**, which the app turns into a code with [`code_url`] and one download. This crate
//! decodes and parses into a [`PobBuild`] and does no network calls itself, so it can be
//! unit-tested headless.
//!
//! The parse is deliberately forgiving: PoB has no formal schema, so children are matched by element
//! name and every attribute is optional (see [`model`]).

#![forbid(unsafe_code)]

pub mod decode;
pub mod error;
pub mod link;
pub mod model;
pub mod parse;

use std::path::Path;

pub use decode::decode_import_code;
pub use error::PobError;
pub use link::code_url;
pub use model::{Gem, PobBuild, SkillGroup, SkillSet, TreeSpec};
pub use parse::parse_xml;

/// Import a build from a PoB code (as pasted): decode base64+zlib, then parse the XML.
pub fn import_code(code: &str) -> Result<PobBuild, PobError> {
    let xml = decode_import_code(code)?;
    parse_xml(&xml)
}

/// Import a build from arbitrary pasted text: raw PoB XML (starts with `<`) is parsed directly,
/// anything else is treated as an import code. Use this for clipboard/paste input.
pub fn import_text(text: &str) -> Result<PobBuild, PobError> {
    if text.trim_start().starts_with('<') {
        parse_xml(text.as_bytes())
    } else {
        import_code(text)
    }
}

/// Import a build from a local file. A file that starts with `<` is treated as raw PoB XML;
/// otherwise its contents are treated as an import code.
pub fn from_local_file(path: &Path) -> Result<PobBuild, PobError> {
    let bytes = std::fs::read(path).map_err(|source| PobError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    import_text(&String::from_utf8_lossy(&bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::write::ZlibEncoder;
    use flate2::Compression;
    use std::io::Write;

    const XML: &str = r#"<PathOfBuilding><Build level="42" className="Ranger"/></PathOfBuilding>"#;

    fn encode(xml: &str) -> String {
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(xml.as_bytes()).expect("compress");
        base64::Engine::encode(
            &base64::engine::general_purpose::URL_SAFE,
            encoder.finish().expect("finish"),
        )
    }

    #[test]
    fn import_code_decodes_and_parses() {
        let build = import_code(&encode(XML)).expect("import");
        assert_eq!(build.level, Some(42));
        assert_eq!(build.class_name.as_deref(), Some("Ranger"));
    }

    #[test]
    fn import_text_accepts_both_raw_xml_and_a_code() {
        // Leading whitespace + raw XML -> parsed directly.
        let from_xml = import_text(&format!("  \n{XML}")).expect("xml");
        assert_eq!(from_xml.level, Some(42));
        // A base64 code -> decoded then parsed.
        let from_code = import_text(&encode(XML)).expect("code");
        assert_eq!(from_code.class_name.as_deref(), Some("Ranger"));
    }

    #[test]
    fn from_local_file_reads_raw_xml() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("build.xml");
        std::fs::write(&path, XML).expect("write");
        let build = from_local_file(&path).expect("import");
        assert_eq!(build.class_name.as_deref(), Some("Ranger"));
    }

    #[test]
    fn from_local_file_reads_a_saved_code() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("build.txt");
        std::fs::write(&path, encode(XML)).expect("write");
        let build = from_local_file(&path).expect("import");
        assert_eq!(build.level, Some(42));
    }

    #[test]
    fn missing_file_is_an_io_error() {
        let err = from_local_file(Path::new("/no/such/pob/file.xml")).unwrap_err();
        assert!(matches!(err, PobError::Io { .. }));
    }
}
