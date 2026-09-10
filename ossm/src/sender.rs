use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::pubsub::{self, Subscriber};

use crate::Ossm;
use crate::command::{Cancelled, MotionCommand, StateCommand, StateResponse};
use crate::state::{MotionPhase, MotionState};

/// Sender half of the motion channels.
///
/// Every method that can move the motor lives here, plus the read
/// methods from [`MotionObserver`](crate::MotionObserver) - motion
/// issuers almost always also need to read state to plan, and reading
/// is harmless. Code that only needs to observe should still take
/// `&MotionObserver` for the tighter contract.
///
/// There is exactly one `MotionSender` per [`Ossm`](crate::Ossm),
/// produced by [`Ossm::split`](crate::Ossm::split). The receiver half
/// is [`MotionReceiver`](crate::MotionReceiver), typically consumed at
/// boot into a [`MotionController`](crate::MotionController).
///
/// Not [`Clone`] and not publicly constructible. The intended pattern
/// is that a single owner holds the sender at boot and lends out
/// `&MotionSender` to whichever subsystem is currently allowed to
/// drive the motor; the borrow expires with that subsystem, so a leaked
/// stash is impossible.
///
/// Code that does not have a value of this type in scope structurally
/// cannot move the motor, no matter what it does with `&Ossm`.
///
/// # Dropping the sender does not stop motion
///
/// The motor is physically powered for as long as the firmware runs.
/// Dropping a `MotionSender` (or letting all `&MotionSender` borrows
/// go out of scope) prevents *future* commands but does **not** stop
/// in-flight motion. If you call [`home`](Self::home) and then drop
/// the sender, the controller continues homing to completion. To stop
/// motion, call [`disable`](Self::disable) or [`pause`](Self::pause)
/// before dropping.
pub struct MotionSender {
    channels: &'static Ossm,
}

impl MotionSender {
    pub(crate) fn new(channels: &'static Ossm) -> Self {
        Self { channels }
    }

    async fn send_state(&self, cmd: StateCommand) -> StateResponse {
        self.channels.state_resp.reset();
        self.channels.state_cmd.send(cmd).await;
        self.channels.state_resp.wait().await
    }

    pub async fn enable(&self) -> StateResponse {
        self.send_state(StateCommand::Enable).await
    }

    pub async fn disable(&self) -> StateResponse {
        self.send_state(StateCommand::Disable).await
    }

    pub async fn home(&self) -> StateResponse {
        self.send_state(StateCommand::Home).await
    }

    pub async fn pause(&self) -> StateResponse {
        self.send_state(StateCommand::Pause).await
    }

    pub async fn resume(&self) -> StateResponse {
        self.send_state(StateCommand::Resume).await
    }

    /// Start a motion without waiting for completion.
    ///
    /// Resets the move response signal, so a subsequent
    /// [`await_motion`](Self::await_motion) waits for this move to finish.
    pub fn begin_motion(&self, cmd: MotionCommand) {
        self.channels.move_resp.reset();
        let _ = self.channels.move_cmd.try_receive();
        let _ = self.channels.move_cmd.try_send(cmd.clamped());
    }

    /// Update the target of an in-flight motion without resetting the
    /// completion signal.
    pub fn update_motion(&self, cmd: MotionCommand) {
        let _ = self.channels.move_cmd.try_receive();
        let _ = self.channels.move_cmd.try_send(cmd.clamped());
    }

    /// Wait for the current in-flight motion to complete.
    pub async fn await_motion(&self) -> Result<(), Cancelled> {
        self.channels.move_resp.wait().await
    }

    /// Read the current motion state (position, velocity, torque, phase).
    ///
    /// `MotionSender` carries [`MotionObserver`](crate::MotionObserver)'s
    /// read capability so motion-issuing code can plan against current
    /// state without also being handed an observer.
    pub fn state(&self) -> MotionState {
        self.channels.motion_state.get()
    }

    /// Create an async subscriber that receives [`MotionPhase`] on every
    /// phase transition.
    ///
    /// Returns `Err` if all subscriber slots are in use.
    pub fn subscribe(
        &self,
    ) -> Result<Subscriber<'static, CriticalSectionRawMutex, MotionPhase, 1, 8, 0>, pubsub::Error>
    {
        self.channels.motion_state.phase_subscriber()
    }
}
