//! Raw API answers on disk, one file per request, expired by modification time.

use std::path::Path;
use std::time::{Duration, SystemTime};

/// FNV-1a of the request. std's hashers don't promise the same output across Rust releases, and
/// the cache outlives an app update.
pub fn key(api: &str, params: &[(&str, &str)]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut feed = |bytes: &[u8]| {
        for b in bytes {
            h ^= u64::from(*b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    feed(api.as_bytes());
    for (k, v) in params {
        feed(b"&");
        feed(k.as_bytes());
        feed(b"=");
        feed(v.as_bytes());
    }
    format!("{h:016x}.json")
}

pub fn read(dir: &Path, key: &str, ttl: Duration) -> Option<String> {
    let path = dir.join(key);
    let age = SystemTime::now()
        .duration_since(std::fs::metadata(&path).ok()?.modified().ok()?)
        .unwrap_or_default();
    if age >= ttl {
        return None;
    }
    std::fs::read_to_string(path).ok()
}

pub fn write(dir: &Path, key: &str, body: &str) {
    if std::fs::create_dir_all(dir).is_ok() {
        let _ = std::fs::write(dir.join(key), body);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_differ_by_params_and_stay_stable() {
        let a = key("https://x/api.php", &[("titles", "Withered")]);
        assert_eq!(a, key("https://x/api.php", &[("titles", "Withered")]));
        assert_ne!(a, key("https://x/api.php", &[("titles", "Wither")]));
        assert_eq!(a.len(), "0123456789abcdef.json".len());
    }

    #[test]
    fn keeps_answers_until_they_expire() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "k.json", "{}");
        assert_eq!(
            read(dir.path(), "k.json", Duration::from_secs(60)).as_deref(),
            Some("{}")
        );
        assert_eq!(read(dir.path(), "k.json", Duration::ZERO), None);
        assert_eq!(
            read(dir.path(), "missing.json", Duration::from_secs(60)),
            None
        );
    }
}
