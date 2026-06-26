use winit::keyboard::{KeyCode, PhysicalKey};

pub struct KeyBindings {
    pub forward: KeyBinding,
    pub left: KeyBinding,
    pub backward: KeyBinding,
    pub right: KeyBinding,
    pub rise: KeyBinding,
    pub descent: KeyBinding,
}

impl KeyBindings {
    pub fn new() -> Self {
        Self {
            forward: KeyBinding::default(),
            backward: KeyBinding::default(),
            left: KeyBinding::default(),
            right: KeyBinding::default(),
            rise: KeyBinding::default(),
            descent: KeyBinding::default(),
        }
    }

    pub fn on_key_change<const T: bool>(&mut self, key: PhysicalKey) {
        if let PhysicalKey::Code(key) = key {
            match key {
                KeyCode::KeyW => self.forward.is_down = T,
                KeyCode::KeyA => self.left.is_down = T,
                KeyCode::KeyS => self.backward.is_down = T,
                KeyCode::KeyD => self.right.is_down = T,
                KeyCode::ShiftLeft => self.rise.is_down = T,
                KeyCode::ShiftRight => self.rise.is_down = T,
                KeyCode::Space => self.descent.is_down = T,
                _ => {}
            }
        }
    }
}

pub struct KeyBinding {
    pub is_down: bool,
}

impl Default for KeyBinding {
    fn default() -> Self {
        Self { is_down: false }
    }
}
