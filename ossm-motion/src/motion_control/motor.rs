use core::fmt::Debug;

use crate::motion_control::timer::Duration;

pub trait Motor {
    type MotorError: Debug;

    /// The minimum timing the commands are allowed to be sent to the motor with
    fn min_consecutive_write_delay() -> Duration;

    /// Absolute position in steps
    fn set_absolute_position(&mut self, steps: i32) -> Result<(), Self::MotorError>;

    /// Torque
    fn set_max_allowed_output(&mut self, output: u16) -> Result<(), Self::MotorError>;

    /// Blocking delay function
    /// Provided by the motor to not waste an extra timer just for this
    fn delay(&mut self, duration: Duration);
}
