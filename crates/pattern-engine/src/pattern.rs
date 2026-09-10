use embassy_futures::select::{self, Either3};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::watch::Receiver;
use embassy_time::{Duration, Ticker};
use embedded_hal_async::delay::DelayNs;
use ossm::{Cancelled, MotionSender, MotionCommand};

use crate::input::{PatternInput, SharedPatternInput};
use crate::util::scale;

/// Cap on how often input-driven motion updates are forwarded to the controller.
/// Each forwarded command costs a Ruckig replan;  replans can take 5-15 ms,
/// so an unthrottled BLE input flood starves the 100 Hz motion loop.
/// 250 ms keeps replans to ~4 Hz, well under the 10 ms tick budget while
/// still feeling responsive to slider movement.
const INPUT_UPDATE_THROTTLE: Duration = Duration::from_millis(250);

pub const MIN_SENSATION: f64 = -1.0;
pub const MAX_SENSATION: f64 = 1.0;

/// An async pattern that drives repetitive motion.
///
/// `run()` loops forever, using `?` on each move to propagate cancellation.
/// When a state command (disable, home) cancels the in-flight move,
/// `send().await?` returns `Err(Cancelled)`, which exits the pattern cleanly.
#[allow(async_fn_in_trait)]
pub trait Pattern {
    const NAME: &'static str;
    const DESCRIPTION: &'static str;

    async fn run(&mut self, ctx: &mut PatternCtx<'_, impl DelayNs>) -> Result<(), Cancelled>;
}

pub struct PatternCtx<'m, D: DelayNs> {
    motion: &'m MotionSender,
    input: &'m SharedPatternInput,
    input_receiver: Receiver<'m, CriticalSectionRawMutex, PatternInput, 1>,
    delay: D,
}

impl<'m, D: DelayNs> PatternCtx<'m, D> {
    pub fn new(
        motion: &'m MotionSender,
        input: &'m SharedPatternInput,
        delay: D,
    ) -> Self {
        let input_receiver = input.receiver().expect("Watch receiver slot already taken");
        Self {
            motion,
            input,
            input_receiver,
            delay,
        }
    }

    /// Read the current sensation value (-1.0 to 1.0).
    ///
    /// Re-read at each `.await` point to pick up live changes from BLE/UI.
    pub fn sensation(&self) -> f64 {
        self.input
            .try_get()
            .unwrap_or(PatternInput::DEFAULT)
            .sensation
    }

    fn input(&self) -> PatternInput {
        self.input.try_get().unwrap_or(PatternInput::DEFAULT)
    }

    /// Start building a motion command.
    ///
    /// Chain `.position()` to set the target, optionally `.speed()` to override
    /// the velocity multiplier, then `.send().await?` to execute.
    ///
    /// ```ignore
    /// ctx.motion().position(1.0).send().await?;
    /// ctx.motion().position(0.5).speed(0.5).send().await?;
    /// ```
    pub fn motion(&mut self) -> MotionBuilder<'_, 'm, D, NoPosition> {
        MotionBuilder {
            ctx: self,
            position: NoPosition,
            speed_factor: 1.0,
            jerk_factor: 0.5,
            torque: None,
        }
    }

    pub async fn delay_ms(&mut self, ms: u64) {
        self.delay.delay_ms(ms as u32).await;
    }

    /// Map the current sensation (-1.0..1.0) to an output range.
    pub fn scale_sensation(&self, out_min: f64, out_max: f64) -> f64 {
        scale(
            self.sensation(),
            MIN_SENSATION,
            MAX_SENSATION,
            out_min,
            out_max,
        )
    }
}

pub struct NoPosition;

pub struct HasPosition(f64);

/// Builder for a single motion command.
///
/// Created via [`PatternCtx::motion()`]. Call `.position()` before `.send()` -
/// the type system enforces this at compile time.
pub struct MotionBuilder<'a, 'm, D: DelayNs, P> {
    ctx: &'a mut PatternCtx<'m, D>,
    position: P,
    speed_factor: f64,
    jerk_factor: f64,
    torque: Option<f64>,
}

impl<'a, 'm, D: DelayNs, P> MotionBuilder<'a, 'm, D, P> {
    /// Set the velocity as a multiplier of the current input velocity.
    ///
    /// Default is 1.0 (full input velocity). 0.5 = half speed, max is 1.0.
    pub fn speed(mut self, factor: f64) -> Self {
        self.speed_factor = factor;
        self
    }

    /// Set the jerk
    /// 0.0 = smooth, 1.0 = choppy 
    pub fn jerk(mut self, factor: f64) -> Self {
        self.jerk_factor = factor;
        self
    }

