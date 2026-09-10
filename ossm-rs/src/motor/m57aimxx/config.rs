use crate::motor::m57aimxx::MotorBaudRate;

// The baud rate that your motor comes with. Will be automatically changed at startup
pub const STOCK_MOTOR_BAUD_RATE: MotorBaudRate = MotorBaudRate::Baud19200;
// Motor baud rate to be used by the firmware
pub const MOTOR_BAUD_RATE: MotorBaudRate = MotorBaudRate::Baud115200;
