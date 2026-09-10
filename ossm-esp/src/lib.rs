#![no_std]

pub mod board;
pub mod motor;
pub mod uart;

#[cfg(feature = "motor-rs485")]
pub mod rs485;
