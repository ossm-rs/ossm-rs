use embassy_time::Delay;
use esp_hal::{
    Blocking,
    analog::adc::{Adc, AdcChannel, AdcConfig, AdcPin, Attenuation},
    gpio::AnalogPin,
    peripherals::ADC1,
};
use ossm::{CurrentSensor, MechanicalConfig};
use stepdir_board::{HomingConfig, StepDirBoard};

use crate::motor::stepdir::Motor;

pub struct Config<P> {
    pub adc1: ADC1<'static>,
    pub current_pin: P,
}

pub type Board<P> = StepDirBoard<Motor, AdcCurrentSensor<P>, Delay>;

pub fn build<P>(
    motor: Motor,
    config: Config<P>,
    mechanical: &'static MechanicalConfig,
) -> Board<P>
where
    P: AdcChannel + AnalogPin,
{
    let mut adc1_config = AdcConfig::new();
    let adc_pin = adc1_config.enable_pin(config.current_pin, Attenuation::_11dB);
    let adc1 = Adc::new(config.adc1, adc1_config);
    let current_sensor = AdcCurrentSensor::new(adc1, adc_pin);

    StepDirBoard::new(motor, current_sensor, Delay, mechanical, HomingConfig::default())
}

/// Reads motor current draw from an analog pin on ADC1.
///
/// ADC1 is used instead of ADC2 because ADC2 conflicts with the Wi-Fi/BLE
/// radio on ESP32.
pub struct AdcCurrentSensor<P> {
    adc: Adc<'static, ADC1<'static>, Blocking>,
    pin: AdcPin<P, ADC1<'static>>,
}

impl<P> AdcCurrentSensor<P> {
    pub fn new(adc: Adc<'static, ADC1<'static>, Blocking>, pin: AdcPin<P, ADC1<'static>>) -> Self {
        Self { adc, pin }
    }
}

impl<P> CurrentSensor for AdcCurrentSensor<P>
where
    P: AdcChannel,
{
    type Error = AdcError;

    async fn read_percent(&mut self) -> Result<f32, Self::Error> {
        let raw = nb::block!(self.adc.read_oneshot(&mut self.pin)).map_err(|_| AdcError)?;
        // 12-bit ADC: 0–4095 → 0.0–100.0%
        Ok(raw as f32 / 4095.0 * 100.0)
    }
}

#[derive(Debug)]
pub struct AdcError;
