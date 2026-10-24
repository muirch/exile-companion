//! Telling the game's window from any other, by its window class and title.

/// Window classes of Path of Exile 1 and 2: the Windows class, the X11 class Steam gives a Proton
/// game (steam_app_<app id>), and the exe name Wine uses for the standalone client.
const GAME_CLASSES: [&str; 6] = [
    "POEWindowClass",
    "steam_app_238960",
    "steam_app_2694490",
    "pathofexile.exe",
    "pathofexile_x64.exe",
    "pathofexilesteam.exe",
];
const GAME_TITLES: [&str; 2] = ["Path of Exile", "Path of Exile 2"];

pub fn is_game_window(class: &str, title: &str) -> bool {
    // Exact title: a browser tab titled "Path of Exile Wiki" must not count.
    GAME_CLASSES.iter().any(|c| c.eq_ignore_ascii_case(class)) || GAME_TITLES.contains(&title)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn knows_the_game_on_windows_and_under_proton() {
        assert!(is_game_window("POEWindowClass", ""));
        assert!(is_game_window("steam_app_238960", "Path of Exile"));
        assert!(is_game_window("PathOfExile_x64.exe", ""));
        assert!(is_game_window("", "Path of Exile 2"));
        assert!(!is_game_window(
            "firefox",
            "Path of Exile Wiki - Mozilla Firefox"
        ));
        assert!(!is_game_window("steam", "Steam"));
    }
}
