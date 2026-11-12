//! The passive tree window: the guide's tree for the character's stage, in the menu key's layer
//! next to the gems window. The player turns it on in the quick menu.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use exile_core::tree::{self, TreeLayout, TreeMode, TreeStage};
use exile_pob_engine::PobDir;
use tauri::{AppHandle, Emitter, Manager};

use crate::settings;
use crate::state::AppState;

pub const TREE_LABEL: &str = "tree";
// At 1080p, as in tauri.conf.json.
const SIZE: (f64, f64) = (632.0, 540.0);
const MIN_SIZE: (f64, f64) = (420.0, 320.0);

static SIZED: OnceLock<()> = OnceLock::new();
static LAYOUTS: Mutex<Option<HashMap<PathBuf, Arc<TreeLayout>>>> = Mutex::new(None);

fn place(app: &AppHandle, window: &tauri::WebviewWindow) {
    let saved = settings::load(app).unwrap_or_default();
    crate::panel::place(
        app,
        window,
        &SIZED,
        crate::panel::Place {
            size: saved.tree_size.unwrap_or(SIZE),
            min_size: MIN_SIZE,
            anchor: saved.tree_anchor,
            // Right of the gems window where it opens first.
            default_anchor: (1.0 / 40.0 + 356.0 / 1920.0, 1.0 / 8.0),
        },
    );
}

pub fn show(app: &AppHandle) {
    let Some(w) = app.get_webview_window(TREE_LABEL) else {
        return;
    };
    if crate::runtime::plan_source(app).is_none() {
        return;
    }
    place(app, &w);
    let _ = w.show();
    let _ = app.emit_to(TREE_LABEL, "tree://opened", ());
}

pub fn hide(app: &AppHandle) {
    let Some(w) = app.get_webview_window(TREE_LABEL) else {
        return;
    };
    if !w.is_visible().unwrap_or(false) {
        return;
    }
    if let (Some(spot), Ok(mut s)) = (crate::panel::spot(&w), settings::load(app)) {
        s.tree_anchor = Some(spot.anchor);
        s.tree_size = Some(spot.size);
        let _ = settings::save(app, &s);
    }
    let _ = w.hide();
}

pub fn shown(app: &AppHandle) -> bool {
    settings::load(app).unwrap_or_default().tree_shown
}

/// Turns the window on or off. The layer is open while the menu is, so it shows or hides at once.
pub fn set_shown(app: &AppHandle, on: bool) -> Result<(), String> {
    let mut s = settings::load(app)?;
    s.tree_shown = on;
    settings::save(app, &s)?;
    if on {
        show(app);
    } else {
        hide(app);
    }
    Ok(())
}

pub fn set_mode(app: &AppHandle, mode: TreeMode) -> Result<(), String> {
    let mut s = settings::load(app)?;
    s.tree_mode = mode;
    settings::save(app, &s)
}

pub fn prefs(app: &AppHandle) -> settings::TreePrefs {
    settings::load(app).unwrap_or_default().tree_prefs
}

pub fn set_prefs(app: &AppHandle, prefs: settings::TreePrefs) -> Result<(), String> {
    let mut s = settings::load(app)?;
    s.tree_prefs = prefs;
    settings::save(app, &s)
}

fn pob_src(app: &AppHandle) -> Result<PathBuf, String> {
    let path = settings::load(app)?
        .pob_path
        .ok_or("Path of Building wasn't found. Pick its folder in the settings.")?;
    PobDir::detect(Path::new(&path))
        .map(|d| d.src)
        .map_err(|e| e.to_string())
}

/// PoB's tree for `version`, read once per run.
pub fn layout(app: &AppHandle, version: Option<&str>) -> Result<Arc<TreeLayout>, String> {
    let src_dir = pob_src(app)?;
    let file = tree::tree_file(&src_dir, version).map_err(|e| e.to_string())?;
    let mut cache = LAYOUTS.lock().unwrap_or_else(|p| p.into_inner());
    let cache = cache.get_or_insert_with(HashMap::new);
    if let Some(l) = cache.get(&file) {
        return Ok(l.clone());
    }
    let src = std::fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))?;
    // A new league can change the tree data's shape before Exile Companion is updated for it.
    let l = TreeLayout::from_lua(&src).map_err(|e| {
        log::error!("passive tree {}: {e}", file.display());
        let version = file
            .parent()
            .and_then(|d| d.file_name())
            .map(|v| v.to_string_lossy().replace('_', "."))
            .unwrap_or_default();
        format!(
            "This version of Exile Companion can't read Path of Building's {version} passive tree. \
             The rest of the app works, and an update of Exile Companion will fix it."
        )
    })?;
    // Without it the tree still draws, only without the passives cluster jewels add.
    let clusters = src_dir.join("Data").join("ClusterJewels.lua");
    let l = Arc::new(
        match std::fs::read_to_string(&clusters)
            .map_err(|e| e.to_string())
            .and_then(|s| tree::ClusterData::from_lua(&s).map_err(|e| e.to_string()))
        {
            Ok(data) => l.with_cluster_data(data),
            Err(e) => {
                log::warn!("cluster jewels {}: {e}", clusters.display());
                l
            }
        },
    );
    log::info!("passive tree {} read", file.display());
    cache.insert(file, l.clone());
    Ok(l)
}

/// Runs `f`, and turns a panic in it into an error the window can show. The tree commands run on
/// the main thread, where a panic on data a new league brings would take the whole app down.
pub fn guarded<T>(f: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).unwrap_or_else(|_| {
        Err(
            "The passive tree hit something this version of Exile Companion doesn't know. \
             The rest of the app works. The log has the details."
                .to_string(),
        )
    })
}

/// Stage `stage` of the guide, or the one the character's level is at, in the saved mode.
pub fn stage(app: &AppHandle, stage: Option<usize>) -> Result<Option<TreeStage>, String> {
    let Some(guide) = crate::runtime::plan_source(app) else {
        return Ok(None);
    };
    let state = app.state::<AppState>();
    let stage = stage.unwrap_or_else(|| tree::stage_for(&guide, state.current_state().level));
    let version = guide.specs.get(stage).and_then(|s| s.tree_version.clone());
    let layout = layout(app, version.as_deref())?;
    // Without a followed guide the tree is the character's own, nothing to compare it with.
    let followed = state
        .guide
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .is_some();
    let character = followed
        .then(|| {
            let builds = state.build.lock().unwrap_or_else(|p| p.into_inner());
            builds.build().cloned()
        })
        .flatten();
    let mode = settings::load(app).unwrap_or_default().tree_mode;
    let mut out = tree::tree_stage(&layout, &guide, stage, character.as_ref(), mode);
    out.latest = tree::latest_version(&pob_src(app)?)
        .ok()
        .filter(|v| Some(v) != out.version.as_ref());
    if out.missing > 0 {
        if let Some(theirs) = out
            .character_version
            .clone()
            .and_then(|v| self::layout(app, Some(&v)).ok())
        {
            out.name_missing(&theirs);
        }
    }
    Ok(Some(out))
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_panic_becomes_an_error_the_window_shows() {
        let out: Result<(), String> = super::guarded(|| panic!("index out of bounds"));
        assert!(out.unwrap_err().contains("doesn't know"));
        assert_eq!(super::guarded(|| Ok(3)), Ok(3));
    }
}
