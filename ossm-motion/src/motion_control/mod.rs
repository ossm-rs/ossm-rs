pub mod debug;
pub mod motor;
pub mod timer;

use core::{
    panic,
    sync::atomic::{AtomicBool, Ordering},
};

use log::{debug, error, info};
use portable_atomic::{AtomicF64, AtomicU16};
use rsruckig::prelude::*;

use crate::{
    config::*,
    motion_control::{
        debug::{DebugOut, DummyDebugOut},
        motor::Motor,
        timer::{Duration, Instant, Timer},
    },
    utils::{saturate_range, scale},
};

static MOVE_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

// Whether to panic on the thresholds being exceeded by motion control
// If false the values will be capped to the allowed limits, but the execution will continue
const PANIC_ON_EXCEEEDED: bool = false;

const VELOCITY_UPDATE_COOLDOWN_MS: u64 = 30;

struct MotionControlStateStorage {
    position: AtomicF64,
    velocity: AtomicF64,
    torque: AtomicU16,
}

static MOTION_CONTROL_STATE_UPDATED: AtomicBool = AtomicBool::new(false);
static MOTION_CONTROL_STATE: MotionControlStateStorage = MotionControlStateStorage {
    position: AtomicF64::new(MIN_MOVE_MM),
    velocity: AtomicF64::new(MOTION_CONTROL_MIN_VELOCITY),
    torque: AtomicU16::new(0),
};

pub struct MotionControl<M: Motor, T: Timer, D: DebugOut> {
    motor: M,
    timer: T,
    debug: D,
    ruckig: Ruckig<1, ThrowErrorHandler>,
    input: InputParameter<1>,
    output: OutputParameter<1>,
    last_update: Instant,
    velocity_setpoint: f64,
    torque_setpoint: u16,
    last_velocity_update: Instant,
    last_motor_write: Instant,
}

impl<M: Motor, T: Timer> MotionControl<M, T, DummyDebugOut> {
    /// Initialises the MotionControl and allows the use of attached functions
    pub fn new(motor: M, timer: T) -> Self {
        Self::new_with_debug(motor, timer, DummyDebugOut::new())
    }
}

impl<M: Motor, T: Timer, D: DebugOut> MotionControl<M, T, D> {
    pub fn new_with_debug(motor: M, timer: T, debug: D) -> Self {
        info!("Motion Control Init");

        let mut input = InputParameter::new(None);

        input.current_position[0] = MIN_MOVE_MM;
        input.max_velocity[0] = MOTION_CONTROL_MIN_VELOCITY;
        input.max_acceleration[0] = MOTION_CONTROL_MAX_ACCELERATION;
        input.max_jerk[0] = MOTION_CONTROL_MAX_JERK;
        input.synchronization = Synchronization::None;
        input.duration_discretization = DurationDiscretization::Discrete;

        let now = timer.now();

        let motion_control = Self {
            motor,
            timer,
            debug,
            ruckig: Ruckig::<1, ThrowErrorHandler>::new(
                None,
                MOTION_CONTROL_LOOP_UPDATE_INTERVAL_MS as f64 / 1000.0,
            ),
            input,
            output: OutputParameter::new(None),
            last_update: now,
            velocity_setpoint: MOTION_CONTROL_MIN_VELOCITY,
            torque_setpoint: 0,
            last_velocity_update: now,
            last_motor_write: now,
        };

        motion_control
    }

