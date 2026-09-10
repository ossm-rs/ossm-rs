use core::fmt::{self, Debug};

use embassy_time::Delay;
use esp_hal::{
    Blocking,
    gpio::{AnyPin, Level, Output, OutputConfig},
    peripherals::RMT,
    rmt::{Channel, PulseCode, Rmt, Tx, TxChannelConfig, TxChannelCreator},
    time::Rate,
};
use m57aim_motor::{Motor57AIM, Motor57AIMConfig};
use ossm::{StepDirConfig, StepDirMotor, StepOutput};

/// RMT clock divider. With a statically set 80 MHz base clock,
/// divider=4 gives 20 MHz (50 ns per tick).
const RMT_CLK_DIVIDER: u8 = 4;

/// Step pulse width in RMT ticks. At 50 ns/tick, 20 ticks = 1 µs per half.
/// Full step period = 2 µs → max 500 kHz step rate.
const STEP_PULSE_TICKS: u16 = 20;

/// Maximum number of step pulses per RMT transmission batch.
/// The ESP32 has 64 entries per channel. We use 63 pulses + 1 end marker.
const STEP_BATCH_SIZE: usize = 63;

pub struct Config {
    pub rmt: RMT<'static>,
    pub step: AnyPin<'static>,
    pub dir: AnyPin<'static>,
    pub enable: AnyPin<'static>,
}

pub type Motor =
    Motor57AIM<StepDirMotor<RmtStepOutput, Output<'static>, Output<'static>>, Delay>;

pub fn build(config: Config) -> Motor {
    let rmt = Rmt::new(config.rmt, Rate::from_mhz(80)).expect("Failed to initialize RMT");
    let tx_config = TxChannelConfig::default().with_clk_divider(RMT_CLK_DIVIDER);
    let rmt_channel = rmt
        .channel0
        .configure_tx(config.step, tx_config)
        .expect("Failed to configure RMT TX channel");

    let step_output = RmtStepOutput::new(rmt_channel);
    let dir_pin = Output::new(config.dir, Level::Low, OutputConfig::default());
    let enable_pin = Output::new(config.enable, Level::High, OutputConfig::default());

    let step_dir_motor =
        StepDirMotor::new(step_output, dir_pin, enable_pin, StepDirConfig::default());

    Motor57AIM::new(step_dir_motor, Motor57AIMConfig::default(), Delay)
}

/// Generates step pulses via the ESP RMT peripheral. Drives the GPIO
/// autonomously in batches; no CPU involvement while pulses are in flight.
pub struct RmtStepOutput {
    channel: Option<Channel<'static, Blocking, Tx>>,
    pulse: PulseCode,
}

impl RmtStepOutput {
    fn new(channel: Channel<'static, Blocking, Tx>) -> Self {
        let pulse = PulseCode::new(Level::High, STEP_PULSE_TICKS, Level::Low, STEP_PULSE_TICKS);
        Self {
            channel: Some(channel),
            pulse,
        }
    }
}

pub enum RmtError {
    Rmt(esp_hal::rmt::Error),
    ChannelInUse,
}

impl Debug for RmtError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rmt(e) => write!(f, "Rmt({:?})", e),
            Self::ChannelInUse => write!(f, "ChannelInUse"),
        }
    }
}

impl StepOutput for RmtStepOutput {
    type Error = RmtError;

    async fn step(&mut self, count: u32) -> Result<(), Self::Error> {
        let mut remaining = count as usize;
        let mut ch = self.channel.take().ok_or(RmtError::ChannelInUse)?;

        while remaining > 0 {
            let batch = remaining.min(STEP_BATCH_SIZE);
            remaining -= batch;

            let mut buf = [PulseCode::end_marker(); STEP_BATCH_SIZE + 1];
            for entry in buf.iter_mut().take(batch) {
                *entry = self.pulse;
            }
            buf[batch] = PulseCode::end_marker();

            let tx = ch.transmit(&buf[..batch + 1]).map_err(RmtError::Rmt)?;
            ch = match tx.wait() {
                Ok(c) => c,
                Err((e, c)) => {
                    self.channel = Some(c);
                    return Err(RmtError::Rmt(e));
                }
            };
        }

        self.channel = Some(ch);
        Ok(())
    }
}
