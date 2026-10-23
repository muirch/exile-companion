//! Carries files over from an earlier install that kept its data under another app id.

use std::path::Path;

/// Copies each of `files` from `old` to `new` unless `new` already has it. Returns what was
/// copied. A missing `old` dir is not an error: most installs never had one.
pub fn carry_over(old: &Path, new: &Path, files: &[&str]) -> std::io::Result<Vec<String>> {
    let mut copied = Vec::new();
    if !old.is_dir() {
        return Ok(copied);
    }
    std::fs::create_dir_all(new)?;
    for name in files {
        let (from, to) = (old.join(name), new.join(name));
        if from.is_file() && !to.exists() {
            std::fs::copy(&from, &to)?;
            copied.push((*name).to_string());
        }
    }
    Ok(copied)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copies_what_the_new_dir_lacks() {
        let dir = tempfile::tempdir().unwrap();
        let (old, new) = (dir.path().join("old"), dir.path().join("new"));
        std::fs::create_dir_all(&old).unwrap();
        std::fs::write(old.join("settings.json"), "{\"opacity\":0.9}").unwrap();
        std::fs::write(old.join("build.xml"), "<PathOfBuilding/>").unwrap();

        let copied = carry_over(&old, &new, &["settings.json", "build.xml"]).unwrap();
        assert_eq!(copied, ["settings.json", "build.xml"]);
        assert_eq!(
            std::fs::read_to_string(new.join("settings.json")).unwrap(),
            "{\"opacity\":0.9}"
        );
    }

    #[test]
    fn never_overwrites_newer_files() {
        let dir = tempfile::tempdir().unwrap();
        let (old, new) = (dir.path().join("old"), dir.path().join("new"));
        std::fs::create_dir_all(&old).unwrap();
        std::fs::create_dir_all(&new).unwrap();
        std::fs::write(old.join("settings.json"), "old").unwrap();
        std::fs::write(new.join("settings.json"), "new").unwrap();

        assert!(carry_over(&old, &new, &["settings.json"])
            .unwrap()
            .is_empty());
        assert_eq!(
            std::fs::read_to_string(new.join("settings.json")).unwrap(),
            "new"
        );
    }

    #[test]
    fn no_old_install_is_fine() {
        let dir = tempfile::tempdir().unwrap();
        let copied = carry_over(&dir.path().join("none"), &dir.path().join("new"), &["a"]).unwrap();
        assert!(copied.is_empty());
    }
}
