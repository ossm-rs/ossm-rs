pub use ossm_esp::motor::stepdir::Config;

#[cfg(feature = "motor-sim")]
pub use ossm_esp::motor::sim::build;

#[cfg(not(feature = "motor-sim"))]
pub use ossm_esp::motor::stepdir::build;
