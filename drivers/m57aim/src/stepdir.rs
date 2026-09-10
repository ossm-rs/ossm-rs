use embedded_hal::digital::OutputPin;
use ossm::transport::step_dir::{StepDirError, StepDirMotor, StepOutput};
use ossm::{Motor, StepDir};

use crate::Motor57AIM;

impl<S, Dir, En, D> Motor for Motor57AIM<StepDirMotor<S, Dir, En>, D>
where
    S: StepOutput,
    Dir: OutputPin,
    En: OutputPin<Error = Dir::Error>,
{
    type Error = StepDirError<S::Error, Dir::Error>;

    fn steps_per_rev(&self) -> u32 {
        self.config.steps_per_rev
    }

    fn max_output(&self) -> u16 {
        self.config.max_output
    }

    async fn enable(&mut self) -> Result<(), Self::Error> {
        self.interface.enable().await
    }

    async fn disable(&mut self) -> Result<(), Self::Error> {
        self.interface.disable().await
    }

    async fn set_absolute_position(&mut self, steps: i32) -> Result<(), Self::Error> {
        self.interface.set_absolute_position(steps).await
    }

    async fn read_absolute_position(&mut self) -> Result<i32, Self::Error> {
        self.interface.read_absolute_position().await
    }

    async fn set_max_output(&mut self, output: u16) -> Result<(), Self::Error> {
        self.interface.set_max_output(output).await
    }
}

impl<S, Dir, En, D> StepDir for Motor57AIM<StepDirMotor<S, Dir, En>, D>
where
    S: StepOutput,
    Dir: OutputPin,
    En: OutputPin<Error = Dir::Error>,
{
    fn reset_position(&mut self, position: i32) {
        self.interface.reset_position(position);
    }
}
