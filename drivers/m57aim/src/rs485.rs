use embedded_hal_async::delay::DelayNs;
use ossm::{Motor, Rs485Motor, SelfHoming, UartReconfigure};

use crate::{
    Modbus, ModbusTransport, Motor57AIM, Motor57AIMConfig, MotorBaudRate, RoRegister, RwRegister,
    STOCK_BAUD_RATE, TARGET_BAUD_RATE,
};

/// Time to wait after UART comes up before the first probe. Gives the
/// motor a chance to finish its own boot and start listening.
const MOTOR_BOOT_DELAY_MS: u32 = 500;

/// Settle window after switching the host UART to the stock baud rate
/// before issuing the baud-provisioning sequence to the motor. Lets the
/// motor drop any garbage it saw at the wrong baud.
const POST_DOWNSHIFT_SETTLE_MS: u32 = 100;

const SET_ABSOLUTE_POSITION_FUNC: u8 = 0x7B;
const HOME_SPEED_RPM: u16 = 80;
const HOME_MAX_OUTPUT: u16 = 89;
const HOME_STEP_THRESHOLD: i32 = 15;
const HOME_POLL_INTERVAL_MS: u32 = 50;
const POST_HOME_SETTLE_MS: u32 = 20;
const POST_ENABLE_SETTLE_MS: u32 = 800;
const OPERATING_SPEED_RPM: u16 = 3000;
const OPERATING_ACCELERATION: u16 = 50000;
const OPERATING_MAX_OUTPUT: u16 = 600;

impl<T: ModbusTransport, D> Motor57AIM<Modbus<T>, D> {
    async fn write_register(&mut self, reg: RwRegister, value: u16) -> Result<(), T::Error> {
        self.interface
            .transport
            .write_holding(self.interface.device_addr, reg as u16, value)
            .await
    }

    async fn read_register(&mut self, reg: u16) -> Result<u16, T::Error> {
        let regs = self
            .interface
            .transport
            .read_holding(self.interface.device_addr, reg, 1)
            .await?;
        Ok(regs[0])
    }

    async fn read_register_pair(&mut self, reg_low: u16) -> Result<i32, T::Error> {
        let regs = self
            .interface
            .transport
            .read_holding(self.interface.device_addr, reg_low, 2)
            .await?;
        let raw = ((regs[1] as u32) << 16) | regs[0] as u32;
        Ok(i32::from_ne_bytes(raw.to_ne_bytes()))
    }

    /// Enable Modbus control and driver output.
    pub async fn enable_driver(&mut self) -> Result<(), T::Error> {
        self.write_register(RwRegister::ModbusEnable, 1).await?;
        self.write_register(RwRegister::DriverOutputEnable, 1).await
    }

    /// Disable driver output and Modbus control.
    pub async fn disable_driver(&mut self) -> Result<(), T::Error> {
        self.write_register(RwRegister::DriverOutputEnable, 0)
            .await?;
        self.write_register(RwRegister::ModbusEnable, 0).await
    }

    /// Atomic 4-byte absolute position command (vendor function 0x7B).
    pub async fn set_absolute_position(&mut self, steps: i32) -> Result<(), T::Error> {
        let mut request = [0u8; 6];
        request[0] = self.interface.device_addr;
        request[1] = SET_ABSOLUTE_POSITION_FUNC;
        request[2..6].copy_from_slice(&steps.to_be_bytes());
        let mut response = [0u8; 8];
        self.interface
            .transport
            .raw_transaction(&request, &mut response)
            .await?;
        Ok(())
    }

    pub async fn set_speed(&mut self, rpm: u16) -> Result<(), T::Error> {
        self.write_register(RwRegister::MotorTargetSpeed, rpm).await
    }

    pub async fn set_acceleration(&mut self, value: u16) -> Result<(), T::Error> {
        self.write_register(RwRegister::MotorAcceleration, value)
            .await
    }

    pub async fn set_max_output(&mut self, output: u16) -> Result<(), T::Error> {
        self.write_register(RwRegister::StandstillMaxOutput, output)
            .await
    }

    /// Write to the motor's DirPolarity register (running value only; does not
    /// persist to EEPROM unless `ParameterSaveFlag` is also written).
    pub async fn set_dir_polarity(&mut self, reverse: bool) -> Result<(), T::Error> {
        self.write_register(RwRegister::DirPolarity, reverse as u16)
            .await
    }

    /// Provision a new persistent baud rate on the motor.
    ///
    /// This is a one-shot setup: after the magic sequence the motor
    /// stores the rate to its EEPROM and will only respond at the new
    /// rate after a power cycle. The final `ModbusEnable=506` write
    /// triggers the apply step and never sees a response, so its
    /// error is intentionally discarded.
    pub async fn set_baud_rate(&mut self, baud_rate: MotorBaudRate) -> Result<(), T::Error> {
        self.write_register(RwRegister::ModbusEnable, 1).await?;
        self.write_register(RwRegister::MotorAcceleration, baud_rate as u16)
            .await?;
        self.write_register(RwRegister::WeakMagneticAngle, 129).await?;
        let _ = self.write_register(RwRegister::ModbusEnable, 506).await;
        Ok(())
    }

    /// Configure the motor for maximum tracking performance.
    ///
    /// Sets speed, acceleration, and output to maximum so the motor acts
    /// as a pure position servo, following commands from ruckig with
    /// minimal lag.
    pub async fn configure_max_tracking(&mut self) -> Result<(), T::Error> {
        self.set_speed(OPERATING_SPEED_RPM).await?;
        self.set_acceleration(OPERATING_ACCELERATION).await?;
        self.set_max_output(OPERATING_MAX_OUTPUT).await
    }

