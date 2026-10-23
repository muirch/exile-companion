use std::path::{Path, PathBuf};

const LOG_IN_INSTALL: &str = "logs/Client.txt";

/// Enough for the last few zones of a session. Client.txt itself can be hundreds of MB.
pub const HISTORY_BYTES: u64 = 512 * 1024;

/// The last `bytes` of the log. The first line may come out cut, which the parser skips.
pub fn read_tail(path: &Path, bytes: u64) -> std::io::Result<String> {
    use std::io::{Read, Seek, SeekFrom};
    let mut file = std::fs::File::open(path)?;
    let len = file.metadata()?.len();
    file.seek(SeekFrom::Start(len.saturating_sub(bytes)))?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf)?;
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

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
    fn reads_only_the_end_of_the_log() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Client.txt");
        std::fs::write(&path, "first line\nsecond line\n").unwrap();
        assert_eq!(read_tail(&path, 12).unwrap(), "second line\n");
        assert_eq!(read_tail(&path, 1000).unwrap(), "first line\nsecond line\n");
    }

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

    #[test]
    fn skips_vdf_lines_that_are_not_paths() {
        let vdf = "\"1\"\n{\n\t\"label\"\t\t\"\"\n\t\"path\"\t\t\"E:\\\\Games\"\n\t\"path\"\n}\n";
        assert_eq!(library_paths(vdf), [PathBuf::from(r"E:\Games")]);
    }

    #[test]
    fn lists_the_steam_root_and_its_vdf_libraries() {
        let steam = tempfile::tempdir().unwrap();
        let library = tempfile::tempdir().unwrap();
        let vdf = format!("\t\t\"path\"\t\t\"{}\"\n", library.path().display());
        std::fs::create_dir_all(steam.path().join("steamapps")).unwrap();
        std::fs::write(steam.path().join("steamapps/libraryfolders.vdf"), vdf).unwrap();

        assert_eq!(
            steam_libraries(steam.path()),
            [steam.path().to_path_buf(), library.path().to_path_buf()]
        );
    }

    #[test]
    fn keeps_the_saved_path_while_it_exists() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("Client.txt");
        std::fs::write(&log, "").unwrap();
        assert_eq!(find_client_log(log.to_str()), Some(log));
    }

    #[test]
    fn ignores_a_saved_path_that_is_gone() {
        let dir = tempfile::tempdir().unwrap();
        let gone = dir.path().join("Client.txt");
        assert_ne!(find_client_log(gone.to_str()), Some(gone));
    }
}
