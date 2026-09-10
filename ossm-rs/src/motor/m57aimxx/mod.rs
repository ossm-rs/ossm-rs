pub mod config;

use log::{debug, error};
use embedded_io::Write;
use enum_iterator::Sequence;
use esp_hal::{
    time::Duration,
    timer::{AnyTimer, Timer},
    uart::{RxError, Uart},
    Blocking,
};
use heapless::Vec;
use rmodbus::{client::ModbusRequest, guess_response_frame_len, ModbusProto};

const PROTO: ModbusProto = ModbusProto::Rtu;
const MIN_REG_READ_REQUIRED: usize = 3;

const MOTOR_TIMEOUT_MS: u64 = 10;
const MOTOR_SHORT_TIMEOUT_MS: u64 = 3;
pub const MOTOR_CONSECUTIVE_READ_DELAY_US: u64 = 2000;

const MAX_REG_READ_AT_ONCE: usize = 8;

pub const MAX_MOTOR_SPEED_RPM: u16 = 3000;

#[derive(Debug, Clone, Copy, PartialEq, Sequence)]
#[repr(u16)]
pub enum ReadWriteMotorRegisters {
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

#[derive(Debug, Clone, Copy, PartialEq, Sequence)]
#[repr(u16)]
pub enum ReadOnlyMotorRegisters {
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

pub trait ReadableMotorRegister {
    fn addr(&self) -> u16;
}

impl ReadableMotorRegister for ReadWriteMotorRegisters {
    fn addr(&self) -> u16 {
        *self as u16
    }
}

impl ReadableMotorRegister for ReadOnlyMotorRegisters {
    fn addr(&self) -> u16 {
        *self as u16
    }
}

#[allow(dead_code)]
#[repr(u16)]
pub enum MotorBaudRate {
    Baud115200 = 803,
    Baud38400 = 802,
    Baud19200 = 801,
    Baud9600 = 800,
}

impl MotorBaudRate {
    pub fn as_int(&self) -> u32 {
        match self {
            MotorBaudRate::Baud115200 => 115200,
            MotorBaudRate::Baud38400 => 38400,
            MotorBaudRate::Baud19200 => 19200,
            MotorBaudRate::Baud9600 => 9600,
        }
    }
}

#[allow(dead_code)]
#[derive(Debug)]
pub enum MotorError {
    Rs485Error(RxError),
    Timeout,
}

// Taken from the rmodbus crate
fn calc_crc16(frame: &[u8], data_length: u8) -> u16 {
    let mut crc: u16 = 0xffff;
    for i in frame.iter().take(data_length as usize) {
        crc ^= u16::from(*i);
        for _ in (0..8).rev() {
            if (crc & 0x0001) == 0 {
                crc >>= 1;
            } else {
                crc >>= 1;
                crc ^= 0xA001;
            }
        }
    }
    crc
}

pub struct Motor57AIMxx {
    rs485: Uart<'static, Blocking>,
    timer: AnyTimer<'static>,
}

impl Motor57AIMxx {
    pub fn new(rs485: Uart<'static, Blocking>, timer: AnyTimer<'static>) -> Self {
        Self { rs485, timer }
    }

    pub fn release(self) -> (Uart<'static, Blocking>, AnyTimer<'static>) {
        (self.rs485, self.timer)
    }

    fn start_timer_delay(&mut self, delay: Duration) {
        if self.timer.is_running() {
            self.timer.stop();
        }

        self.timer.clear_interrupt();
        self.timer.reset();

        self.timer.enable_auto_reload(false);
        self.timer.load_value(delay).unwrap();
        self.timer.start();
    }

    pub fn delay(&mut self, delay: Duration) {
        self.start_timer_delay(delay);

        while !self.timer.is_interrupt_set() {}

        self.timer.stop();
        self.timer.clear_interrupt();
    }

    fn read_with_given_timeout(
        &mut self,
        mut buf: &mut [u8],
        timeout_ms: u64,
    ) -> Result<(), MotorError> {
        self.start_timer_delay(Duration::from_millis(timeout_ms));

        while !buf.is_empty() && !self.timer.is_interrupt_set() {
            match self.rs485.read_buffered(buf) {
                Ok(n) => buf = &mut buf[n..],
                Err(e) => return Err(MotorError::Rs485Error(e)),
            }
        }

        let timeout = self.timer.is_interrupt_set();

        self.timer.stop();
        self.timer.clear_interrupt();

        if timeout {
            return Err(MotorError::Timeout);
        }

        Ok(())
    }

    fn read_with_timeout(&mut self, buf: &mut [u8]) -> Result<(), MotorError> {
        self.read_with_given_timeout(buf, MOTOR_TIMEOUT_MS)
    }