    /// Trigger the motor's built-in homing sequence.
    pub async fn trigger_homing(&mut self) -> Result<(), T::Error> {
        self.write_register(RwRegister::MotorTargetSpeed, HOME_SPEED_RPM)
            .await?;
        self.write_register(RwRegister::StandstillMaxOutput, HOME_MAX_OUTPUT)
            .await?;
        self.write_register(RwRegister::SpecificFunction, 1).await
    }

    pub async fn read_absolute_position(&mut self) -> Result<i32, T::Error> {
        self.read_register_pair(RwRegister::AbsolutePositionLowU16 as u16)
            .await
    }

    pub async fn read_remaining_steps(&mut self) -> Result<i32, T::Error> {
        self.read_register_pair(RoRegister::TargetPositionLowU16 as u16)
            .await
    }

    pub async fn read_current_amps(&mut self) -> Result<f32, T::Error> {
        let raw = self.read_register(RoRegister::SystemCurrent as u16).await?;
        Ok(raw as f32 / 2000.0)
    }

    pub async fn read_voltage_volts(&mut self) -> Result<f32, T::Error> {
        let raw = self.read_register(RoRegister::SystemVoltage as u16).await?;
        Ok(raw as f32 / 327.0)
    }

    pub async fn read_temperature(&mut self) -> Result<u16, T::Error> {
        self.read_register(RoRegister::SystemTemperature as u16)
            .await
    }

    pub async fn read_alarm_code(&mut self) -> Result<u16, T::Error> {
        self.read_register(RoRegister::AlarmCode as u16).await
    }
}

/// Bring up a 57AIM motor from a freshly opened RS-485 transport.
///
/// Waits for the motor to finish booting, probes at [`TARGET_BAUD_RATE`],
/// and on success returns a ready-to-use motor wrapper. If the motor is
/// silent at the target rate this drops the host UART to
/// [`STOCK_BAUD_RATE`], writes the baud-provisioning sequence, and
/// panics - the motor needs a physical power cycle for the new rate to
/// take effect.
pub async fn provision<T, D>(
    transport: T,
    device_addr: u8,
    motor_config: Motor57AIMConfig,
    delay: D,
) -> Motor57AIM<Modbus<T>, D>
where
    T: ModbusTransport + UartReconfigure,
    D: DelayNs,
{
    let mut motor = Motor57AIM::new(Modbus::new(transport, device_addr), motor_config, delay);
    motor.delay.delay_ms(MOTOR_BOOT_DELAY_MS).await;

    if motor.read_absolute_position().await.is_ok() {
        log::info!("Motor responsive at {} baud", TARGET_BAUD_RATE.as_int());
        return motor;
    }

    log::error!(
        "Motor unresponsive at {} baud; falling back to {} baud to provision",
        TARGET_BAUD_RATE.as_int(),
        STOCK_BAUD_RATE.as_int()
    );

    if motor
        .interface
        .transport
        .reconfigure_baud(STOCK_BAUD_RATE.as_int())
        .await
        .is_err()
    {
        panic!("Failed to reconfigure UART to stock baud");
    }
    motor.delay.delay_ms(POST_DOWNSHIFT_SETTLE_MS).await;

    if motor.set_baud_rate(TARGET_BAUD_RATE).await.is_err() {
        panic!("Failed to write target baud to motor");
    }

    log::error!(
        "Motor baud rate provisioned to {}. Power cycle the motor to apply.",
        TARGET_BAUD_RATE.as_int()
    );
    panic!("Power cycle required after motor baud provisioning");
}

impl<T: ModbusTransport, D: DelayNs> Motor for Motor57AIM<Modbus<T>, D> {
    type Error = T::Error;

    fn steps_per_rev(&self) -> u32 {
        self.config.steps_per_rev
    }

    fn max_output(&self) -> u16 {
        self.config.max_output
    }

    async fn enable(&mut self) -> Result<(), Self::Error> {
        self.enable_driver().await?;
        self.configure_max_tracking().await
    }

    async fn disable(&mut self) -> Result<(), Self::Error> {
        self.disable_driver().await
    }

    async fn set_absolute_position(&mut self, steps: i32) -> Result<(), Self::Error> {
        Motor57AIM::set_absolute_position(self, steps).await
    }

    async fn read_absolute_position(&mut self) -> Result<i32, Self::Error> {
        Motor57AIM::read_absolute_position(self).await
    }

    async fn set_max_output(&mut self, output: u16) -> Result<(), Self::Error> {
        Motor57AIM::set_max_output(self, output).await
    }
}

impl<T: ModbusTransport, D: DelayNs> Rs485Motor for Motor57AIM<Modbus<T>, D> {
    async fn set_dir_polarity(&mut self, reverse: bool) -> Result<(), Self::Error> {
        Motor57AIM::set_dir_polarity(self, reverse).await
    }
}

impl<T: ModbusTransport, D: DelayNs> SelfHoming for Motor57AIM<Modbus<T>, D> {
    async fn home(&mut self) -> Result<(), Self::Error> {
        self.trigger_homing().await?;

        loop {
            self.delay.delay_ms(HOME_POLL_INTERVAL_MS).await;
            let remaining = self.read_remaining_steps().await?;
            if remaining.abs() < HOME_STEP_THRESHOLD {
                break;
            }
        }

        self.delay.delay_ms(POST_HOME_SETTLE_MS).await;

        // Re-enable Modbus (homing resets it on the 57AIM)
        self.enable_driver().await?;
        self.delay.delay_ms(POST_ENABLE_SETTLE_MS).await;

        self.configure_max_tracking().await
    }
}
