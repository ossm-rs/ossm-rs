use rsruckig::prelude::*;
use num_traits::float::Float;

use crate::command::{Cancelled, MotionCommand, StateCommand, StateResponse};
use crate::state::MotionPhase;
use crate::{Board, MotionLimits, Ossm};

// Floor applied to velocity requests to prevent degenerate Ruckig inputs.
const MIN_VELOCITY: f64 = 0.001;

#[derive(Debug, Clone, Copy, PartialEq)]
enum MotionState {
    Disabled,
    Enabled,
    Ready,
    Moving,
    /// Ruckig is decelerating to a smooth stop for the given reason.
    Stopping(StopReason),
    /// Motor is stationary; the instructed target is preserved for resume.
    Paused,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum StopReason {
    Pause,
    Disable,
    Home,
}

/// The last-commanded motion intent, independent of what ruckig is currently
/// planning. Pause/resume manipulates the ruckig input while leaving this
/// untouched.
#[derive(Debug, Clone, Copy)]
struct MotionTarget {
    /// Target position (mm).
    position: f64,
    /// Maximum velocity (mm/s).
    velocity: f64,
    /// Maximum acceleration (mm/s/s)
    jerk: f64,
    /// Torque limit as a fraction (0.0–1.0). `None` uses the motor default.
    torque: Option<f64>,
    /// True for the dedicated SexCode 10 ms streaming servo path.
    direct_stream: bool,
}

/// Drives the motion state machine and enforces safe motion profiles.
///
/// The controller owns a ruckig instance and generates jerk-limited
/// trajectories. Each tick, it samples the trajectory and calls
/// `board.set_position(mm)` with the next point on the curve. The board
/// is a dumb position follower — it never plans its own trajectory.
///
/// # Safety
///
/// Ruckig enforces the acceleration and jerk limits from [`MotionLimits`].
/// No upstream code (patterns, UI, remote) can cause motion that exceeds
/// these limits. The motor's internal trajectory planner is bypassed by
/// configuring it for maximum tracking speed.
pub struct MotionController<'a, B: Board> {
    board: B,
    channels: &'a Ossm,
    state: MotionState,
    limits: MotionLimits,
    /// The last-instructed motion target. `Some` when a move has been commanded,
    /// `None` when there is no active motion intent (e.g. disabled, just homed).
    target: Option<MotionTarget>,
    ruckig: Ruckig<1, ThrowErrorHandler>,
    input: InputParameter<1>,
    output: OutputParameter<1>,
    /// Fixed motion-loop period used by both Ruckig and the direct SexCode servo.
    update_interval_secs: f64,
}

impl<'a, B: Board> MotionController<'a, B> {
    /// Create a new `MotionController` in the `Disabled` state.
    ///
    /// `update_interval_secs` must match the ticker period the caller uses.
    /// Ruckig uses this as its fixed time step, so timing accuracy matters.
    pub(crate) fn new(
        board: B,
        limits: MotionLimits,
        update_interval_secs: f64,
        channels: &'a Ossm,
    ) -> Self {
        let mut input = InputParameter::new(None);
        input.current_position[0] = limits.min_position_mm;
        input.target_position[0] = limits.min_position_mm;
        input.max_velocity[0] = MIN_VELOCITY;
        input.max_acceleration[0] = limits.max_acceleration_mm_s2;
        input.max_jerk[0] = limits.max_jerk_mm_s3;
        input.synchronization = Synchronization::None;
        input.duration_discretization = DurationDiscretization::Discrete;

        Self {
            board,
            channels,
            state: MotionState::Disabled,
            limits,
            target: None,
            ruckig: Ruckig::<1, ThrowErrorHandler>::new(None, update_interval_secs),
            input,
            output: OutputParameter::new(None),
            update_interval_secs,
        }
    }

