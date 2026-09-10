use core::cell::Cell;
use core::iter;
use core::pin::{Pin, pin};
use core::task::{Context, Poll, Waker};

use alloc::vec::Vec;

use ossm::planner::Planner;
use ossm::{MotionCommand, MotionReceiver, MotionSender, StateCommand, StateResponse};
use pattern_engine::{Pattern, PatternCtx, PatternInput, SharedPatternInput};

/// Shared state between `RecordingDelay` and the recorder loop.
/// The delay writes the requested duration; the loop reads and clears it.
struct DelayState {
    pending_ns: Cell<u64>,
}

impl DelayState {
    const fn new() -> Self {
        Self {
            pending_ns: Cell::new(0),
        }
    }

    fn accumulate(&self, ns: u64) {
        self.pending_ns.set(self.pending_ns.get() + ns);
    }

    fn take_pending_ms(&self, timestep_ms: f64) -> usize {
        let ns = self.pending_ns.replace(0);
        let ms = ns as f64 / 1_000_000.0;
        (ms / timestep_ms).round() as usize
    }
}

/// A delay that yields once (returning `Pending`) so the recorder
/// loop can emit idle samples, then resolves on the next poll.
struct RecordingDelay<'a> {
    state: &'a DelayState,
}

impl embedded_hal_async::delay::DelayNs for RecordingDelay<'_> {
    async fn delay_ns(&mut self, ns: u32) {
        self.state.accumulate(ns as u64);
        // Yield once so the recorder loop sees the pending delay.
        YieldOnce::new().await;
    }
}

/// Future that returns `Pending` once, then `Ready`.
struct YieldOnce(bool);

impl YieldOnce {
    fn new() -> Self {
        Self(false)
    }
}

impl Future for YieldOnce {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<()> {
        if self.0 {
            Poll::Ready(())
        } else {
            self.0 = true;
            Poll::Pending
        }
    }
}

/// Recorded sample from a single planner tick.
#[derive(Debug, Clone, Copy)]
pub struct Sample {
    pub position: f64,
    pub velocity: f64,
    pub acceleration: f64,
}

/// Synchronously records a pattern's trajectory by manually polling
/// the pattern's async future and feeding motion commands to a planner.
pub struct PatternRecorder<'r, 'm> {
    receiver: &'r MotionReceiver,
    input: &'static SharedPatternInput,
    motion: &'m MotionSender,
}

impl<'r, 'm> PatternRecorder<'r, 'm> {
    pub fn new(
        receiver: &'r MotionReceiver,
        input: &'static SharedPatternInput,
        motion: &'m MotionSender,
    ) -> Self {
        Self {
            receiver,
            input,
            motion,
        }
    }

    /// Record `max_samples` of trajectory data from `pattern` using the
    /// given `planner` and `pattern_input`.
    ///
    /// The pattern is polled synchronously. Each time it yields a motion
    /// command, the planner ticks to completion, recording every sample.
    /// Delays emit idle samples at the current position.
    /// The loop ends when `max_samples` is reached.
    pub fn record<P: Planner>(
        &self,
        pattern: &mut impl Pattern,
        planner: &mut P,
        pattern_input: PatternInput,
        start_position: f64,
        timestep_ms: f64,
        max_samples: usize,
    ) -> Vec<Sample> {
        // Publish the input so PatternCtx can read it.
        self.input.sender().send(pattern_input);

        // Set the planner to the start position.
        planner.set_position(start_position);

        // Clear any stale commands.
        let _ = self.receiver.try_recv_state();
        let _ = self.receiver.try_recv_motion();

        let delay_state = DelayState::new();
        let delay = RecordingDelay {
            state: &delay_state,
        };
        let mut ctx = PatternCtx::new(self.motion, self.input, delay);
        let future = pattern.run(&mut ctx);
        let mut future = pin!(future);

        let waker = Waker::noop();
        let mut cx = Context::from_waker(&waker);

        let mut samples = Vec::with_capacity(max_samples);

        loop {
            let poll = future.as_mut().poll(&mut cx);

            if poll.is_ready() {
                break;
            }

            // Check for state commands (enable, home) and respond.
            if let Some(cmd) = self.receiver.try_recv_state() {
                match cmd {
                    StateCommand::Enable | StateCommand::Home => {
                        self.receiver.respond_state(StateResponse::Completed);
                    }
                    _ => {}
                }
                continue;
            }

            // Check for a motion command.
            if let Some(cmd) = self.receiver.try_recv_motion() {
                self.drive_planner(planner, &cmd, &mut samples, max_samples);

                if samples.len() >= max_samples {
                    break;
                }

                self.receiver.signal_motion_complete();
                continue;
            }

            // No command - check if a delay was requested.
            let delay_ticks = delay_state.take_pending_ms(timestep_ms);
            if delay_ticks > 0 {
                let idle = Sample {
                    position: planner.position(),
                    velocity: 0.0,
                    acceleration: 0.0,
                };
                let count = delay_ticks.min(max_samples - samples.len());
                samples.extend(iter::repeat(idle).take(count));

                if samples.len() >= max_samples {
                    break;
                }
                continue;
            }
        }

        samples
    }

    /// Tick the planner to completion for a given command, recording samples.
    fn drive_planner<P: Planner>(
        &self,
        planner: &mut P,
        cmd: &MotionCommand,
        samples: &mut Vec<Sample>,
        max_samples: usize,
    ) {
        planner.set_target(cmd.position, cmd.speed, cmd.jerk);

        loop {
            let out = planner.tick();
            samples.push(Sample {
                position: out.position,
                velocity: out.velocity,
                acceleration: out.acceleration,
            });

            if out.finished || samples.len() >= max_samples {
                break;
            }
        }
    }
}
