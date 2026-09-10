pub mod timer;

use crate::{
    config::{MIN_MOVE_MM, REVERSE_DIRECTION, STEPS_PER_MM},
    motor::m57aimxx::{Motor57AIMxx, MAX_MOTOR_SPEED_RPM},
};
use log::info;

/// Set the default motor settings
pub fn set_motor_settings(motor: &mut Motor57AIMxx) {
    // Set high speed and acceleration since those are controlled by motion control
    motor
        .set_target_speed(MAX_MOTOR_SPEED_RPM)
        .expect("Failed to set target speed");
    motor
        .set_target_acceleration(50000)
        .expect("Failed to set target acceleration");

    // Defaults from OSSM
    motor
        .set_speed_proportional_coefficient(3000)
        .expect("Failed to set speed proportional coefficient");
    motor
        .set_position_proportional_coefficient(3000)
        .expect("Failed to set position proportional coefficient");
    motor
        .set_max_allowed_output(600)
        .expect("Failed to set max allowed output");
}

/// Home and wait until done
pub fn wait_for_home(motor: &mut Motor57AIMxx) {
    // Set slower speed and output for homing
    motor
        .set_target_speed(80)
        .expect("Failed to set target speed");
    motor
        .set_max_allowed_output(89)
        .expect("Failed to set max allowed output");
    motor
        .set_dir_polarity(REVERSE_DIRECTION)
        .expect("Failed to set direction polarity");

    motor.home().expect("Failed to start homing");

    info!("Homing...");
    motor.wait_for_target_reached(15);
    info!("Homing Done");

    motor.delay(esp_hal::time::Duration::from_millis(20));

    // Enabling modbus seems to reset the target speed and the max allowed output to default
    motor.enable_modbus(true).expect("Failed to enable modbus");

    motor.delay(esp_hal::time::Duration::from_millis(800));

    let mut new_steps = MIN_MOVE_MM * STEPS_PER_MM;
    if !REVERSE_DIRECTION {
        new_steps = -new_steps;
    }

    motor
        .set_target_speed(100)
        .expect("Failed to set target speed");
    motor
        .set_absolute_position(new_steps as i32)
        .expect("Failed to move to the minimum position");

    motor.delay(esp_hal::time::Duration::from_millis(20));

    motor.wait_for_target_reached(15);

    info!("Moved to minimum position");
}

#[embassy_executor::task]
pub async fn run_motion() {
    ossm_motion::motion::run_motion().await;
}