    /// Advance the motion control loop by one step.
    ///
    /// Returns `Err` if the board reports a critical fault. The caller should
    /// treat this as an unrecoverable error for this control cycle — the
    /// controller will have already transitioned to `Disabled`.
    pub async fn update(&mut self) -> Result<(), B::Error> {
        if let Err(e) = self.board.tick().await {
            log::error!("Board tick fault: {:?}", e);
            self.enter_fault();
            return Err(e);
        }

        self.tick().await?;

        if let Ok(cmd) = self.channels.state_cmd.try_receive() {
            self.process_state_command(cmd).await?;
        }

        if let Ok(cmd) = self.channels.move_cmd.try_receive() {
            self.process_move_command(cmd).await;
        }

        Ok(())
    }

    async fn process_state_command(&mut self, cmd: StateCommand) -> Result<(), B::Error> {
        match (&self.state, cmd) {
            (MotionState::Disabled, StateCommand::Enable) => {
                match self.board.enable().await {
                    Ok(()) => {
                        self.transition(MotionState::Enabled);
                        self.respond(StateResponse::Completed);
                    }
                    Err(e) => {
                        log::error!("Board enable failed: {:?}", e);
                        self.respond(StateResponse::Fault);
                        return Err(e);
                    }
                }
            }
            // Idempotent: already in the target state, nothing to do.
            // BLE remote RADR thrashes sometimes causing the catch-all
            // to trigger.
            (MotionState::Enabled, StateCommand::Enable)
            | (MotionState::Disabled, StateCommand::Disable) => {
                self.respond(StateResponse::Completed);
            }
            (MotionState::Enabled | MotionState::Ready, StateCommand::Disable) => {
                self.disable().await;
                self.respond(StateResponse::Completed);
            }
            (MotionState::Paused, StateCommand::Disable) => {
                self.channels.move_resp.signal(Err(Cancelled));
                self.disable().await;
                self.respond(StateResponse::Completed);
            }
            (MotionState::Moving, StateCommand::Disable) => {
                self.channels.move_resp.signal(Err(Cancelled));
                self.stop(StopReason::Disable);
            }
            (MotionState::Stopping(_), StateCommand::Disable) => {
                self.state = MotionState::Stopping(StopReason::Disable);
            }

            (MotionState::Enabled | MotionState::Ready, StateCommand::Home) => {
                match self.home().await {
                    Ok(()) => self.respond(StateResponse::Completed),
                    Err(e) => {
                        self.respond(StateResponse::Fault);
                        return Err(e);
                    }
                }
            }
            (MotionState::Moving, StateCommand::Home) => {
                self.channels.move_resp.signal(Err(Cancelled));
                self.stop(StopReason::Home);
            }
            (MotionState::Paused, StateCommand::Home) => {
                self.channels.move_resp.signal(Err(Cancelled));
                match self.home().await {
                    Ok(()) => self.respond(StateResponse::Completed),
                    Err(e) => {
                        self.respond(StateResponse::Fault);
                        return Err(e);
                    }
                }
            }

            (MotionState::Moving, StateCommand::Pause) => {
                self.stop(StopReason::Pause);
                self.respond(StateResponse::Completed);
            }

            (MotionState::Paused, StateCommand::Resume) => {
                self.resume().await;
                self.respond(StateResponse::Completed);
            }

            _ => {
                self.respond(StateResponse::InvalidTransition);
            }
        }

        Ok(())
    }

    async fn process_move_command(&mut self, cmd: MotionCommand) {
        match self.state {
            MotionState::Ready => {
                self.set_motion_target(cmd);
                self.apply_torque().await;
                self.transition(MotionState::Moving);
            }

            MotionState::Moving => {
                // SexCode is a live position servo.  The legacy pattern update gate
                // intentionally ignores retargets near the end of a point-to-point
                // move, but that causes slow slider motion to accumulate position
                // error and then catch up in a sudden high-speed run.  SexCode's
                // proportional speed law already bounds the catch-up velocity, so
                // accept its live retargets immediately.
                let leaving_direct_stream = self.target.as_ref()
                    .map(|t| t.direct_stream)
                    .unwrap_or(false) && !cmd.direct_stream;

                if cmd.direct_stream || leaving_direct_stream {
                    // Live SexCode targets are always accepted immediately. Also
                    // accept the first normal XToys/pattern command immediately
                    // when leaving SexCode mode so a stale direct-servo target can
                    // never keep control of the motion loop.
                    self.set_motion_target(cmd);
                    self.apply_torque().await;
                } else {
                    // Legacy XToys/pattern behaviour: only attempt to update when
                    // the existing move has enough time remaining, the current
                    // velocity limit is zero, or Ruckig is at time zero.
                    let remaining_time = self.output.trajectory.get_duration() - self.output.time;
                    if self.input.max_velocity[0] == 0.0 || remaining_time > 1.0 || self.output.time == 0.0 {
                        self.set_motion_target(cmd);
                        self.apply_torque().await;
                    }
                }
            }

            _ => {}
        }
    }

