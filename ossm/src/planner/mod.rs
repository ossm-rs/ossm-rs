mod linear;
mod ruckig;

pub use linear::LinearPlanner;
pub use ruckig::RuckigPlanner;

/// Output from a single planner tick.
#[derive(Debug, Clone, Copy)]
pub struct PlannerOutput {
    /// Current position (0.0–1.0).
    pub position: f64,
    /// Current velocity (fraction/s).
    pub velocity: f64,
    /// Current acceleration (fraction/s²).
    pub acceleration: f64,
    /// Whether the trajectory has reached its target.
    pub finished: bool,
}

/// Trajectory planner operating in the 0–1 domain.
pub trait Planner {
    /// Set the current position without starting motion.
    fn set_position(&mut self, position: f64);

    /// Set a new motion target.
    ///
    /// `position` is in 0–1. `velocity_fraction` is a multiplier on the
    /// planner's configured max velocity (0.0–1.0).
    fn set_target(&mut self, position: f64, velocity_fraction: f64, jerk_fraction: f64);

    /// Advance one timestep and return the current state.
    fn tick(&mut self) -> PlannerOutput;

    /// Whether a trajectory is in progress.
    fn is_moving(&self) -> bool;

    /// Current position (0.0–1.0).
    fn position(&self) -> f64;

    /// Reset to idle at position 0.
    fn home(&mut self) {
        self.set_position(0.0);
    }
}
