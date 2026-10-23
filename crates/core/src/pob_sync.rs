//! Keeps the app's build and one build file in Path of Building's Builds folder the same. Whichever
//! changed last wins: the player saves in PoB, or the app imports or the assistant changes the build.

use std::path::Path;
use std::time::SystemTime;

use crate::build_history::write_atomic;

/// The file in PoB's Builds folder when the player hasn't picked one. PoB lists it by this name.
pub const DEFAULT_FILE: &str = "Exile Companion.xml";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Synced {
    /// Both are the same, or neither exists.
    Nothing,
    /// The app's build went to PoB's file.
    Pushed,
    /// PoB's file became the app's build.
    Pulled,
}

fn read(path: &Path) -> std::io::Result<Option<(Vec<u8>, SystemTime)>> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(Some((bytes, std::fs::metadata(path)?.modified()?))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

pub fn sync(build: &Path, pob_file: &Path) -> std::io::Result<Synced> {
    let bytes = match (read(build)?, read(pob_file)?) {
        (None, None) => return Ok(Synced::Nothing),
        (Some((a, _)), Some((b, _))) if a == b => return Ok(Synced::Nothing),
        // On a tie PoB wins: the player saved there.
        (Some((ours, t)), Some((_, u))) if t > u => return push(&ours, pob_file),
        (Some((ours, _)), None) => return push(&ours, pob_file),
        (_, Some((theirs, _))) => theirs,
    };
    // PoB may be halfway through saving, or the file isn't a build at all. Try again next time.
    if exile_pob::parse_xml(&bytes).is_err() {
        return Ok(Synced::Nothing);
    }
    write_atomic(build, &bytes)?;
    Ok(Synced::Pulled)
}

fn push(bytes: &[u8], pob_file: &Path) -> std::io::Result<Synced> {
    if let Some(dir) = pob_file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    write_atomic(pob_file, bytes)?;
    Ok(Synced::Pushed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    const A: &str = r#"<PathOfBuilding><Build level="10" className="Ranger"/></PathOfBuilding>"#;
    const B: &str = r#"<PathOfBuilding><Build level="20" className="Ranger"/></PathOfBuilding>"#;

    fn write(path: &Path, text: &str, age_secs: u64) {
        std::fs::write(path, text).unwrap();
        let time = SystemTime::now() - Duration::from_secs(age_secs);
        std::fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(time)
            .unwrap();
    }

    fn setup() -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let build = dir.path().join("build.xml");
        let pob = dir.path().join("Builds").join(DEFAULT_FILE);
        (dir, build, pob)
    }

    fn text(path: &Path) -> String {
        std::fs::read_to_string(path).unwrap()
    }

    #[test]
    fn creates_the_pob_file_from_the_build() {
        let (_dir, build, pob) = setup();
        assert_eq!(sync(&build, &pob).unwrap(), Synced::Nothing);
        write(&build, A, 0);
        assert_eq!(sync(&build, &pob).unwrap(), Synced::Pushed);
        assert_eq!(text(&pob), A);
        assert_eq!(sync(&build, &pob).unwrap(), Synced::Nothing);
    }

    #[test]
    fn the_newer_side_wins() {
        let (_dir, build, pob) = setup();
        std::fs::create_dir_all(pob.parent().unwrap()).unwrap();
        write(&build, A, 60);
        write(&pob, B, 10);
        assert_eq!(sync(&build, &pob).unwrap(), Synced::Pulled);
        assert_eq!(text(&build), B);

        write(&build, A, 0);
        assert_eq!(sync(&build, &pob).unwrap(), Synced::Pushed);
        assert_eq!(text(&pob), A);
    }

    #[test]
    fn takes_a_pob_build_when_the_app_has_none() {
        let (_dir, build, pob) = setup();
        std::fs::create_dir_all(pob.parent().unwrap()).unwrap();
        write(&pob, B, 0);
        assert_eq!(sync(&build, &pob).unwrap(), Synced::Pulled);
        assert_eq!(text(&build), B);
    }

    #[test]
    fn leaves_the_build_alone_while_pob_writes_half_a_file() {
        let (_dir, build, pob) = setup();
        std::fs::create_dir_all(pob.parent().unwrap()).unwrap();
        write(&build, A, 60);
        write(&pob, "<PathOfBuilding><Bui", 0);
        assert_eq!(sync(&build, &pob).unwrap(), Synced::Nothing);
        assert_eq!(text(&build), A);
    }
}