    /// Advance the active motion source by one controller tick.
    async fn tick(&mut self) -> Result<(), B::Error> {
        if !matches!(self.state, MotionState::Moving | MotionState::Stopping(_)) {
            return Ok(());
        }

        // SexCode is a continuously streamed position servo, not a sequence of
        // point-to-point trajectories. Sample the latest target at the native
        // 10 ms motion-loop rate and move toward it directly. This avoids the
        // repeated 5-15 ms Ruckig replans that caused visible micro-stutter.
        if matches!(self.state, MotionState::Moving)
            && self.target.as_ref().map(|t| t.direct_stream).unwrap_or(false)
        {
            return self.tick_direct_stream().await;
        }

        let result = match self.ruckig.update(&self.input, &mut self.output) {
            Ok(result) => result,
            Err(error) => {
                log::error!(
                    "Ruckig update error: {:?} state={:?} pos={:.3} vel={:.3} acc={:.3} target={:.3} vmax={:.3} amax={:.3} jmax={:.3}",
                    error,
                    self.state,
                    self.input.current_position[0],
                    self.input.current_velocity[0],
                    self.input.current_acceleration[0],
                    self.input.target_position[0],
                    self.input.max_velocity[0],
                    self.input.max_acceleration[0],
                    self.input.max_jerk[0],
                );

                // Never leave callers blocked forever on await_motion(). For an
                // ordinary point-to-point move, cancel the failed trajectory and
                // return to Ready so XToys streaming can consume the next queued
                // position command. SexCode direct streaming does not use Ruckig.
                if matches!(self.state, MotionState::Moving) {
                    self.target = None;
                    self.channels.move_resp.signal(Err(Cancelled));
                    self.transition(MotionState::Ready);
                }
                return Ok(());
            },
        };

        if !matches!(result, RuckigResult::Working | RuckigResult::Finished) {
            return Ok(());
        }

        let mm = self.output.new_position[0]
            .clamp(self.limits.min_position_mm, self.limits.max_position_mm);
        if let Err(e) = self.board.set_position(mm).await {
            log::error!("Board set_position failed: {:?}", e);
            self.enter_fault();
            return Err(e);
        }
        self.output.pass_to_input(&mut self.input);
        self.publish_state();

        if result == RuckigResult::Finished {
            match self.state {
                MotionState::Stopping(StopReason::Pause) => {
                    self.transition(MotionState::Paused);
                }
                MotionState::Stopping(StopReason::Disable) => {
                    self.disable().await;
                    self.respond(StateResponse::Completed);
                }
                MotionState::Stopping(StopReason::Home) => match self.home().await {
                    Ok(()) => self.respond(StateResponse::Completed),
                    Err(e) => {
                        self.respond(StateResponse::Fault);
                        return Err(e);
                    }
                },
                _ => {
                    self.target = None;
                    self.channels.move_resp.signal(Ok(()));
                    self.transition(MotionState::Ready);
                }
            }
        }

        Ok(())
    }

