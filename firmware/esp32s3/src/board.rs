#[cfg(feature = "motor-sim")]
pub use ossm_esp::board::sim::Board;

// The rs485 board carries no extra Config (unlike stepdir), so we adapt the
// sim builder's generic pass-through slot with `()` to match rs485's 2-arg
// shape. Keeps lib.rs's `board::build(motor, &MECHANICAL)` call uniform.
#[cfg(feature = "motor-sim")]
pub fn build(
    motor: crate::motor::Motor,
    mechanical: &'static ossm::MechanicalConfig,
) -> Board {
    ossm_esp::board::sim::build(motor, (), mechanical)
}

#[cfg(all(feature = "motor-rs485", not(feature = "motor-sim")))]
pub use ossm_esp::board::rs485::{Board, build};
