use crate::Ossm;
use crate::state::{MotionPhase, MotionState};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::pubsub::{self, Subscriber};

/// Read-only observer of motion state.
///
/// Produced by [`Ossm::split`](crate::Ossm::split) alongside
/// [`MotionSender`](crate::MotionSender) and
/// [`MotionReceiver`](crate::MotionReceiver). Anyone with a borrow of
/// the observer can read the current motion state and subscribe to
/// phase transitions, but cannot move the motor or service motion
/// commands.
///
/// Not [`Clone`] and not publicly constructible.
pub struct MotionObserver {
    channels: &'static Ossm,
}

impl MotionObserver {
    pub(crate) fn new(channels: &'static Ossm) -> Self {
        Self { channels }
    }

    /// Read the current motion state (position, velocity, torque, phase).
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
