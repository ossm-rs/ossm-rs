use rsruckig::prelude::*;
use num_traits::float::Float;

use super::{Planner, PlannerOutput};

const MIN_VELOCITY: f64 = 0.001;

/// Jerk-limited trajectory planner backed by Ruckig.
pub struct RuckigPlanner {
    ruckig: Ruckig<1, ThrowErrorHandler>,
    input: InputParameter<1>,
    output: OutputParameter<1>,
    max_velocity: f64,
    max_jerk: f64,
    moving: bool,
}

impl RuckigPlanner {
    /// Create a new Ruckig-backed planner.
    ///
    /// All limits are in the 0–1 domain (fraction/s, fraction/s², fraction/s³).
    /// `timestep_secs` is the fixed interval between `tick()` calls.
    pub fn new(
        max_velocity: f64,
        max_acceleration: f64,
        max_jerk: f64,
        timestep_secs: f64,
    ) -> Self {
        let mut input = InputParameter::new(None);
        input.current_position[0] = 0.0;
        input.target_position[0] = 0.0;
        input.max_velocity[0] = MIN_VELOCITY;
        input.max_acceleration[0] = max_acceleration;
        input.max_jerk[0] = max_jerk;
        input.synchronization = Synchronization::None;
        input.duration_discretization = DurationDiscretization::Discrete;

        Self {
            ruckig: Ruckig::<1, ThrowErrorHandler>::new(None, timestep_secs),
            input,
            output: OutputParameter::new(None),
            max_velocity,
            max_jerk,
            moving: false,
        }
    }
}

impl Planner for RuckigPlanner {
    fn set_position(&mut self, position: f64) {
        let position = position.clamp(0.0, 1.0);
        self.input.current_position[0] = position;
        self.input.target_position[0] = position;
        self.input.current_velocity[0] = 0.0;
        self.input.current_acceleration[0] = 0.0;
        self.output.new_position[0] = position;
        self.output.new_velocity[0] = 0.0;
        self.output.new_acceleration[0] = 0.0;
        self.moving = false;
    }

    fn set_target(&mut self, position: f64, velocity_fraction: f64, mut jerk_fraction: f64) {
        if velocity_fraction <= 0.0 {
            return;
        }

        let position = position.clamp(0.0, 1.0);
        let velocity = (velocity_fraction * self.max_velocity).clamp(MIN_VELOCITY, self.max_velocity);

        let speed_3 = 2.0 * velocity.powf(3.0);
        let max_jerk = speed_3 / 0.066.powf(2.0);
        let rail_2 = 1.0;
        let min_jerk= speed_3 / rail_2;
        jerk_fraction = 1.0 - jerk_fraction;
        jerk_fraction = jerk_fraction.powf(0.5);
        jerk_fraction = 1.0 - jerk_fraction;
        jerk_fraction = jerk_fraction.powf(2.0);
        let mm_s3 = jerk_fraction * max_jerk + min_jerk;
        self.input.max_jerk[0] = mm_s3.clamp(1.0, self.max_jerk);

        self.input.target_position[0] = position;
        self.input.max_velocity[0] = velocity;
        self.output.time = 0.0;
        self.ruckig.reset();
        self.moving = true;
    }

    fn tick(&mut self) -> PlannerOutput {
        if !self.moving {
            return PlannerOutput {
                position: self.input.current_position[0],
                velocity: 0.0,
                acceleration: 0.0,
                finished: true,
            };
        }

        let result = self.ruckig.update(&self.input, &mut self.output);

        let finished = matches!(result, Ok(RuckigResult::Finished));
        let output = PlannerOutput {
            position: self.output.new_position[0].clamp(0.0, 1.0),
            velocity: self.output.new_velocity[0],
            acceleration: self.output.new_acceleration[0],
            finished,
        };

        self.output.pass_to_input(&mut self.input);

        if finished {
            self.moving = false;
        }

        output
    }

    fn is_moving(&self) -> bool {
        self.moving
    }

    fn position(&self) -> f64 {
        self.input.current_position[0]
    }
}
