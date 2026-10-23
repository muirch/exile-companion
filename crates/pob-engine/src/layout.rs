use std::path::{Path, PathBuf};

use crate::EngineError;

/// Where PoB's Lua program and its runtime libraries live. A git checkout keeps them in `src/` and
/// `runtime/lua/`, an installed copy has the program at its root and the libraries in `lua/`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PobDir {
    pub root: PathBuf,
    pub src: PathBuf,
    pub lua: PathBuf,
    /// From manifest.xml. None when there's no manifest, as in a bare checkout of `src/`.
    pub version: Option<String>,
}

impl PobDir {
    pub fn detect(root: &Path) -> Result<Self, EngineError> {
        // The host moves into `src` before Lua looks for the libraries, so relative paths would break.
        // Not canonicalize: on Windows it adds a \\?\ prefix that the forward-slash Lua path can't keep.
        let root = &std::path::absolute(root).map_err(EngineError::Io)?;
        let candidates = [
            (root.join("src"), root.join("runtime").join("lua")),
            (root.to_path_buf(), root.join("lua")),
        ];
        let (src, lua) = candidates
            .into_iter()
            .find(|(src, lua)| src.join("Launch.lua").is_file() && lua.join("xml.lua").is_file())
            .ok_or_else(|| EngineError::NotPob(root.to_path_buf()))?;
        let version = std::fs::read_to_string(root.join("manifest.xml"))
            .ok()
            .and_then(|m| manifest_version(&m));
        Ok(Self {
            root: root.to_path_buf(),
            src,
            lua,
            version,
        })
    }
}

fn manifest_version(manifest: &str) -> Option<String> {
    let tag = &manifest[manifest.find("<Version ")?..];
    let tag = &tag[..tag.find('>')?];
    let value = &tag[tag.find("number=")? + "number=".len()..];
    let quote = value.chars().next().filter(|c| *c == '"' || *c == '\'')?;
    let value = &value[1..];
    Some(value[..value.find(quote)?].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(path: &Path) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "").unwrap();
    }

    #[test]
    fn finds_a_git_checkout() {
        let dir = tempfile::tempdir().unwrap();
        touch(&dir.path().join("src/Launch.lua"));
        touch(&dir.path().join("runtime/lua/xml.lua"));
        std::fs::write(
            dir.path().join("manifest.xml"),
            "<PoBVersion>\n\t<Version number=\"2.67.2\" />\n</PoBVersion>",
        )
        .unwrap();
        let pob = PobDir::detect(dir.path()).unwrap();
        assert_eq!(pob.src, dir.path().join("src"));
        assert_eq!(pob.lua, dir.path().join("runtime/lua"));
        assert_eq!(pob.version.as_deref(), Some("2.67.2"));
    }

    #[test]
    fn finds_an_installed_copy() {
        let dir = tempfile::tempdir().unwrap();
        touch(&dir.path().join("Launch.lua"));
        touch(&dir.path().join("lua/xml.lua"));
        let pob = PobDir::detect(dir.path()).unwrap();
        assert_eq!(pob.src, dir.path());
        assert_eq!(pob.lua, dir.path().join("lua"));
        assert_eq!(pob.version, None);
    }

    #[test]
    fn rejects_a_folder_without_pob() {
        let dir = tempfile::tempdir().unwrap();
        touch(&dir.path().join("Launch.lua"));
        assert!(matches!(
            PobDir::detect(dir.path()),
            Err(EngineError::NotPob(_))
        ));
    }

    #[test]
    fn reads_the_version_with_either_quote() {
        assert_eq!(
            manifest_version("<Version number='2.60.0' branch=\"master\"/>").as_deref(),
            Some("2.60.0")
        );
        assert_eq!(manifest_version("<PoBVersion></PoBVersion>"), None);
    }
}
