// `Config` always matches the real motor's shape (rs485 for this crate today).
// Sim mode uses the same fields and drops them inside `build`.
#[cfg(feature = "motor-rs485")]
pub use ossm_esp::motor::rs485::Config;

#[cfg(feature = "motor-sim")]
pub use ossm_esp::motor::sim::{Motor, build};

#[cfg(all(feature = "motor-rs485", not(feature = "motor-sim")))]
pub use ossm_esp::motor::rs485::build;
