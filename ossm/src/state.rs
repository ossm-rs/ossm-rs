use core::cell::Cell;

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::blocking_mutex::Mutex;
use embassy_sync::pubsub::{self, PubSubChannel};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MotionPhase {
    Disabled,
    Enabled,
    Ready,
    Moving,
    Stopping,
    Paused,
}

#[derive(Debug, Clone, Copy)]
pub struct MotionState {
    pub phase: MotionPhase,
    /// Current position as a fraction of the machine range (0.0–1.0).
    pub position: f32,
    /// Current velocity as a fraction of max velocity (0.0–1.0).
    pub velocity: f32,
    /// Current acceleration as a fraction of max acceleration (0.0–1.0).
    pub acceleration: f32,
    /// Current torque limit as a fraction (0.0–1.0).
    pub torque: f32,
}

impl MotionState {
    pub(crate) const fn new() -> Self {
        Self {
            phase: MotionPhase::Disabled,
            position: 0.0,
            velocity: 0.0,
            acceleration: 0.0,
            torque: 0.0,
        }
    }
}

/// Broadcast channel for [`MotionPhase`] transitions.
///
/// Uses `PubSubChannel` so subscribers can be created and dropped
/// dynamically as services start and stop.
///
/// - `CAP = 1`: only the latest transition matters; older messages are dropped.
/// - `SUBS = 8`: up to 8 concurrent async subscribers.
/// - `PUBS = 0`: publishing uses [`PubSubChannel::immediate_publisher()`]
///   which does not consume a publisher slot.
type PhaseChannel = PubSubChannel<CriticalSectionRawMutex, MotionPhase, 1, 8, 0>;

pub(crate) struct MotionStateChannels {
    state: Mutex<CriticalSectionRawMutex, Cell<MotionState>>,
    phase: PhaseChannel,
}

impl MotionStateChannels {
    pub(crate) const fn new() -> Self {
        Self {
            state: Mutex::new(Cell::new(MotionState::new())),
            phase: PhaseChannel::new(),
        }
    }

    pub(crate) fn update(&self, new_state: MotionState) {
        self.state.lock(|cell| cell.set(new_state));
    }

    pub(crate) fn publish_phase(&self, phase: MotionPhase) {
        self.phase.immediate_publisher().publish_immediate(phase);
    }

    pub fn get(&self) -> MotionState {
        self.state.lock(|cell| cell.get())
    }

    pub fn phase_subscriber(
        &self,
    ) -> Result<pubsub::Subscriber<'_, CriticalSectionRawMutex, MotionPhase, 1, 8, 0>, pubsub::Error>
    {
        self.phase.subscriber()
    }
}
