use std::path::{Path, PathBuf};

use crate::PobDir;

/// The saved folder if it still holds PoB, otherwise the first PoB found in `extra` (install
/// folders the app read from the registry) or in the usual install locations.
pub fn find_pob(saved: Option<&Path>, extra: &[PathBuf]) -> Option<PobDir> {
    saved
        .map(Path::to_path_buf)
        .into_iter()
        .chain(extra.iter().cloned())
        .chain(usual_dirs())
        .find_map(|dir| PobDir::detect(&dir).ok())
}

fn usual_dirs() -> Vec<PathBuf> {
    let env = |name: &str| std::env::var_os(name).map(PathBuf::from);
    let mut dirs = Vec::new();
    for base in [
        env("APPDATA"),
        env("LOCALAPPDATA").map(|d| d.join("Programs")),
    ]
    .into_iter()
    .chain([env("ProgramFiles"), env("ProgramFiles(x86)")])
    .flatten()
    {
        dirs.push(base.join("Path of Building Community"));
        dirs.push(base.join("Path of Building"));
    }
    // The portable zip unpacks into a folder of its own name, usually where it was downloaded.
    if let Some(home) = env("USERPROFILE").or_else(|| env("HOME")) {
        for place in ["Downloads", "Desktop", "Documents"] {
            dirs.push(home.join(place).join("PathOfBuildingCommunity-Portable"));
            dirs.push(home.join(place).join("Path of Building Community"));
        }
    }
    dirs
}

/// Install folder from an uninstall registry entry: InstallLocation when the installer wrote it,
/// otherwise the folder of the uninstaller or the icon, which sit in the install folder.
pub fn install_dir_from_uninstall_entry(
    install_location: Option<&str>,
    uninstall_string: Option<&str>,
    display_icon: Option<&str>,
) -> Option<PathBuf> {
    if let Some(dir) = install_location.map(str::trim).filter(|d| !d.is_empty()) {
        return Some(PathBuf::from(dir.trim_matches('"')));
    }
    [uninstall_string, display_icon]
        .into_iter()
        .flatten()
        .find_map(|cmd| {
            // `"C:\...\uninstall.exe" /S` or `C:\...\Path of Building.exe,0`
            let cmd = cmd.trim();
            let exe = match cmd.strip_prefix('"') {
                Some(rest) => &rest[..rest.find('"')?],
                None => cmd.split(',').next()?.trim(),
            };
            // Registry paths use backslashes, which Path only splits on Windows.
            if !exe.to_ascii_lowercase().ends_with(".exe") {
                return None;
            }
            let dir = &exe[..exe.rfind(['\\', '/'])?];
            Some(PathBuf::from(dir))
        })
}

/// Whether an uninstall entry's DisplayName is PoB for Path of Exile 1. PoB for PoE 2 is a
/// separate program with the same name and a suffix.
pub fn is_pob1_display_name(name: &str) -> bool {
    name.starts_with("Path of Building") && !name.contains("PoE2") && !name.contains("PoE 2")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_pob(dir: &Path) {
        for f in ["Launch.lua", "lua/xml.lua"] {
            let p = dir.join(f);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, "").unwrap();
        }
    }

    #[test]
    fn prefers_the_saved_folder_then_the_registry() {
        let saved = tempfile::tempdir().unwrap();
        let registry = tempfile::tempdir().unwrap();
        fake_pob(saved.path());
        fake_pob(registry.path());
        let extra = [registry.path().to_path_buf()];

        let found = find_pob(Some(saved.path()), &extra).unwrap();
        assert_eq!(found.root, std::path::absolute(saved.path()).unwrap());

        let gone = saved.path().join("moved");
        let found = find_pob(Some(&gone), &extra).unwrap();
        assert_eq!(found.root, std::path::absolute(registry.path()).unwrap());
    }

    #[test]
    fn reads_the_install_folder_from_an_uninstall_entry() {
        assert_eq!(
            install_dir_from_uninstall_entry(Some(r"C:\PoB"), Some(r#""D:\x\u.exe""#), None),
            Some(PathBuf::from(r"C:\PoB"))
        );
        assert_eq!(
            install_dir_from_uninstall_entry(
                None,
                Some(r#""C:\Users\me\AppData\Roaming\PoB\Uninstall.exe" /S"#),
                None
            ),
            Some(PathBuf::from(r"C:\Users\me\AppData\Roaming\PoB"))
        );
        assert_eq!(
            install_dir_from_uninstall_entry(
                Some(""),
                None,
                Some(r"C:\PoB\Path of Building.exe,0")
            ),
            Some(PathBuf::from(r"C:\PoB"))
        );
        assert_eq!(install_dir_from_uninstall_entry(None, None, None), None);
    }

    #[test]
    fn tells_pob_for_poe1_from_poe2() {
        assert!(is_pob1_display_name("Path of Building Community"));
        assert!(!is_pob1_display_name("Path of Building Community (PoE2)"));
        assert!(!is_pob1_display_name("Path of Exile"));
    }
}
