#[cfg(feature = "motor-rs485")]
pub mod rs485;

#[cfg(feature = "motor-stepdir")]
pub mod stepdir;

#[cfg(feature = "motor-sim")]
pub mod sim;
