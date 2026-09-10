use core::f64::INFINITY;

use log::info;
use embassy_time::{Duration, Ticker, Timer};
pub mod motion_state;

use crate::{
    config::{
        MIN_MOVE_MM, MOTION_CONTROL_MIN_VELOCITY, RETRACT_ON_MOTION_DISABLED, RETRACT_VELOCITY,
    },
    motion::motion_state::{MachineMotionState, get_motion_state},
    motion_control::{self, set_max_velocity, set_target_position, set_torque},
    pattern::{Pattern, PatternExecutor, PatternInput, PatternMove},
};

async fn retract() {
    let motion_state: MachineMotionState = get_motion_state().into();

    set_target_position(MIN_MOVE_MM);
    set_max_velocity(RETRACT_VELOCITY);
    while motion_control::is_move_in_progress() {
        Timer::after(Duration::from_millis(10)).await;
    }
    // Restore the previous velocity
    set_max_velocity(motion_state.velocity);
}

pub async fn run_motion() {
    let mut ticker = Ticker::every(Duration::from_millis(10));
    let mut prev_motion_enabled = false;

    let mut pattern_executor = PatternExecutor::new();
    let mut prev_pattern: u32 = 0;
    let mut pattern_move = PatternMove::default();
    let mut prev_pattern_move = PatternMove::default();
    // Values to be overriden on the first move
    prev_pattern_move.velocity = INFINITY;
    prev_pattern_move.torque = INFINITY;

    info!("Task Motion Started");

    loop {
        let motion_state: MachineMotionState = get_motion_state().into();

        // Retract the machine if motion was disabled
        if !motion_state.motion_enabled && prev_motion_enabled {
            if RETRACT_ON_MOTION_DISABLED {
                pattern_executor.reset();
                retract().await;
            } else {
                set_max_velocity(MOTION_CONTROL_MIN_VELOCITY);
            }
        }

        if motion_state.motion_enabled && !prev_motion_enabled {
            // Restore the previous velocity
            if !RETRACT_ON_MOTION_DISABLED {
                set_max_velocity(pattern_move.velocity);
            }
        }

        if motion_state.pattern != prev_pattern {
            pattern_executor.set_pattern(motion_state.pattern);
            pattern_executor.reset();
            info!(
                "Pattern set to: {}",
                pattern_executor.get_current_pattern_name()
            );
            prev_pattern = motion_state.pattern;
        }

        if !motion_control::is_move_in_progress() && motion_state.motion_enabled {
            // Apply the delay from the previous move before executing the next one
            Timer::after_millis(pattern_move.delay_ms).await;

            let input = PatternInput {
                velocity: motion_state.velocity,
                depth: motion_state.depth,
                motion_length: motion_state.motion_length,
                sensation: motion_state.sensation,
            };

            // A move with all the constraints met
            pattern_move = pattern_executor.next_move(&input);

            if pattern_move.velocity != prev_pattern_move.velocity {
                set_max_velocity(pattern_move.velocity);
            }
            if pattern_move.torque != prev_pattern_move.torque {
                set_torque(pattern_move.torque);
            }
            set_target_position(pattern_move.position);

            prev_pattern_move = pattern_move;
        } else {
            ticker.next().await;
        }

        prev_motion_enabled = motion_state.motion_enabled;
    }
}
