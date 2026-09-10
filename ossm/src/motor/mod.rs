mod current_sensor;
mod rs485;
mod self_homing;
mod step_dir;

pub use current_sensor::CurrentSensor;
pub use rs485::Rs485Motor;
pub use self_homing::SelfHoming;
pub use step_dir::StepDir;

/// A motor that a board can drive as a position follower.
///
/// Covers the basics: position control, enable/disable, torque.
/// Communication interface traits ([`StepDir`], [`Rs485Motor`]) extend this
/// with interface-specific capabilities.
#[allow(async_fn_in_trait)]
pub trait Motor {
    type Error: core::fmt::Debug;

    /// Steps per revolution, used by the board to convert mm to steps.
    fn steps_per_rev(&self) -> u32;

    /// Maximum output value for torque scaling.
    fn max_output(&self) -> u16;

    /// Enable the motor and prepare it for position following.
    async fn enable(&mut self) -> Result<(), Self::Error>;

    /// Disable the motor.
    async fn disable(&mut self) -> Result<(), Self::Error>;

    /// Command an absolute position in steps.
    async fn set_absolute_position(&mut self, steps: i32) -> Result<(), Self::Error>;

    /// Read the current absolute position in steps.
    async fn read_absolute_position(&mut self) -> Result<i32, Self::Error>;

    /// Set the maximum torque output (raw motor units).
    async fn set_max_output(&mut self, output: u16) -> Result<(), Self::Error>;
}