    /// The handler that must be called every MOTION_CONTROL_LOOP_UPDATE_INTERVAL_MS
    pub fn update_handler(&mut self) {
        if MOTION_CONTROL_STATE_UPDATED.load(Ordering::Acquire) {
            MOTION_CONTROL_STATE_UPDATED.store(false, Ordering::Release);
            let position = MOTION_CONTROL_STATE.position.load(Ordering::Acquire) as f64;
            if position != self.input.target_position[0] {
                info!("Going to a new target position: {} mm", position);
                self.input.target_position[0] = position;
                self.output.time = 0.0;
            }

            let velocity = MOTION_CONTROL_STATE.velocity.load(Ordering::Acquire) as f64;
            if velocity != self.velocity_setpoint {
                self.velocity_setpoint = velocity;
                self.last_velocity_update = self.timer.now();
            }

            let torque = MOTION_CONTROL_STATE.torque.load(Ordering::Acquire);
            if torque != self.torque_setpoint {
                info!("Torque set to {}", torque);
                self.torque_setpoint = torque;
                self.motor
                    .set_max_allowed_output(torque as u16)
                    .expect("Failed to set max allowed output (torque)");
            }
        }

        if MOVE_IN_PROGRESS.load(Ordering::Acquire) {
            let start = self.timer.now();

            // Restrict how often the velocity can be updated
            // Updating it too often can lead to unstable motion
            if self.velocity_setpoint != self.input.max_velocity[0]
                && self.elapsed(self.last_velocity_update).to_millis() > VELOCITY_UPDATE_COOLDOWN_MS
            {
                self.input.max_velocity[0] = self.velocity_setpoint;
                self.output.time = 0.0;
                self.last_velocity_update = self.timer.now();
                info!("Set velocity to {} mm/s", self.velocity_setpoint);
            }

            let res = self.ruckig.update(&self.input, &mut self.output);

            let since_last = self.elapsed(self.last_update).to_micros();
            self.last_update = self.timer.now();

            match res {
                Ok(ok) => {
                    match ok {
                        RuckigResult::Working => {
                            let mut new_position = self.output.new_position[0];

                            // Saturate the position if out of bounds
                            let mut exceeded = false;
                            if new_position < MIN_MOVE_MM {
                                error!(
                                    "Motion control exceeded the min allowed move ({} < {})",
                                    new_position, MIN_MOVE_MM
                                );
                                new_position = MIN_MOVE_MM;
                                exceeded = true;
                            }

                            if new_position > MAX_MOVE_MM {
                                error!(
                                    "Motion control exceeded the max allowed move ({} > {})",
                                    new_position, MAX_MOVE_MM
                                );
                                new_position = MAX_MOVE_MM;
                                exceeded = true;
                            }

                            if exceeded && PANIC_ON_EXCEEEDED {
                                panic!("Motion control thresholds were exceeded. See above ^");
                            }

                            let mut new_steps = new_position * STEPS_PER_MM;
                            if !REVERSE_DIRECTION {
                                new_steps = -new_steps;
                            }

                            // Avoid writing to the motor too often to prevent a timeout
                            let since_last_motor_write = self.elapsed(self.last_motor_write);
                            if since_last_motor_write < M::min_consecutive_write_delay() {
                                self.motor.delay(
                                    M::min_consecutive_write_delay() - since_last_motor_write,
                                );
                            }

                            if let Err(err) = self.motor.set_absolute_position(new_steps as i32) {
                                error!("Failed to set motor position {:?}", err);
                            }
                            self.last_motor_write = self.timer.now();

                            debug!("Set motor position to {} mm", new_position);

                            self.debug.new_position(new_position);
                            self.debug.new_velocity(self.output.new_velocity[0]);
                            self.debug.new_acceleration(self.output.new_acceleration[0]);
                            self.debug.new_jerk(self.output.new_jerk[0]);

                            self.output.pass_to_input(&mut self.input);
                        }
                        RuckigResult::Finished => {
                            MOVE_IN_PROGRESS.store(false, Ordering::Release);
                        }
                        _ => {
                            error!("Error!");
                        }
                    }
                }
                Err(err) => {
                    error!("Ruckig Error {:?}", err);
                }
            }

            let duration_ms = self.elapsed(start).to_millis();

            debug!(
                "Update took: {} ms Since last call: {} us",
                duration_ms, since_last
            );

            if duration_ms > MOTION_CONTROL_LOOP_UPDATE_INTERVAL_MS {
                error!(
                    "Update took longer than the update interval {} > {}",
                    duration_ms, MOTION_CONTROL_LOOP_UPDATE_INTERVAL_MS
                );
            }
        } else {
            self.debug.new_position(self.output.new_position[0]);
            self.debug.new_velocity(self.output.new_velocity[0]);
            self.debug.new_acceleration(self.output.new_acceleration[0]);
            self.debug.new_jerk(self.output.new_jerk[0]);
        }
    }

    pub fn elapsed(&mut self, since: Instant) -> Duration {
        self.timer.now() - since
    }
}

pub fn is_move_in_progress() -> bool {
    MOVE_IN_PROGRESS.load(Ordering::Acquire)
}

pub fn set_target_position(position: f64) {
    MOTION_CONTROL_STATE
        .position
        .store(position, Ordering::Release);
    MOTION_CONTROL_STATE_UPDATED.store(true, Ordering::Release);

    if !MOVE_IN_PROGRESS.load(Ordering::Acquire) {
        MOVE_IN_PROGRESS.store(true, Ordering::Release);
    }
}

/// Set the maximum velocity for the move
pub fn set_max_velocity(mut max_velocity: f64) {
    // A velocity of 0 breaks motion control
    // Set some small minimum velocity
    if max_velocity < MOTION_CONTROL_MIN_VELOCITY {
        max_velocity = MOTION_CONTROL_MIN_VELOCITY;
    }
    if max_velocity > MOTION_CONTROL_MAX_VELOCITY {
        error!(
            "Velocity {} is larger than allowed {}",
            max_velocity, MOTION_CONTROL_MAX_VELOCITY
        );
        max_velocity = MOTION_CONTROL_MAX_VELOCITY;
    }

    MOTION_CONTROL_STATE
        .velocity
        .store(max_velocity, Ordering::Release);
    MOTION_CONTROL_STATE_UPDATED.store(true, Ordering::Release);
}

/// Set the maximum velocity based on the ratio between the
/// current value in MOTION_STATE and the actual current motor velocity
/// (velocity sent by the remote and the velocity set by the pattern)
///
/// This is to ensure that the updated velocity sent to motion control
/// follows the velocity scaling done by the pattern
pub fn set_max_velocity_scaled(current_velocity: f64, new_max_velocity: f64) {
    let velocity_setpoint = MOTION_CONTROL_STATE.velocity.load(Ordering::Acquire) as f64;
    let ratio = velocity_setpoint / current_velocity;
    let scaled_velocity = new_max_velocity * ratio;

    set_max_velocity(scaled_velocity);
}

/// Set the maximum torque for the move in %
pub fn set_torque(max_torque: f64) {
    let mut torque = saturate_range(max_torque, 0.0, 100.0);
    torque = scale(torque, 0.0, 100.0, MOTOR_MIN_OUTPUT, MOTOR_MAX_OUTPUT);
    // TODO: Refactor to not depend on the specific motor
    // The last digit is 0 for no alarm
    torque = torque * 10.0;

    let torque = torque as u16;

    MOTION_CONTROL_STATE.torque.store(torque, Ordering::Release);
    MOTION_CONTROL_STATE_UPDATED.store(true, Ordering::Release);
}
