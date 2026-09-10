use super::Motor;

/// Trait for motors wired over RS-485.
#[allow(async_fn_in_trait)]
pub trait Rs485Motor: Motor {
    /// Set the motor's direction polarity in its running register.
    ///
    /// Does not persist to EEPROM - the board rewrites this on every homing
    /// cycle from its own config, matching the reference OSSM firmware's
    /// pattern. When `reverse == true`, the motor's built-in homing seeks the
    /// opposite direction and positive step commands produce opposite physical
    /// motion. When coupled with a corresponding sign flip in
    /// `MechanicalConfig::mm_to_steps`, the net physical motion for a given
    /// mm command is consistent.
    ///
    /// Default no-op for motors without a configurable polarity register.
    async fn set_dir_polarity(&mut self, _reverse: bool) -> Result<(), Self::Error> {
        Ok(())
    }
}
