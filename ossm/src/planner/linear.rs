use super::{Planner, PlannerOutput};

/// Linear planner that lerps to the target at a constant velocity.
///
/// No acceleration or jerk limiting — moves in a straight line from
/// current position to target. Useful for testing pattern logic
/// without trajectory smoothing.
pub struct LinearPlanner {
    position: f64,
    target: f64,
    velocity: f64,
    max_velocity: f64,
    timestep_secs: f64,
    moving: bool,
}

impl LinearPlanner {
    /// Create a new linear planner.
    ///
    /// `max_velocity` is in fraction/s. `timestep_secs` is the fixed
    /// interval between `tick()` calls.
    pub fn new(max_velocity: f64, timestep_secs: f64) -> Self {
        Self {
            position: 0.0,
            target: 0.0,
            velocity: 0.0,
            max_velocity,
            timestep_secs,
            moving: false,
        }
    }
}

impl Planner for LinearPlanner {
    fn set_position(&mut self, position: f64) {
        self.position = position.clamp(0.0, 1.0);
        self.target = self.position;
        self.velocity = 0.0;
        self.moving = false;
    }

    fn set_target(&mut self, position: f64, velocity_fraction: f64, jerk_fraction: f64) {
        if velocity_fraction <= 0.0 || jerk_fraction < 0.0 {
            return;
        }

        self.target = position.clamp(0.0, 1.0);
        self.velocity = (velocity_fraction * self.max_velocity).clamp(0.0, self.max_velocity);
        self.moving = self.target != self.position;
    }

    fn tick(&mut self) -> PlannerOutput {
        if !self.moving {
            return PlannerOutput {
                position: self.position,
                velocity: 0.0,
                acceleration: 0.0,
                finished: true,
            };
        }

        let distance = self.target - self.position;
        let step = self.velocity * self.timestep_secs;

        let finished = step >= distance.abs();
        if finished {
            self.position = self.target;
            self.velocity = 0.0;
            self.moving = false;
        } else {
            self.position += step * distance.signum();
        }

        PlannerOutput {
            position: self.position,
            velocity: if finished { 0.0 } else { self.velocity * distance.signum() },
            acceleration: 0.0,
            finished,
        }
    }

    fn is_moving(&self) -> bool {
        self.moving
    }

    fn position(&self) -> f64 {
        self.position
    }
}
