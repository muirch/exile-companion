//! What the player reads when Path of Building fails a tool call. The bridge's own errors are
//! written for them. A Lua error from inside PoB, a dead engine or a lost server is not, and after
//! a league patch it usually means PoB changed under this version.

const RAW: [&str; 7] = [
    "attempt to ",
    ".lua:",
    "stack traceback",
    "the engine stopped",
    "panicked",
    "The tool failed",
    "bad argument",
];

pub fn for_player(text: &str) -> String {
    if RAW.iter().any(|r| text.contains(r)) || text.trim().is_empty() {
        "Path of Building answered something this version of Exile Companion doesn't expect. \
         A new Path of Building or league may need an update of Exile Companion."
            .to_string()
    } else {
        text.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_the_bridges_own_messages() {
        let own = "Path of Building couldn't read that item";
        assert_eq!(for_player(own), own);
    }

    #[test]
    fn hides_errors_from_inside_pob() {
        for raw in [
            "[string \"bridge\"]:236: attempt to index field 'calcsTab' (a nil value)",
            "the engine stopped:\nModules/Build.lua:12: something",
            "task 3 panicked",
            "The tool failed: connection closed",
            "",
        ] {
            assert!(
                for_player(raw).starts_with("Path of Building answered something"),
                "{raw}"
            );
        }
    }
}
