use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::signal::Signal;

pub(crate) type MoveChannel = Channel<CriticalSectionRawMutex, MotionCommand, 1>;
pub(crate) type StateChannel = Channel<CriticalSectionRawMutex, StateCommand, 1>;
pub(crate) type StateResponseSignal = Signal<CriticalSectionRawMutex, StateResponse>;
pub(crate) type MoveResponseSignal = Signal<CriticalSectionRawMutex, Result<(), Cancelled>>;

#[derive(Debug, Clone, Copy)]
pub struct MotionCommand {
    /// Target position as a fraction of the machine range (0.0–1.0).
    pub position: f64,
    /// Velocity as a fraction of max velocity (0.0–1.0).
    pub speed: f64,
    /// Jerk modifies motion profile between smooth and choppy (0.0-1.0)
    pub jerk: f64,
    /// Torque limit as a fraction (0.0–1.0). `None` uses the motor default.
    pub torque: Option<f64>,
    /// Real-time servo-style stream. Uses the controller's full acceleration/jerk
    /// envelope while `speed` remains the requested velocity ceiling.
    pub direct_stream: bool,
}

impl MotionCommand {
    pub fn clamped(self) -> Self {
        Self {
            position: self.position.clamp(0.0, 1.0),
            speed: self.speed.clamp(0.0, 1.0),
            jerk: self.jerk.clamp(0.0, 1.0),
            torque: self.torque.map(|t| t.clamp(0.0, 1.0)),
            direct_stream: self.direct_stream,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum StateCommand {
    Enable,
    Disable,
    Home,
    Pause,
    Resume,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StateResponse {
    Completed,
    InvalidTransition,
    /// A board-level fault occurred while processing the command.
    Fault,
}

/// Returned when an in-flight move is cancelled by a state command (e.g. disable, home).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cancelled;