    /// Write one motor register
    pub fn write_register(
        &mut self,
        reg: &ReadWriteMotorRegisters,
        val: u16,
    ) -> Result<(), MotorError> {
        let mut modbus_req = ModbusRequest::new(1, PROTO);
        let mut request: Vec<u8, 32> = Vec::new();

        modbus_req
            .generate_set_holding(reg.addr(), val, &mut request)
            .expect("Failed to generate reg write request");

        self.rs485
            .write_all(&request)
            .expect("Failed to write the request bytes to RS485");
        self.rs485.flush().expect("Failed to flush RS485");

        let mut response = [0u8; 32];
        self.read_with_timeout(&mut response[0..MIN_REG_READ_REQUIRED])?;

        let len = guess_response_frame_len(&response[0..MIN_REG_READ_REQUIRED], PROTO)
            .expect("Failed to guess frame len") as usize;
        if len > MIN_REG_READ_REQUIRED {
            self.read_with_timeout(&mut response[MIN_REG_READ_REQUIRED..len])?;
        }
        let response = &response[0..len];

        modbus_req.parse_ok(response).expect("Modbus error");

        // Make sure that multiple operations in a row can succeed
        self.delay(Duration::from_micros(MOTOR_CONSECUTIVE_READ_DELAY_US));

        Ok(())
    }

    /// Read one or more motor registers
    pub fn read_registers<T: ReadableMotorRegister>(
        &mut self,
        reg: &T,
        count: u16,
    ) -> Result<Vec<u16, MAX_REG_READ_AT_ONCE>, MotorError> {
        let mut modbus_req = ModbusRequest::new(1, PROTO);
        let mut request: Vec<u8, 32> = Vec::new();

        modbus_req
            .generate_get_holdings(reg.addr(), count, &mut request)
            .expect("Failed to generate reg read request");

        debug!("Req {:x?}", request);
        self.rs485
            .write_all(&request)
            .expect("Failed to write the request bytes to RS485");
        self.rs485.flush().expect("Failed to flush RS485");

        // let now = Instant::now();

        let mut response = [0u8; 32];
        self.read_with_timeout(&mut response[0..MIN_REG_READ_REQUIRED])?;

        let len = guess_response_frame_len(&response[0..MIN_REG_READ_REQUIRED], PROTO)
            .expect("Failed to guess frame len") as usize;
        if len > MIN_REG_READ_REQUIRED {
            self.read_with_timeout(&mut response[MIN_REG_READ_REQUIRED..len])?;
        }
        let response = &response[0..len];

        // let elapsed = now.elapsed().as_micros();
        // info!("Motor responded in {} us", elapsed);

        let mut res: Vec<u16, MAX_REG_READ_AT_ONCE> = Vec::new();
        modbus_req
            .parse_u16(response, &mut res)
            .expect("Failed to parse response reg");

        // Make sure that multiple operations in a row can succeed
        self.delay(Duration::from_micros(MOTOR_CONSECUTIVE_READ_DELAY_US));

        Ok(res)
    }

    /// Read one motor register
    pub fn read_register<T: ReadableMotorRegister>(&mut self, reg: &T) -> Result<u16, MotorError> {
        Ok(self.read_registers(reg, 1)?[0])
    }

    /// Set the absolute position using the custom 0x7b command
    pub fn set_absolute_position(&mut self, position: i32) -> Result<(), MotorError> {
        let mut request = [0u8; 8];
        let bytes = position.to_be_bytes();

        request[0] = 0x1;
        request[1] = 0x7b;
        request[2..6].copy_from_slice(&bytes);
        let crc = calc_crc16(&request[0..6], 6).to_le_bytes();
        request[6..8].copy_from_slice(&crc);

        // info!("Request {:x}", request);

        self.rs485
            .write_all(&request)
            .expect("Failed to write the request bytes to RS485");
        self.rs485.flush().expect("Failed to flush RS485");

        let mut response = [0u8; 32];
        self.read_with_given_timeout(&mut response[0..8], MOTOR_SHORT_TIMEOUT_MS)?;

        if response[0..2] != [0x1, 0x7b] {
            error!(
                "Incorrect response to a 0x7b command: {:x?}",
                &response[0..8]
            );
        }

        // Delay not necessary because we prioritise the update rate over missed positions
        Ok(())
    }

    /// Set the motor baud rate
    pub fn set_baud_rate(&mut self, baud_rate: MotorBaudRate) -> Result<(), MotorError> {
        // Magic sequence
        self.write_register(&ReadWriteMotorRegisters::ModbusEnable, 1)?;
        self.write_register(
            &ReadWriteMotorRegisters::MotorAcceleration,
            baud_rate as u16,
        )?;
        self.write_register(&ReadWriteMotorRegisters::WeakMagneticAngle, 129)?;
        // No response for the last one
        self.write_register(&ReadWriteMotorRegisters::ModbusEnable, 506)
            .ok();

        Ok(())
    }

    /// Wait for the target position to be below the threshold
    pub fn wait_for_target_reached(&mut self, threshold: i32) {
        loop {
            let target_position = self
                .get_target_position()
                .expect("Failed to get target position");

            debug!("Target {}", target_position,);
            if target_position.abs() < threshold {
                break;
            }
            self.delay(Duration::from_micros(MOTOR_CONSECUTIVE_READ_DELAY_US * 2));
        }
    }

    // ---- RO regs ----

