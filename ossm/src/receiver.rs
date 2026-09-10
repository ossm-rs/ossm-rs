use crate::{Board, MotionController, MotionLimits, Ossm};

/// Receiver half of the motion channels.
///
/// Counterpart to [`MotionSender`](crate::MotionSender). Produced by
/// [`Ossm::split`](crate::Ossm::split), and in the standard firmware
/// path immediately consumed by [`into_controller`](Self::into_controller)
/// to build the running [`MotionController`].
///
/// Sim / test builds can instead hold a `MotionReceiver` long-term and
/// drive the receiver-side dance manually (see the methods gated under
/// `cfg(feature = "sim")`).
///
/// Not [`Clone`] and not publicly constructible.
pub struct MotionReceiver {
    channels: &'static Ossm,
}

impl MotionReceiver {
    pub(crate) fn new(channels: &'static Ossm) -> Self {
        Self { channels }
    }

    /// Build the [`MotionController`] that owns this receiver end.
    ///
    /// Consumes `self`, so no second controller can be constructed for
    /// the same channels. The board is a position follower - it doesn't
    /// need to know about mechanical config or motion limits; those are
    /// the controller's concern.
    ///
    /// The returned controller takes ownership of the board, and through
    /// it the motor pins. Once spawned into a task the controller runs
    /// for the rest of the program's lifetime; the motor remains
    /// physically powered and under the controller's command until the
    /// firmware loses power. There is no `Drop` path that disables the
    /// motor.
    pub fn into_controller<B: Board>(
        self,
        board: B,
        limits: MotionLimits,
        update_interval_secs: f64,
    ) -> MotionController<'static, B> {
        MotionController::new(board, limits, update_interval_secs, self.channels)
    }
}

#[cfg(feature = "sim")]
mod sim {
    use super::MotionReceiver;
    use crate::command::{StateCommand, StateResponse};
    use crate::MotionCommand;

    impl MotionReceiver {
        /// Try to read a pending motion command from the channel.
        pub fn try_recv_motion(&self) -> Option<MotionCommand> {
            self.channels.move_cmd.try_receive().ok()
        }

        /// Signal that the current motion has completed.
        pub fn signal_motion_complete(&self) {
            self.channels.move_resp.signal(Ok(()));
        }

        /// Signal that the current state command has completed.
        pub fn respond_state(&self, resp: StateResponse) {
            self.channels.state_resp.signal(resp);
        }

        /// Try to read a pending state command from the channel.
        pub fn try_recv_state(&self) -> Option<StateCommand> {
            self.channels.state_cmd.try_receive().ok()
        }
    }
}
