//! The experience penalty for fighting monsters far from the character's level, Path of Exile 1's
//! formula as the wiki has it.

use crate::data::ZoneInfo;

/// The share of experience a character of `char_level` gets in an area of `area_level`, 0.01 to 1.
/// None from level 95, where an extra penalty applies that a leveling overlay doesn't need.
pub fn multiplier(char_level: u32, area_level: u32) -> Option<f64> {
    if char_level == 0 || char_level >= 95 {
        return None;
    }
    let player = f64::from(char_level);
    let mut area = f64::from(area_level);
    // Above 70 the game counts a lower effective monster level.
    if area_level > 70 {
        area = -0.03 * area * area + 5.17 * area - 144.9;
    }
    let safe = (3.0 + player / 16.0).floor();
    let diff = ((player - area).abs() - safe).max(0.0);
    let m = ((player + 5.0) / (player + 5.0 + diff.powf(2.5))).powf(1.5);
    Some(m.max(0.01))
}

/// For the area the character stands in. Towns and hideouts have no monsters, so None.
pub fn in_zone(char_level: u32, zone: &ZoneInfo) -> Option<f64> {
    if zone.is_town || zone.is_hideout {
        return None;
    }
    multiplier(char_level, u32::try_from(zone.area_level).ok()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_experience_inside_the_safe_zone() {
        // Level 10: 3 + 10/16 = 3 levels either way.
        assert_eq!(multiplier(10, 13), Some(1.0));
        assert_eq!(multiplier(10, 7), Some(1.0));
    }

    #[test]
    fn falls_off_outside_it() {
        let m = multiplier(10, 15).unwrap();
        // 2 levels past the safe zone: (15 / (15 + 2^2.5))^1.5
        let expected = (15.0f64 / (15.0 + 2f64.powf(2.5))).powf(1.5);
        assert!((m - expected).abs() < 1e-9, "{m}");
        assert!(m < 0.8);
    }

    #[test]
    fn never_below_one_percent() {
        assert_eq!(multiplier(80, 1), Some(0.01));
    }

    #[test]
    fn not_for_the_top_levels() {
        assert_eq!(multiplier(95, 83), None);
        assert_eq!(multiplier(0, 1), None);
    }
}
