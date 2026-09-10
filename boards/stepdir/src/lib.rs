#![no_std]

use embedded_hal_async::delay::DelayNs;
use log::info;
use ossm::{Board, CurrentSensor, MechanicalConfig, StepDir};

#[derive(Debug)]
pub enum BoardError<M: core::fmt::Debug, C: core::fmt::Debug> {
    Motor(M),
    CurrentSensor(C),
    HomingTimeout,
}

#[derive(Debug, Clone, Copy)]
#[repr(i32)]
pub enum Direction {
    Forward = -1,
    Backward = 1,
}

pub struct HomingConfig {
    /// How far the motor crawls per polling iteration during homing (mm).
    /// Effective crawl speed = crawl_mm_per_poll * 1000 / poll_delay_ms.
    pub crawl_mm_per_poll: f32,
    /// Current reading (percent above offset) that indicates a stall.
    /// Matches reference firmware
    pub current_threshold: f32,
    /// Number of current sensor samples to average for offset calibration.
    pub calibration_samples: u32,
    /// Milliseconds between current checks during homing.
    pub poll_delay_ms: u32,
    /// Maximum time allowed for homing before timeout (ms).
    pub timeout_ms: u32,
}

impl Default for HomingConfig {
    fn default() -> Self {
        Self {
            crawl_mm_per_poll: 0.25,
            current_threshold: 6.0,
            calibration_samples: 1000,
            poll_delay_ms: 10,
            timeout_ms: 40_000,
        }
    }
}

pub struct StepDirBoard<M: StepDir, C: CurrentSensor, D: DelayNs> {
    motor: M,
    current_sensor: C,
    delay: D,
    mechanical: &'static MechanicalConfig,
    homing: HomingConfig,
}

impl<M: StepDir, C: CurrentSensor, D: DelayNs> StepDirBoard<M, C, D> {
    pub fn new(
        motor: M,
        current_sensor: C,
        delay: D,
        mechanical: &'static MechanicalConfig,
        homing: HomingConfig,
    ) -> Self {
        Self {
            motor,
            current_sensor,
            delay,
            mechanical,
            homing,
        }
    }

    /// Calibrate the current sensor by averaging readings at idle.
    async fn calibrate_current_offset(&mut self) -> Result<f32, C::Error> {
        let mut sum: f32 = 0.0;
        for _ in 0..self.homing.calibration_samples {
            sum += self.current_sensor.read_percent().await?;
        }
        Ok(sum / self.homing.calibration_samples as f32)
    }

    /// Crawl in one [`Direction`] until a current spike is detected, then back off.
    async fn crawl_until_stall(
        &mut self,
        direction: Direction,
        offset: f32,
    ) -> Result<i32, BoardError<M::Error, C::Error>> {
        let crawl_steps = self.mechanical.mm_to_steps(
            self.homing.crawl_mm_per_poll as f64,
            self.motor.steps_per_rev(),
        );
        let max_polls = self.homing.timeout_ms / self.homing.poll_delay_ms;
        let mut position: i32 = 0;

        for _ in 0..max_polls {
            position += direction as i32 * crawl_steps;
            self.motor
                .set_absolute_position(position)
                .await
                .map_err(BoardError::Motor)?;

            self.delay.delay_ms(self.homing.poll_delay_ms).await;

            let reading = self
                .current_sensor
                .read_percent()
                .await
                .map_err(BoardError::CurrentSensor)?;
            let current = (reading - offset).abs();

            if current > self.homing.current_threshold {
                info!(
                    "Stall detected at position {} (current: {:.1}%)",
                    position, current
                );
                return Ok(position);
            }
        }

        Err(BoardError::HomingTimeout)
    }
}

impl<M, C, D> Board for StepDirBoard<M, C, D>
where
    M: StepDir,
    C: CurrentSensor,
    D: DelayNs,
{
    type Error = BoardError<M::Error, C::Error>;

    async fn enable(&mut self) -> Result<(), Self::Error> {
        self.motor.enable().await.map_err(BoardError::Motor)
    }

    async fn disable(&mut self) -> Result<(), Self::Error> {
        self.motor.disable().await.map_err(BoardError::Motor)
    }

    async fn home(&mut self) -> Result<(), Self::Error> {
        info!("Homing started");

        self.motor.enable().await.map_err(BoardError::Motor)?;

        let offset = self
            .calibrate_current_offset()
            .await
            .map_err(BoardError::CurrentSensor)?;
        info!("Current sensor offset: {:.2}%", offset);

        // Crawl backward (toward home) until stall
        self.crawl_until_stall(Direction::Forward, offset).await?;

        // Zero at the hard stop - the controller will move to min_position_mm
        self.motor.reset_position(0);

        info!("Homing complete");
        Ok(())
    }

    async fn set_position(&mut self, position_mm: f64) -> Result<(), Self::Error> {
        let steps = self
            .mechanical
            .mm_to_steps(position_mm, self.motor.steps_per_rev());
        self.motor
            .set_absolute_position(steps)
            .await
            .map_err(BoardError::Motor)
    }

    async fn set_torque(&mut self, _fraction: f64) -> Result<(), Self::Error> {
        // Step/dir drivers handle current limiting in hardware.
        Ok(())
    }

    async fn position_mm(&mut self) -> Result<f64, Self::Error> {
        let steps = self
            .motor
            .read_absolute_position()
            .await
            .map_err(BoardError::Motor)?;
        Ok(self
            .mechanical
            .steps_to_mm(steps, self.motor.steps_per_rev()))
    }

    async fn tick(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}
