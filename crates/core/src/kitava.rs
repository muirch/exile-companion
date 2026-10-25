//! The zones where the player meets Kitava, and the resistance penalty after each fight.

/// The penalty in percent once the Kitava fought in `area_id` is dead: -30 after act 5, -60
/// after act 10. None for every other area.
pub fn penalty_after(area_id: &str) -> Option<i32> {
    match area_id {
        // The Cathedral Rooftop, act 5.
        "1_5_8" => Some(-30),
        // The Feeding Trough, act 10.
        "2_10_6" => Some(-60),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_kitava_fights() {
        assert_eq!(penalty_after("1_5_8"), Some(-30));
        assert_eq!(penalty_after("2_10_6"), Some(-60));
        // Act 10 starts on a Cathedral Rooftop too, Kitava isn't there.
        assert_eq!(penalty_after("2_10_1"), None);
    }
}
