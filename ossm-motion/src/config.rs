// ---- User Parameters ----
const PULLEY_TOOTH_COUNT: f64 = 20.0;
const BELT_PITCH: f64 = 2.0;
// The minimum allowed move forward from the homing position
pub const MIN_MOVE_MM: f64 = 10.0;
// The maximum allowed move forward from the homing position
pub const MAX_MOVE_MM: f64 = 190.0;
// The max total travel distance of the machine
pub const MAX_TRAVEL_MM: f64 = MAX_MOVE_MM - MIN_MOVE_MM;
// Retracts the machine when the motion is disabled if true or just stops it if false
pub const RETRACT_ON_MOTION_DISABLED: bool = true;
// The velocity at which the machine retracts when it is turned off
// or switching to a different a pattern in mm/s
pub const RETRACT_VELOCITY: f64 = MOTION_CONTROL_MAX_VELOCITY / 4.0;
// Change this if your machine is going the wrong way
pub const REVERSE_DIRECTION: bool = false;

// ---- Critical parameters. No touchy unless you know what you are doing ----
// Using the full encoder resolution
const MOTOR_STEPS_PER_REVOLUTION: f64 = 32768.0;
// How often the motion control loop runs
pub const MOTION_CONTROL_LOOP_UPDATE_INTERVAL_MS: u64 = 10;
// In mm/s
// Has to be larger than 0
pub const MOTION_CONTROL_MIN_VELOCITY: f64 = 0.001;
// In mm/s
pub const MOTION_CONTROL_MAX_VELOCITY: f64 = 600.0;
// In mm/s²
pub const MOTION_CONTROL_MAX_ACCELERATION: f64 = 30000.0;
// In mm/s³
pub const MOTION_CONTROL_MAX_JERK: f64 = 100000.0;
// pub const MOTION_CONTROL_MAX_VELOCITY: f64 = 10000.0;
// // In mm/s²
// pub const MOTION_CONTROL_MAX_ACCELERATION: f64 = 100000.0;
// // In mm/s³
// pub const MOTION_CONTROL_MAX_JERK: f64 = 100000.0;
// Turn the machine off after no heartbeat was received for this long
pub const MAX_NO_REMOTE_HEARTBEAT_MS: u64 = 8000;
// Min output in torque mode. 0-60
pub const MOTOR_MIN_OUTPUT: f64 = 12.0;
// Max output in torque mode. 0-60
pub const MOTOR_MAX_OUTPUT: f64 = 60.0;

// ---- BLE parameters ----
pub const CONNECTIONS_MAX: usize = 1;
pub const L2CAP_CHANNELS_MAX: usize = 2;
pub const MAX_COMMAND_LENGTH: usize = 64;
pub const MAX_STATE_LENGTH: usize = 128;
pub const MAX_PATTERN_LENGTH: usize = 256;

// ---- Calculated parameters ----
pub const STEPS_PER_MM: f64 = MOTOR_STEPS_PER_REVOLUTION / (PULLEY_TOOTH_COUNT * BELT_PITCH);
pub const MM_PER_ROTATION: f64 = MOTOR_STEPS_PER_REVOLUTION / STEPS_PER_MM;
pub const MAX_RPM: u16 = ((MOTION_CONTROL_MAX_VELOCITY / STEPS_PER_MM) * 60.0) as u16;