    /// Set the torque limit as a factor between 0.0 and 1.0.
    ///
    /// `None` (the default) uses the motor's default torque.
    pub fn torque(mut self, factor: f64) -> Self {
        self.torque = Some(factor);
        self
    }
}

impl<'a, 'm, D: DelayNs> MotionBuilder<'a, 'm, D, NoPosition> {
    /// Set the target position as a fraction of the stroke range.
    ///
    /// 0.0 = shallowest (`depth - stroke`), 1.0 = deepest (`depth`).
    pub fn position(self, fraction: f64) -> MotionBuilder<'a, 'm, D, HasPosition> {
        MotionBuilder {
            ctx: self.ctx,
            position: HasPosition(fraction),
            speed_factor: self.speed_factor,
            jerk_factor: self.jerk_factor,
            torque: self.torque,
        }
    }
}

fn compute_command(
    input: &PatternInput,
    fraction: f64,
    speed_factor: f64,
    jerk_factor: f64,
    torque: Option<f64>,
) -> MotionCommand {
    let stroke = input.stroke.clamp(0.0, input.depth);
    let shallow = input.depth - stroke;
    let position = shallow + fraction * stroke;
    let speed = input.velocity * speed_factor.clamp(0.0, 1.0);
    let jerk = jerk_factor.clamp(0.0, 1.0);
    MotionCommand {
        position,
        speed,
        jerk,
        torque,
        direct_stream: false,
    }
}

impl<'a, 'm, D: DelayNs> MotionBuilder<'a, 'm, D, HasPosition> {
    pub async fn send(self) -> Result<(), Cancelled> {
        let fraction = self.position.0.clamp(0.0, 1.0);
        let speed_factor = self.speed_factor;
        let jerk_factor = self.jerk_factor;
        let torque = self.torque;

        let mut input = self.ctx.input();
        let mut cmd = compute_command(&input, fraction, speed_factor, jerk_factor, torque);

        // Some legacy BLE clients issue Play while their speed is still zero,
        // then ramp speed up with later set:speed commands. Do not start a
        // zero-velocity Ruckig trajectory; wait here until the first positive
        // speed arrives. Dropping this future still cancels normally when the
        // runner receives Stop/Pause/another Play command.
        while cmd.speed <= 0.0001 {
            input = self.ctx.input_receiver.changed().await;
            cmd = compute_command(&input, fraction, speed_factor, jerk_factor, torque);
        }

        self.ctx.motion.begin_motion(cmd);

        let mut move_done = core::pin::pin!(self.ctx.motion.await_motion());
        let mut throttle = Ticker::every(INPUT_UPDATE_THROTTLE);
        let mut pending: Option<PatternInput> = None;
        // Legacy BLE clients such as Possum implement pause by ramping
        // set:speed down to exactly zero, then ramping it back up on resume.
        // Never feed velocity=0 into an active point-to-point Ruckig move:
        // that can invalidate/cancel the trajectory and terminate the pattern,
        // leaving later nonzero speed updates with nothing left to resume.
        // Instead pause the low-level motion while keeping this pattern future
        // alive, then resume the same pending stroke when speed becomes > 0.
        let mut zero_speed_paused = false;

        loop {
            match select::select3(
                move_done.as_mut(),
                self.ctx.input_receiver.changed(),
                throttle.next(),
            )
            .await
            {
                Either3::First(result) => return result,
                Either3::Second(new_input) => {
                    pending = Some(new_input);
                }
                Either3::Third(()) => {
                    if let Some(input) = pending.take() {
                        let cmd = compute_command(&input, fraction, speed_factor, jerk_factor, torque);

                        if cmd.speed <= 0.0001 {
                            if !zero_speed_paused {
                                match self.ctx.motion.pause().await {
                                    ossm::StateResponse::Completed => {
                                        zero_speed_paused = true;
                                        log::info!("Pattern input speed reached zero; motion soft-paused");
                                    }
                                    response => {
                                        log::warn!("Pattern zero-speed pause failed: {:?}", response);
                                    }
                                }
                            }
                            continue;
                        }

                        if zero_speed_paused {
                            match self.ctx.motion.resume().await {
                                ossm::StateResponse::Completed => {
                                    zero_speed_paused = false;
                                    log::info!("Pattern input speed positive; motion resumed");
                                }
                                response => {
                                    log::warn!("Pattern zero-speed resume failed: {:?}", response);
                                    continue;
                                }
                            }
                        }

                        self.ctx.motion.update_motion(cmd);
                    }
                }
            }
        }
    }
}
