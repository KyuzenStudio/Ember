use std::collections::HashSet;

#[derive(Clone, Debug, Default)]
pub struct InputState {
    down: HashSet<i32>,
    pressed: HashSet<i32>,
}

impl InputState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn press(&mut self, key: i32) {
        self.down.insert(key);
        self.pressed.insert(key);
    }

    pub fn press_down(&mut self, key: i32) {
        self.down.insert(key);
    }

    pub fn release(&mut self, key: i32) {
        self.down.remove(&key);
    }

    pub fn clear(&mut self) {
        self.down.clear();
        self.pressed.clear();
    }

    pub fn is_down(&self, key: i32) -> bool {
        self.down.contains(&key)
    }

    pub fn take_pressed(&mut self) -> HashSet<i32> {
        std::mem::take(&mut self.pressed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn press_and_release() {
        let mut input = InputState::new();
        assert!(!input.is_down(3));
        input.press(3);
        assert!(input.is_down(3));
        input.release(3);
        assert!(!input.is_down(3));
    }

    #[test]
    fn clear_empties() {
        let mut input = InputState::new();
        input.press(1);
        input.press(2);
        input.clear();
        assert!(!input.is_down(1));
    }

    #[test]
    fn pressed_edge_consumed_once() {
        let mut input = InputState::new();
        input.press_down(3);
        assert!(!input.take_pressed().contains(&3));
        input.press(3);
        assert!(input.take_pressed().contains(&3));
        assert!(!input.take_pressed().contains(&3));
        assert!(input.is_down(3));
    }
}
