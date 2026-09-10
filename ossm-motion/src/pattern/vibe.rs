use crate::{pattern::{MAX_SENSATION, MIN_SENSATION}, utils::scale};

use super::{Pattern, PatternInput, PatternMove};

const MIN_VIBE_MM: f64 = 1.0;
const MAX_VIBE_MM: f64 = 10.0;

#[derive(Default)]
enum VibeState {
    #[default]
    VibeOut,
    VibeIn,
    Move,
}

#[derive(Default)]
pub struct Vibe {
    out_stroke: bool,
    vibe_state: VibeState,
    current_depth: f64,
}

impl Vibe {
    pub fn new() -> Self {
        let mut pattern = Self::default();
        pattern.reset();
        pattern
    }
}

impl Pattern for Vibe {
    fn get_name(&self) -> &'static str {
        "Vibe Stroke"
    }

    fn get_description(&self) -> &'static str {
        "Vibe in and out. Sensation does nothing."
    }

    fn reset(&mut self) {
        self.out_stroke = true;
    }

    fn next_move(&mut self, input: &PatternInput) -> PatternMove {
        let vibe_amount = scale(input.sensation, MIN_SENSATION, MAX_SENSATION, MIN_VIBE_MM, MAX_VIBE_MM);

        // Restrict depth
        let in_stroke_depth = input.depth - input.motion_length;

        // How much to move after each vibration
        let move_amount = vibe_amount * 2.0;

        match self.vibe_state {
            VibeState::VibeOut => {
                self.current_depth += vibe_amount;
                self.vibe_state = VibeState::VibeIn;
            }
            VibeState::VibeIn => {
                self.current_depth -= vibe_amount;
                self.vibe_state = VibeState::Move;
            }
            VibeState::Move => {
                if self.out_stroke {
                    self.current_depth += move_amount;
                } else {
                    self.current_depth -= move_amount;
                }
                self.vibe_state = VibeState::VibeOut;
            }
        }

        if self.current_depth > input.depth {
            self.current_depth = input.depth;
            self.out_stroke = false;
        }
        if self.current_depth < in_stroke_depth {
            self.current_depth = in_stroke_depth;
            self.out_stroke = true;
        }

        PatternMove::new(input.velocity, self.current_depth)
    }
}