    /// Dedicated SexCode streaming servo. The latest target is sampled every
    /// motion tick (10 ms on ESP32-S3). Position is advanced directly through
    /// the normal board abstraction, with a conservative acceleration slew so
    /// hand-controlled slider motion remains responsive without becoming harsh.
    async fn tick_direct_stream(&mut self) -> Result<(), B::Error> {
        let Some(target) = self.target else { return Ok(()); };

        let dt = self.update_interval_secs;
        let current = self.input.current_position[0]
            .clamp(self.limits.min_position_mm, self.limits.max_position_mm);

        // Sexync uses speed=0 as release/hold. Preserve the v6.5.18 behavior:
        // stop advancing immediately at the current commanded position.
        if target.velocity <= 0.0 {
            if let Err(e) = self.board.set_position(current).await {
                log::error!("Board hold failed in SexCode servo: {:?}", e);
                self.enter_fault();
                return Err(e);
            }
            self.input.current_velocity[0] = 0.0;
            self.input.current_acceleration[0] = 0.0;
            self.input.target_position[0] = current;
            self.output.new_position[0] = current;
            self.output.new_velocity[0] = 0.0;
            self.output.new_acceleration[0] = 0.0;
            self.publish_state();
            return Ok(());
        }

        let error = target.position - current;

        // Prevent overshoot: never command more velocity than required to land
        // exactly on the current target in one tick.
        let max_for_error = if dt > 0.0 { error.abs() / dt } else { 0.0 };
        let desired_mag = target.velocity.min(max_for_error);
        let desired_velocity = if error > 0.0 {
            desired_mag
        } else if error < 0.0 {
            -desired_mag
        } else {
            0.0
        };

        // First conservative servo tune: 3000 mm/s^2. At 10 ms this changes
        // velocity by at most 30 mm/s per tick. This is independent of Ruckig
        // and remains below the controller's configured acceleration ceiling.
        const SEXCODE_SERVO_ACCEL_MM_S2: f64 = 3000.0;
        let accel_limit = SEXCODE_SERVO_ACCEL_MM_S2.min(self.limits.max_acceleration_mm_s2);
        let max_dv = accel_limit * dt;
        let previous_velocity = self.input.current_velocity[0];
        let dv = desired_velocity - previous_velocity;
        let velocity = if dv > max_dv {
            previous_velocity + max_dv
        } else if dv < -max_dv {
            previous_velocity - max_dv
        } else {
            desired_velocity
        };

        let mut next = current + velocity * dt;
        if (error >= 0.0 && next > target.position) || (error <= 0.0 && next < target.position) {
            next = target.position;
        }
        next = next.clamp(self.limits.min_position_mm, self.limits.max_position_mm);

        if let Err(e) = self.board.set_position(next).await {
            log::error!("Board set_position failed in SexCode servo: {:?}", e);
            self.enter_fault();
            return Err(e);
        }

        let actual_velocity = if dt > 0.0 { (next - current) / dt } else { 0.0 };
        let acceleration = if dt > 0.0 { (actual_velocity - previous_velocity) / dt } else { 0.0 };

        self.input.control_interface = ControlInterface::Position;
        self.input.current_position[0] = next;
        self.input.current_velocity[0] = actual_velocity;
        self.input.current_acceleration[0] = acceleration;
        self.input.target_position[0] = target.position;
        self.input.max_velocity[0] = target.velocity.max(MIN_VELOCITY);

        self.output.new_position[0] = next;
        self.output.new_velocity[0] = actual_velocity;
        self.output.new_acceleration[0] = acceleration;
        self.publish_state();
        Ok(())
    }

    /// Run the homing sequence. Transitions to `Ready` on success, stays
    /// `Disabled` on failure.
    async fn home(&mut self) -> Result<(), B::Error> {
        if let Err(e) = self.board.home().await {
            log::error!("Board home failed: {:?}", e);
            self.transition(MotionState::Disabled);
            return Err(e);
        }

        self.input.control_interface = ControlInterface::Position;
        self.input.current_position[0] = self.limits.min_position_mm;
        self.input.target_position[0] = self.limits.min_position_mm;
        self.input.current_velocity[0] = 0.0;
        self.input.current_acceleration[0] = 0.0;

        if let Err(e) = self.board.set_position(self.limits.min_position_mm).await {
            log::error!("Board set_position after home failed: {:?}", e);
            return Err(e);
        }

        self.target = None;
        self.transition(MotionState::Ready);
        Ok(())
    }

