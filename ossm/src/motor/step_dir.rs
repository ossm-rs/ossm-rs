use super::Motor;

/// A motor driven via step/direction GPIO pulses.
///
/// Position is tracked in software, so the board must call
/// [`reset_position`](StepDir::reset_position) after homing to
/// synchronise the counter.
#[allow(async_fn_in_trait)]
pub trait StepDir: Motor {
    /// Reset the internal position counter (e.g. after board-managed homing).
    fn reset_position(&mut self, position: i32);
}
