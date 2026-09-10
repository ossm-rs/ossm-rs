use esp_hal::peripherals::GPIO36;

/// ossm-reference has the motor current sense on GPIO36 (ADC1 channel 0).
/// Fixing the pin type here keeps `Board` non-generic so `motion_task`
/// doesn't need a type parameter.
pub type CurrentPin = GPIO36<'static>;

pub type Config = ossm_esp::board::stepdir::Config<CurrentPin>;

#[cfg(feature = "motor-sim")]
pub use ossm_esp::board::sim::{Board, build};

#[cfg(not(feature = "motor-sim"))]
pub use ossm_esp::board::stepdir::build;
#[cfg(not(feature = "motor-sim"))]
pub type Board = ossm_esp::board::stepdir::Board<CurrentPin>;
