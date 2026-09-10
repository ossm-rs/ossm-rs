use log::info;

use crate::{
    pattern::{MAX_SENSATION, MIN_SENSATION},
    utils::scale,
};

use super::{Pattern, PatternInput, PatternMove};

const MAX_STROKES: usize = 5;
const MIN_DELAY_MS: f64 = 100.0;
const MAX_DELAY_MS: f64 = 10000.0;

#[derive(Default)]
pub struct StopNGo {
    out_stroke: bool,
    num_strokes: usize,
    current_stroke: usize,
    counting_up: bool,
    previous_sensation: f64,
}

impl StopNGo {
    pub fn new() -> Self {
        let mut pattern = Self::default();
        pattern.reset();
        pattern
    }
}

impl Pattern for StopNGo {
    fn get_name(&self) -> &'static str {
        "Stop'n'Go"
    }

    fn get_description(&self) -> &'static str {
        "Stops after a series of strokes. Sensation controls the delay between each series"
    }

    fn reset(&mut self) {
        self.out_stroke = true;
        self.num_strokes = 1;
        self.current_stroke = 1;
        self.counting_up = true;
        self.previous_sensation = 0.0;
    }

    fn next_move(&mut self, input: &PatternInput) -> PatternMove {
        // Reset the strokes if sensation changes
        if input.sensation != self.previous_sensation {
            self.num_strokes = 1;
            self.current_stroke = 1;
            self.previous_sensation = input.sensation;
        }

        let new_move = if self.out_stroke {
            PatternMove::new(input.velocity, input.depth)
        } else {
            let in_stroke_depth = input.depth - input.motion_length;
            let mut delay_ms = 0;
            // On the last stroke
            if self.current_stroke == self.num_strokes {
                info!("Stroke series with {} strokes complete", self.num_strokes);
                delay_ms = scale(
                    input.sensation,
                    MIN_SENSATION,
                    MAX_SENSATION,
                    MIN_DELAY_MS,
                    MAX_DELAY_MS,
                ) as u64;

                // Switch direction when reaching the end
                if self.num_strokes == 1 {
                    self.counting_up = true;
                }
                if self.num_strokes == MAX_STROKES {
                    self.counting_up = false;
                }

                if self.counting_up {
                    self.num_strokes += 1;
                } else {
                    self.num_strokes -= 1;
                }

                self.current_stroke = 0;
            }
            self.current_stroke += 1;
            PatternMove::new_with_delay(input.velocity, in_stroke_depth, delay_ms)
        };

        self.out_stroke = !self.out_stroke;

        new_move
    }
}