    /// Best-effort disable. Logs errors but always transitions to `Disabled`,
    /// because there is no useful recovery if the motor won't turn off.
    async fn disable(&mut self) {
        if let Err(e) = self.board.disable().await {
            log::error!("Board disable failed: {:?}", e);
        }
        self.input.control_interface = ControlInterface::Position;
        self.target = None;
        self.transition(MotionState::Disabled);
    }

    fn stop(&mut self, reason: StopReason) {
        // Switch to velocity control and target zero velocity. Ruckig handles
        // the jerk-limited deceleration trajectory — no manual math needed.
        self.input.control_interface = ControlInterface::Velocity;
        self.input.target_velocity[0] = 0.0;
        self.output.time = 0.0;
        self.transition(MotionState::Stopping(reason));
    }

    async fn resume(&mut self) {
        // Switch back to position control and restore the instructed target.
        self.input.control_interface = ControlInterface::Position;
        self.sync_ruckig();
        self.apply_torque().await;
        self.transition(MotionState::Moving);
    }

    /// Cancel any in-flight motion and transition to `Disabled`.
    ///
    /// Called when `board.tick()` reports a critical fault. Signals appropriate
    /// responses on the channels so callers aren't left waiting.
    fn enter_fault(&mut self) {
        match self.state {
            MotionState::Moving | MotionState::Paused => {
                self.channels.move_resp.signal(Err(Cancelled));
            }
            MotionState::Stopping(StopReason::Pause) => {
                self.channels.move_resp.signal(Err(Cancelled));
            }
            MotionState::Stopping(StopReason::Disable | StopReason::Home) => {
                self.respond(StateResponse::Fault);
            }
            _ => {}
        }
        self.target = None;
        self.transition(MotionState::Disabled);
    }

    fn respond(&self, resp: StateResponse) {
        self.channels.state_resp.signal(resp);
    }

    fn fraction_to_mm(&self, fraction: f64) -> f64 {
        let mm = self.limits.min_position_mm
            + fraction * (self.limits.max_position_mm - self.limits.min_position_mm);
        mm.clamp(self.limits.min_position_mm, self.limits.max_position_mm)
    }

    fn fraction_to_velocity(&self, fraction: f64) -> f64 {
        let mm_s = fraction * self.limits.max_velocity_mm_s;
        mm_s.clamp(MIN_VELOCITY, self.limits.max_velocity_mm_s)
    }

    fn ramp_by_exponent(&self, value: f64, exponent: f64) -> f64 {
        let mut ramped = 1.0 - value;
        ramped = ramped.powf(exponent);
        ramped = 1.0 - ramped;
        return ramped.powf(1.0/exponent);
    }

    /// Calculates minimum and maximum jerk values
    /// Minimum based on meeting the requested speed at least momentatirly along the full rail.
    /// Maximum value based on 12mm of jerk distance.
    /// Ramped by the squareroot of the input to allow fine granularity of low values.
    /// Only 95% of the maximum value is potentially used. This seems to make ruckig more stable.
    /// When velocity is slowing, previous jerk is used if it is greater then the new jerk to ensure time to slow down.
    fn fraction_to_jerk(&self, fraction: f64, speed: f64) -> f64 {
        let speed_3 = 2.0 * speed.powf(3.0);
        let max_jerk = 0.95 * speed_3 / 12.0.powf(2.0);
        let rail_2 = (self.limits.max_position_mm - self.limits.min_position_mm).powf(2.0);
        let min_jerk= speed_3 / rail_2;
        let mm_s3 = self.ramp_by_exponent(fraction, 0.5) * max_jerk + min_jerk;
        if self.input.current_velocity[0].abs() > speed && self.input.max_jerk[0] > mm_s3{
            return self.input.max_jerk[0];
        }
        mm_s3.clamp(1.0, self.limits.max_jerk_mm_s3)
    }