    /// Get the current in A
    pub fn get_current(&mut self) -> Result<f32, MotorError> {
        let reg = self.read_register(&ReadOnlyMotorRegisters::SystemCurrent)?;
        let current = reg as f32 / 2000.0;
        Ok(current)
    }

    /// Get the voltage in V
    pub fn get_voltage(&mut self) -> Result<f32, MotorError> {
        let reg = self.read_register(&ReadOnlyMotorRegisters::SystemVoltage)?;
        let voltage = reg as f32 / 327.0;
        Ok(voltage)
    }

    /// Get how many steps need to be taken to reach the target
    pub fn get_target_position(&mut self) -> Result<i32, MotorError> {
        let regs = self.read_registers(&ReadOnlyMotorRegisters::TargetPositionLowU16, 2)?;
        let bytes = (((regs[1] as u32) << 16) | regs[0] as u32).to_le_bytes();
        let target_position = i32::from_le_bytes(bytes);
        Ok(target_position)
    }

    // ---- RW regs ----

    /// Enables modbus
    pub fn enable_modbus(&mut self, enable: bool) -> Result<(), MotorError> {
        self.write_register(&ReadWriteMotorRegisters::ModbusEnable, enable as u16)
    }

    pub fn get_target_speed(&mut self) -> Result<u16, MotorError> {
        self.read_register(&ReadWriteMotorRegisters::MotorTargetSpeed)
    }

    /// Set the target speed in RPM 0-3000
    pub fn set_target_speed(&mut self, speed: u16) -> Result<(), MotorError> {
        if speed > 3000 {
            panic!("The speed cannot be more than 3000")
        }

        self.write_register(&ReadWriteMotorRegisters::MotorTargetSpeed, speed)
    }

    /// Get the target acceleration 0-59999. 60000 means disabled
    pub fn get_target_acceleration(&mut self) -> Result<u16, MotorError> {
        self.read_register(&ReadWriteMotorRegisters::MotorAcceleration)
    }

    /// 0-59999. 60000 means disabled
    pub fn set_target_acceleration(&mut self, acceleration: u16) -> Result<(), MotorError> {
        self.write_register(&ReadWriteMotorRegisters::MotorAcceleration, acceleration)
    }

    /// Set the speed proportional coefficient
    pub fn set_speed_proportional_coefficient(
        &mut self,
        coefficient: u16,
    ) -> Result<(), MotorError> {
        self.write_register(
            &ReadWriteMotorRegisters::SpeedRingProportionalCoefficient,
            coefficient,
        )
    }

    /// Set the position proportional coefficient
    pub fn set_position_proportional_coefficient(
        &mut self,
        coefficient: u16,
    ) -> Result<(), MotorError> {
        self.write_register(
            &ReadWriteMotorRegisters::PositionRingProportionalCoefficient,
            coefficient,
        )
    }

    /// 0 - Applying external DIR results in anticlockwise rotation
    /// 1 - Applying external DIR results in clockwise rotation
    pub fn set_dir_polarity(&mut self, polarity: bool) -> Result<(), MotorError> {
        self.write_register(&ReadWriteMotorRegisters::DirPolarity, polarity as u16)
    }

    /// Get the absolute position in encoder pulses
    pub fn get_abolute_position(&mut self) -> Result<i32, MotorError> {
        let regs = self.read_registers(&ReadWriteMotorRegisters::AbsolutePositionLowU16, 2)?;
        let bytes = (((regs[1] as u32) << 16) | regs[0] as u32).to_le_bytes();
        let absolute_position = i32::from_le_bytes(bytes);

        Ok(absolute_position)
    }

    /// Get the maximum allowed output in the standstill
    pub fn get_max_allowed_output(&mut self) -> Result<u16, MotorError> {
        self.read_register(&ReadWriteMotorRegisters::StandstillMaxOutput)
    }

    /// Set the maximum allowed output in the standstill
    /// First two digits 0-60 represent the output
    /// Last digit 0-9 represents the alarm. 0 means no alarm and the output is reduced by half after 3 s
    pub fn set_max_allowed_output(&mut self, output: u16) -> Result<(), MotorError> {
        self.write_register(&ReadWriteMotorRegisters::StandstillMaxOutput, output)
    }

    /// Home automatically
    pub fn home(&mut self) -> Result<(), MotorError> {
        self.write_register(&ReadWriteMotorRegisters::SpecificFunction, 1)
    }
}

impl ossm_motion::motion_control::motor::Motor for Motor57AIMxx {
    type MotorError = MotorError;

    fn min_consecutive_write_delay() -> ossm_motion::motion_control::timer::Duration {
        ossm_motion::motion_control::timer::Duration::micros(MOTOR_CONSECUTIVE_READ_DELAY_US)
    }

    fn set_absolute_position(&mut self, steps: i32) -> Result<(), Self::MotorError> {
        self.set_absolute_position(steps)
    }

    fn set_max_allowed_output(&mut self, torque: u16) -> Result<(), MotorError> {
        self.set_max_allowed_output(torque)
    }

    fn delay(&mut self, duration: ossm_motion::motion_control::timer::Duration) {
        self.delay(Duration::from_micros(duration.to_micros()));
    }
}
