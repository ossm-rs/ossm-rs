use crate::{
    pattern::{MAX_SENSATION, MIN_SENSATION},
    utils::scale,
};

use super::{Pattern, PatternInput, PatternMove};

#[derive(Default)]
pub struct Torque {
    out_stroke: bool,
}

impl Torque {
    pub fn new() -> Self {
        let mut pattern = Self::default();
        pattern.reset();
        pattern
    }
}

impl Pattern for Torque {
    fn get_name(&self) -> &'static str {
        "Torque"
    }

    fn get_description(&self) -> &'static str {
        "Same as the simple pattern. Sensation controls the torque applied"
    }

    fn reset(&mut self) {
        self.out_stroke = true;
    }

    fn next_move(&mut self, input: &PatternInput) -> PatternMove {
        let torque = scale(input.sensation, MIN_SENSATION, MAX_SENSATION, 0.0, 100.0);

        let new_move = if self.out_stroke {
            PatternMove::new_with_torque(input.velocity, input.depth, torque)
        } else {
            let in_stroke_depth = input.depth - input.motion_length;
            PatternMove::new_with_torque(input.velocity, in_stroke_depth, torque)
        };
        self.out_stroke = !self.out_stroke;

        new_move
    }
}
