use core::fmt::Debug;

/// A sensor that can read motor current draw.
///
/// Used by the stepdir board for sensorless homing.
#[allow(async_fn_in_trait)]
pub trait CurrentSensor {
    type Error: Debug;

    /// Read the current draw as a percentage of the sensor's full range.
    async fn read_percent(&mut self) -> Result<f32, Self::Error>;
}
