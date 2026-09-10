#![no_std]
extern crate alloc;

mod board;
mod build_info;
pub use build_info::BuildMeta;
mod command;
mod limits;
pub mod logging;
mod mechanical;
mod motion;
mod motor;
mod observer;
pub mod planner;
mod receiver;
mod sender;
mod state;
pub mod transport;

pub use board::Board;
pub use command::{Cancelled, MotionCommand, StateCommand, StateResponse};
use command::{MoveChannel, MoveResponseSignal, StateChannel, StateResponseSignal};
pub use limits::MotionLimits;
pub use mechanical::MechanicalConfig;
pub use motion::MotionController;
pub use motor::{CurrentSensor, Motor, Rs485Motor, SelfHoming, StepDir};
pub use observer::MotionObserver;
pub use receiver::MotionReceiver;
pub use sender::MotionSender;
use state::MotionStateChannels;
pub use state::{MotionPhase, MotionState};
pub use transport::{
    Modbus, ModbusTransport, ReadNonBlocking, Rs485ModbusTransport, StepDirConfig, StepDirError,
    StepDirMotor, StepOutput, TransportError, UartReconfigure,
};

/// Root container for an OSSM instance.
///
/// Carries the motion-channel state that the three capability handles
/// project from. `Ossm` is instantiated once in static storage and
/// immediately consumed by [`split`](Self::split). After split returns,
/// nothing in the program holds an `&Ossm`: the three named
/// capabilities below cover every role.
///
/// # Motor lifecycle
///
/// The motor is physically powered for as long as the firmware is
/// running. There is no Rust-level mechanism to power it off, and
/// **dropping any of the capabilities returned by `split` does not
/// stop in-flight motion**. The motor pins are sealed into the
/// [`MotionController`] when [`MotionReceiver::into_controller`] is
/// called, and the controller owns them for the rest of the program's
/// lifetime.
///
/// To stop motion, send a state command via [`MotionSender`] (e.g.
/// [`disable`](MotionSender::disable), [`pause`](MotionSender::pause))
/// *before* letting the sender go out of scope. Dropping the sender
/// only prevents future commands; it does not interrupt automatic,
/// self-completing commands like [`home`](MotionSender::home) that the
/// controller is already executing.
pub struct Ossm {
    pub(crate) move_cmd: MoveChannel,
    pub(crate) state_cmd: StateChannel,
    pub(crate) state_resp: StateResponseSignal,
    pub(crate) move_resp: MoveResponseSignal,
    pub(crate) motion_state: MotionStateChannels,
}

impl Ossm {
    pub const fn new() -> Self {
        Self {
            move_cmd: MoveChannel::new(),
            state_cmd: StateChannel::new(),
            state_resp: StateResponseSignal::new(),
            move_resp: MoveResponseSignal::new(),
            motion_state: MotionStateChannels::new(),
        }
    }

    /// Split into the three motion-channel capabilities.
    ///
    /// Consumes a unique `&'static mut Self`, so it can be called at
    /// most once: nothing in the program can produce a second
    /// `&'static mut Ossm`. The expected boot shape is
    /// `StaticCell::init(Ossm::new()).split()`.
    ///
    /// - [`MotionReceiver`] is the receiver-side capability. The
    ///   standard firmware path immediately consumes it via
    ///   [`MotionReceiver::into_controller`] to build the running
    ///   controller. Sim/test builds may hold it long-term.
    /// - [`MotionObserver`] is the read-only observer of motion state
    ///   and phase transitions.
    /// - [`MotionSender`] is the sender-side capability. Move it to
    ///   whichever subsystem is responsible for issuing motion.
    pub fn split(&'static mut self) -> (MotionReceiver, MotionObserver, MotionSender) {
        (
            MotionReceiver::new(self),
            MotionObserver::new(self),
            MotionSender::new(self),
        )
    }
}
