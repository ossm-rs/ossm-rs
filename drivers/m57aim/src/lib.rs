#![no_std]

mod rs485;
mod stepdir;

pub use ossm::{Modbus, ModbusTransport};
pub use rs485::provision;

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u16)]
pub enum RwRegister {
    ModbusEnable = 0x00,
    DriverOutputEnable = 0x01,
    MotorTargetSpeed = 0x02,
    MotorAcceleration = 0x03,
    WeakMagneticAngle = 0x04,
    SpeedRingProportionalCoefficient = 0x05,
    SpeedLoopIntegrationTime = 0x06,
    PositionRingProportionalCoefficient = 0x07,
    SpeedFeedForward = 0x08,
    DirPolarity = 0x09,
    ElectronicGearNumerator = 0x0A,
    ElectronicGearDenominator = 0x0B,
    ParameterSaveFlag = 0x14,
    AbsolutePositionLowU16 = 0x16,
    AbsolutePositionHighU16 = 0x17,
    StandstillMaxOutput = 0x18,
    SpecificFunction = 0x19,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u16)]
pub enum RoRegister {
    TargetPositionLowU16 = 0x0C,
    TargetPositionHighU16 = 0x0D,
    AlarmCode = 0x0E,
    SystemCurrent = 0x0F,
    MotorCurrentSpeed = 0x10,
    SystemVoltage = 0x11,
    SystemTemperature = 0x12,
    SystemOutputPwm = 0x13,
    DeviceAddress = 0x15,
}

pub struct Motor57AIMConfig {
    pub steps_per_rev: u32,
    pub max_output: u16,
}

impl Default for Motor57AIMConfig {
    fn default() -> Self {
        Self {
            steps_per_rev: 32_768,
            max_output: 600,
        }
    }
}

pub const DEFAULT_DEVICE_ADDR: u8 = 0x01;

/// Baud rate that the motor ships with from the factory.
pub const STOCK_BAUD_RATE: MotorBaudRate = MotorBaudRate::Baud19200;

/// Baud rate the firmware uses at runtime. Provisioned into the motor
/// once via [`Motor57AIM::set_baud_rate`] when first encountered at the
/// stock rate; persists across power cycles thereafter.
pub const TARGET_BAUD_RATE: MotorBaudRate = MotorBaudRate::Baud115200;

/// 57AIM baud rate. The discriminants are the motor's magic register
/// codes used when writing the baud through the provisioning sequence,
/// not the integer baud itself - call [`MotorBaudRate::as_int`] for
/// the actual UART rate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum MotorBaudRate {
    Baud9600 = 800,
    Baud19200 = 801,
    Baud38400 = 802,
    Baud115200 = 803,
}

impl MotorBaudRate {
    pub const fn as_int(self) -> u32 {
        match self {
            MotorBaudRate::Baud9600 => 9_600,
            MotorBaudRate::Baud19200 => 19_200,
            MotorBaudRate::Baud38400 => 38_400,
            MotorBaudRate::Baud115200 => 115_200,
        }
    }
}

/// 57AIM BLDC servo motor, generic over communication interface and delay.
pub struct Motor57AIM<I, D> {
    pub(crate) interface: I,
    pub(crate) config: Motor57AIMConfig,
    pub(crate) delay: D,
}

impl<I, D> Motor57AIM<I, D> {
    pub fn new(interface: I, config: Motor57AIMConfig, delay: D) -> Self {
        Self {
            interface,
            config,
            delay,
        }
    }
}
