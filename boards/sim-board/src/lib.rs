#![no_std]

use core::convert::Infallible;

use ossm::{Board, MechanicalConfig};
use sim_motor::SimMotor;

/// Thin wrapper implementing `Board` for `SimMotor`.
///
/// Converts mm positions from the motion controller into steps
/// for the simulated motor, using the mechanical config for the
/// conversion factor.
pub struct SimBoard {
    motor: SimMotor,
    mechanical: &'static MechanicalConfig,
}

impl SimBoard {
    pub fn new(motor: SimMotor, mechanical: &'static MechanicalConfig) -> Self {
        Self { motor, mechanical }
    }
}

impl Board for SimBoard {
    type Error = Infallible;

    async fn enable(&mut self) -> Result<(), Self::Error> {
        self.motor.enable().await
    }

    async fn disable(&mut self) -> Result<(), Self::Error> {
        self.motor.disable().await
    }

    async fn home(&mut self) -> Result<(), Self::Error> {
        self.motor.home().await
    }

    async fn set_position(&mut self, position_mm: f64) -> Result<(), Self::Error> {
        let steps = self
            .mechanical
            .mm_to_steps(position_mm, SimMotor::STEPS_PER_REV);
        self.motor.set_absolute_position(steps).await
    }

    async fn set_torque(&mut self, _fraction: f64) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn position_mm(&mut self) -> Result<f64, Self::Error> {
        Ok(self
            .mechanical
            .steps_to_mm(self.motor.position(), SimMotor::STEPS_PER_REV))
    }

    async fn tick(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}
