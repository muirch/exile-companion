use std::path::{Path, PathBuf};

const LOG_IN_INSTALL: &str = "logs/Client.txt";

/// The saved path if it still exists, otherwise the first Client.txt found in the usual install
/// locations, including every Steam library listed in libraryfolders.vdf.
pub fn find_client_log(saved: Option<&str>) -> Option<PathBuf> {
    if let Some(p) = saved.map(PathBuf::from).filter(|p| p.is_file()) {
        return Some(p);
    }
    install_dirs()
        .into_iter()
        .map(|dir| dir.join(LOG_IN_INSTALL))
        .find(|p| p.is_file())
}

fn install_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    for steam in steam_roots() {
        for library in steam_libraries(&steam) {
            dirs.push(library.join("steamapps/common/Path of Exile"));
        }
    }
    if cfg!(windows) {
        for drive in ["C:", "D:", "E:"] {
            dirs.push(PathBuf::from(format!(
                r"{drive}\Program Files (x86)\Grinding Gear Games\Path of Exile"
            )));
            dirs.push(PathBuf::from(format!(
                r"{drive}\Program Files\Grinding Gear Games\Path of Exile"
            )));
        }
    }
    dirs
}

fn steam_roots() -> Vec<PathBuf> {
    if cfg!(windows) {
        vec![
            PathBuf::from(r"C:\Program Files (x86)\Steam"),
            PathBuf::from(r"C:\Program Files\Steam"),
        ]
    } else {
        let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
            return Vec::new();
        };
        vec![
            home.join(".local/share/Steam"),
            home.join(".steam/steam"),
            home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"),
        ]
    }
}

fn steam_libraries(steam_root: &Path) -> Vec<PathBuf> {
    let mut libraries = vec![steam_root.to_path_buf()];
    let vdf = steam_root.join("steamapps/libraryfolders.vdf");
    if let Ok(text) = std::fs::read_to_string(vdf) {
        libraries.extend(library_paths(&text));
    }
    libraries
}

// libraryfolders.vdf has lines like `"path"		"D:\\SteamLibrary"`, backslashes escaped.
fn library_paths(vdf: &str) -> Vec<PathBuf> {
    vdf.lines()
        .filter_map(|line| {
            let rest = line.trim().strip_prefix("\"path\"")?.trim();
            let value = rest.strip_prefix('"')?.strip_suffix('"')?;
            Some(PathBuf::from(value.replace("\\\\", "\\")))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_library_paths_from_vdf() {
        let vdf = "\"libraryfolders\"\n{\n\t\"0\"\n\t{\n\t\t\"path\"\t\t\"C:\\\\Program Files (x86)\\\\Steam\"\n\t}\n\t\"1\"\n\t{\n\t\t\"path\"\t\t\"D:\\\\SteamLibrary\"\n\t}\n}\n";
        assert_eq!(
            library_paths(vdf),
            [
                PathBuf::from(r"C:\Program Files (x86)\Steam"),
                PathBuf::from(r"D:\SteamLibrary"),
            ]
        );
    }
}