    fn set_motion_target(&mut self, cmd: MotionCommand) {
        let speed = if cmd.speed <= 0.0 {
            0.0
        } else {
            self.fraction_to_velocity(cmd.speed)
        };
        self.target = Some(MotionTarget {
            position: self.fraction_to_mm(cmd.position),
            velocity: speed,
            jerk: if cmd.direct_stream {
                self.limits.max_jerk_mm_s3
            } else {
                self.fraction_to_jerk(cmd.jerk, speed)
            },
            torque: cmd.torque,
            direct_stream: cmd.direct_stream,
        });

        // The dedicated SexCode servo is sampled directly by tick() every
        // 10 ms, so there is intentionally no Ruckig validation/reset here.
        if !cmd.direct_stream {
            self.sync_ruckig();
        }
    }

    /// Write the instructed target into ruckig's input parameters and reset
    /// the trajectory timer so ruckig replans.
    /// If slowing, set current velocity to maximum so that recalculation doesn't overshoot position
    /// This may cause some jerk, but is acceptable compared to the alternative over greatly overshooting the target.
    fn sync_ruckig(&mut self) {
        if let Some(target) = &self.target {
            self.input.target_position[0] = target.position;
            self.input.max_jerk[0] = target.jerk;
            self.input.max_velocity[0] = target.velocity;
            if self.input.current_velocity[0].abs() > target.velocity {
                self.input.current_velocity[0] = target.velocity * (self.input.current_velocity[0]/self.input.current_velocity[0].abs());
                self.input.current_acceleration[0] = 0.0;
            }
            let _result = match self.ruckig.validate_input(&self.input, true, true){
                Ok(result) => result,
                Err(error) => {
                    log::error!("{:?}", error);
                },
            };
            self.output.time = 0.0;
            self.ruckig.reset();
        }
    }

    async fn apply_torque(&mut self) {
        let fraction = self.target.as_ref().and_then(|t| t.torque).unwrap_or(1.0);
        if let Err(e) = self.board.set_torque(fraction).await {
            log::error!("Board set_torque failed: {:?}", e);
            self.enter_fault();
        }
    }

    fn phase(&self) -> MotionPhase {
        match self.state {
            MotionState::Disabled => MotionPhase::Disabled,
            MotionState::Enabled => MotionPhase::Enabled,
            MotionState::Ready => MotionPhase::Ready,
            MotionState::Moving => MotionPhase::Moving,
            MotionState::Stopping(_) => MotionPhase::Stopping,
            MotionState::Paused => MotionPhase::Paused,
        }
    }

    fn mm_to_fraction(&self, mm: f64) -> f32 {
        let range = self.limits.max_position_mm - self.limits.min_position_mm;
        if range <= 0.0 {
            return 0.0;
        }
        ((mm - self.limits.min_position_mm) / range) as f32
    }

    fn velocity_to_fraction(&self, mm_s: f64) -> f32 {
        if self.limits.max_velocity_mm_s <= 0.0 {
            return 0.0;
        }
        (mm_s / self.limits.max_velocity_mm_s) as f32
    }

    fn acceleration_to_fraction(&self, mm_s2: f64) -> f32 {
        if self.limits.max_acceleration_mm_s2 <= 0.0 {
            return 0.0;
        }
        (mm_s2 / self.limits.max_acceleration_mm_s2) as f32
    }

    fn publish_state(&self) {
        let position_mm = self.output.new_position[0]
            .clamp(self.limits.min_position_mm, self.limits.max_position_mm);
        let velocity_mm_s = self.output.new_velocity[0];
        let acceleration_mm_s2 = self.output.new_acceleration[0];
        let torque = self.target.as_ref().and_then(|t| t.torque).unwrap_or(1.0);

        self.channels.motion_state.update(crate::state::MotionState {
            phase: self.phase(),
            position: self.mm_to_fraction(position_mm),
            velocity: self.velocity_to_fraction(velocity_mm_s.abs()),
            acceleration: self.acceleration_to_fraction(acceleration_mm_s2.abs()),
            torque: torque as f32,
        });
    }

    fn transition(&mut self, new_state: MotionState) {
        self.state = new_state;
        self.publish_state();
        self.channels.motion_state.publish_phase(self.phase());
    }
}
