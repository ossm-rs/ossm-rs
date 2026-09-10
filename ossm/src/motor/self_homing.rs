use super::Motor;

/// A motor that can home itself without board-level sensors.
///
/// Motors with built-in homing firmware (e.g. the 57AIM's Modbus homing
/// command) implement this. The board delegates homing entirely to the motor.
#[allow(async_fn_in_trait)]
pub trait SelfHoming: Motor {
    /// Run the full homing sequence. Returns when the motor is homed
    /// and ready for position following.
    async fn home(&mut self) -> Result<(), Self::Error>;
}
