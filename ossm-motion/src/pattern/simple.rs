use super::{Pattern, PatternInput, PatternMove};

#[derive(Default)]
pub struct Simple {
    out_stroke: bool,
}

impl Simple {
    pub fn new() -> Self {
        let mut pattern = Self::default();
        pattern.reset();
        pattern
    }
}

impl Pattern for Simple {
    fn get_name(&self) -> &'static str {
        "Simple Stroke"
    }

    fn get_description(&self) -> &'static str {
        "Simple in and out. Sensation does nothing."
    }

    fn reset(&mut self) {
        self.out_stroke = true;
    }

    fn next_move(&mut self, input: &PatternInput) -> PatternMove {
        let new_move = if self.out_stroke {
            PatternMove::new(input.velocity, input.depth)
        } else {
            let in_stroke_depth = input.depth - input.motion_length;
            PatternMove::new(input.velocity, in_stroke_depth)
        };
        self.out_stroke = !self.out_stroke;

        new_move
    }
}
