//! Input model. Physical host mapping stays in the frontend.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct InputButtons {
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
    pub a: bool,
    pub b: bool,
    pub start: bool,
    pub select: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct InputState {
    pub held: InputButtons,
    pub pressed: InputButtons,
    pub released: InputButtons,
}

impl InputState {
    pub fn from_held(held: InputButtons) -> Self {
        Self {
            held,
            pressed: InputButtons::default(),
            released: InputButtons::default(),
        }
    }

    pub fn update_held(&mut self, held: InputButtons) {
        macro_rules! edge {
            ($($f:ident),* $(,)?) => {
                $( self.pressed.$f = held.$f && !self.held.$f;
                   self.released.$f = !held.$f && self.held.$f; )*
            };
        }
        edge!(up, down, left, right, a, b, start, select);
        self.held = held;
    }

    pub fn clear_edges(&mut self) {
        self.pressed = InputButtons::default();
        self.released = InputButtons::default();
    }
}
