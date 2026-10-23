//! Where Path of Building lives on this machine. The build assistant runs its calculations.

use std::path::{Path, PathBuf};

use exile_pob_engine::{locate, EngineError, PobDir};
use serde::Serialize;
use tauri::AppHandle;

use crate::settings;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PobInfo {
    pub path: String,
    pub version: Option<String>,
}

impl From<&PobDir> for PobInfo {
    fn from(dir: &PobDir) -> Self {
        Self {
            path: dir.root.to_string_lossy().into_owned(),
            version: dir.version.clone(),
        }
    }
}

/// The saved PoB folder if it's still there, otherwise the first one found. A new find is saved.
pub fn find(app: &AppHandle) -> Option<PobInfo> {
    let mut saved = settings::load(app).unwrap_or_default();
    let dir = locate::find_pob(saved.pob_path.as_deref().map(Path::new), &registry_dirs())?;
    let info = PobInfo::from(&dir);
    if saved.pob_path.as_deref() != Some(info.path.as_str()) {
        log::info!("Path of Building found at {}", info.path);
        saved.pob_path = Some(info.path.clone());
        let _ = settings::save(app, &saved);
    }
    Some(info)
}

/// A folder the player picked. Saved only if it holds PoB for Path of Exile 1.
pub fn choose(app: &AppHandle, dir: &Path) -> Result<PobInfo, String> {
    let dir = PobDir::detect(dir).map_err(|e| match e {
        EngineError::Poe2(_) => {
            "That's Path of Building for Path of Exile 2. Pick the one for Path of Exile 1."
                .to_string()
        }
        EngineError::NotPob(_) => {
            "There's no Path of Building in that folder. Pick the folder with Path of Building.exe."
                .to_string()
        }
        e => e.to_string(),
    })?;
    let info = PobInfo::from(&dir);
    let mut saved = settings::load(app).unwrap_or_default();
    saved.pob_path = Some(info.path.clone());
    settings::save(app, &saved)?;
    Ok(info)
}

#[cfg(windows)]
fn registry_dirs() -> Vec<PathBuf> {
    win::uninstall_entries()
        .into_iter()
        .filter(|e| {
            e.display_name
                .as_deref()
                .is_some_and(locate::is_pob1_display_name)
        })
        .filter_map(|e| {
            locate::install_dir_from_uninstall_entry(
                e.install_location.as_deref(),
                e.uninstall_string.as_deref(),
                e.display_icon.as_deref(),
            )
        })
        .collect()
}

#[cfg(not(windows))]
fn registry_dirs() -> Vec<PathBuf> {
    Vec::new()
}

#[cfg(windows)]
mod win {
    use windows_sys::Win32::Foundation::ERROR_SUCCESS;
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegEnumKeyExW, RegGetValueW, RegOpenKeyExW, HKEY, HKEY_CURRENT_USER,
        HKEY_LOCAL_MACHINE, KEY_READ, RRF_RT_REG_SZ,
    };

    const UNINSTALL: &str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall";

    pub struct Entry {
        pub display_name: Option<String>,
        pub install_location: Option<String>,
        pub uninstall_string: Option<String>,
        pub display_icon: Option<String>,
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// Every program in the per-user and per-machine uninstall lists.
    pub fn uninstall_entries() -> Vec<Entry> {
        let mut entries = Vec::new();
        for root in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
            let mut key: HKEY = std::ptr::null_mut();
            let path = wide(UNINSTALL);
            if unsafe { RegOpenKeyExW(root, path.as_ptr(), 0, KEY_READ, &mut key) } != ERROR_SUCCESS
            {
                continue;
            }
            let mut index = 0;
            loop {
                let mut name = [0u16; 256];
                let mut len = name.len() as u32;
                let status = unsafe {
                    RegEnumKeyExW(
                        key,
                        index,
                        name.as_mut_ptr(),
                        &mut len,
                        std::ptr::null(),
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                    )
                };
                if status != ERROR_SUCCESS {
                    break;
                }
                index += 1;
                let sub = &name[..len as usize];
                let value = |v: &str| read_string(key, sub, v);
                entries.push(Entry {
                    display_name: value("DisplayName"),
                    install_location: value("InstallLocation"),
                    uninstall_string: value("UninstallString"),
                    display_icon: value("DisplayIcon"),
                });
            }
            unsafe { RegCloseKey(key) };
        }
        entries
    }

    fn read_string(key: HKEY, sub: &[u16], value: &str) -> Option<String> {
        let sub: Vec<u16> = sub.iter().copied().chain(std::iter::once(0)).collect();
        let value = wide(value);
        let mut buf = [0u16; 1024];
        let mut size = std::mem::size_of_val(&buf) as u32;
        let status = unsafe {
            RegGetValueW(
                key,
                sub.as_ptr(),
                value.as_ptr(),
                RRF_RT_REG_SZ,
                std::ptr::null_mut(),
                buf.as_mut_ptr().cast(),
                &mut size,
            )
        };
        if status != ERROR_SUCCESS {
            return None;
        }
        let chars = (size as usize / 2).saturating_sub(1);
        Some(String::from_utf16_lossy(&buf[..chars]))
    }
}
