//! Long-press detection for the quick menu key, fed by a keyboard hook that only watches keys.

/// Tracks one key. Key repeat sends more key-downs while the key is held, so only a fresh press
/// starts a hold. A key-up the hook never saw is recovered on the next real press.
#[derive(Debug, Default)]
pub struct HoldKey {
    held: bool,
    press: u64,
}

impl HoldKey {
    pub const fn new() -> Self {
        Self {
            held: false,
            press: 0,
        }
    }

    /// A key-down. `was_down` is whether the key was already physically down before this event
    /// (true for key repeat). Returns the id of a new press, if this is one.
    pub fn down(&mut self, was_down: bool) -> Option<u64> {
        if self.held && was_down {
            return None;
        }
        self.held = true;
        self.press += 1;
        Some(self.press)
    }

    pub fn up(&mut self) {
        self.held = false;
    }

    /// Whether the press `id` is still the current one and the key hasn't been released.
    pub fn still_held(&self, id: u64) -> bool {
        self.held && self.press == id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_repeat_keeps_the_first_press() {
        let mut k = HoldKey::default();
        let id = k.down(false).unwrap();
        assert_eq!(k.down(true), None);
        assert_eq!(k.down(true), None);
        assert!(k.still_held(id));
    }

    #[test]
    fn release_ends_the_hold() {
        let mut k = HoldKey::default();
        let id = k.down(false).unwrap();
        k.up();
        assert!(!k.still_held(id));
    }

    #[test]
    fn a_new_press_replaces_an_old_one() {
        let mut k = HoldKey::default();
        let first = k.down(false).unwrap();
        k.up();
        let second = k.down(false).unwrap();
        assert!(!k.still_held(first));
        assert!(k.still_held(second));
    }

    #[test]
    fn a_missed_key_up_does_not_block_the_next_press() {
        let mut k = HoldKey::default();
        let first = k.down(false).unwrap();
        // The key-up never reached the hook, but the key is physically up again.
        let second = k.down(false).expect("fresh press after a lost key-up");
        assert_ne!(first, second);
        assert!(k.still_held(second));
        assert!(!k.still_held(first));
    }
}
