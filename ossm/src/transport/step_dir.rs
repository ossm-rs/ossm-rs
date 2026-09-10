use crate::motor::{Motor, StepDir as StepDirTrait};
use core::fmt::Debug;
use embedded_hal::digital::OutputPin;

/// Abstracts step-pulse generation.
///
/// The firmware layer provides a concrete implementation (e.g. MCPWM on ESP32).
/// The driver only cares that `count` pulses are produced - timing, duty cycle,
/// and hardware details live behind this trait.
#[allow(async_fn_in_trait)]
pub trait StepOutput {
    type Error: Debug;

    /// Generate `count` step pulses. Returns when all pulses have been emitted.
    async fn step(&mut self, count: u32) -> Result<(), Self::Error>;
}

pub struct StepDirConfig {
    pub steps_per_rev: u32,
    /// Maximum output value for the Motor trait. Step/dir drivers handle
    /// current limiting in hardware, so this is largely informational.
    pub max_output: u16,
}

impl Default for StepDirConfig {
    fn default() -> Self {
        Self {
            steps_per_rev: 800,
            max_output: 1000,
        }
    }
}

#[derive(Debug)]
pub enum StepDirError<S: Debug, P: Debug> {
    Step(S),
    Pin(P),
}

pub struct StepDirMotor<S: StepOutput, D: OutputPin, E: OutputPin> {
    step: S,
    dir: D,
    enable: E,
    position: i32,
    config: StepDirConfig,
}

impl<S: StepOutput, D: OutputPin, E: OutputPin> StepDirMotor<S, D, E> {
    pub fn new(step: S, dir: D, enable: E, config: StepDirConfig) -> Self {
        Self {
            step,
            dir,
            enable,
            position: 0,
            config,
        }
    }
}

impl<S, D, E> Motor for StepDirMotor<S, D, E>
where
    S: StepOutput,
    D: OutputPin,
    E: OutputPin<Error = D::Error>,
{
    type Error = StepDirError<S::Error, D::Error>;

    fn steps_per_rev(&self) -> u32 {
        self.config.steps_per_rev
    }

    fn max_output(&self) -> u16 {
        self.config.max_output
    }

    async fn enable(&mut self) -> Result<(), Self::Error> {
        // ENA is active-low on stock OSSM hardware
        self.enable.set_low().map_err(StepDirError::Pin)?;
        Ok(())
    }

    async fn disable(&mut self) -> Result<(), Self::Error> {
        self.enable.set_high().map_err(StepDirError::Pin)?;
        Ok(())
    }

    async fn set_absolute_position(&mut self, steps: i32) -> Result<(), Self::Error> {
        let delta = steps - self.position;
        if delta == 0 {
            return Ok(());
        }

        if delta > 0 {
            self.dir.set_high().map_err(StepDirError::Pin)?;
        } else {
            self.dir.set_low().map_err(StepDirError::Pin)?;
        }

        self.step
            .step(delta.unsigned_abs())
            .await
            .map_err(StepDirError::Step)?;

        self.position = steps;
        Ok(())
    }

    async fn read_absolute_position(&mut self) -> Result<i32, Self::Error> {
        Ok(self.position)
    }

    async fn set_max_output(&mut self, _output: u16) -> Result<(), Self::Error> {
        // Step/dir drivers handle current limiting in hardware.
        Ok(())
    }
}

impl<S, D, E> StepDirTrait for StepDirMotor<S, D, E>
where
    S: StepOutput,
    D: OutputPin,
    E: OutputPin<Error = D::Error>,
{
    fn reset_position(&mut self, position: i32) {
        self.position = position;
    }
}
