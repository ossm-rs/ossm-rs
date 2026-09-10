use crate::{
    config::{
        MAX_STATE_LENGTH, MAX_TRAVEL_MM, MOTION_CONTROL_MAX_VELOCITY, MOTION_CONTROL_MIN_VELOCITY,
    },
    motion_control::set_max_velocity_scaled,
    pattern::{MAX_SENSATION, MIN_SENSATION},
    utils::{saturate_range, scale},
};
use core::{
    fmt::Write,
    sync::atomic::{AtomicBool, AtomicU32, Ordering},
};
use log::error;
use heapless::String;

#[allow(dead_code)]
use num_traits::float::Float;

struct MotionStateStorage {
    depth: AtomicU32,
    motion_length: AtomicU32,
    velocity: AtomicU32,
    sensation: AtomicU32,
    pattern: AtomicU32,
    motion_enabled: AtomicBool,
}

static MOTION_STATE: MotionStateStorage = MotionStateStorage {
    depth: AtomicU32::new(0),
    motion_length: AtomicU32::new(0),
    velocity: AtomicU32::new(0),
    sensation: AtomicU32::new(50),
    pattern: AtomicU32::new(0),
    motion_enabled: AtomicBool::new(false),
};

/// Motion state representation in %
pub struct MotionState {
    // Depth in %
    pub depth: u32,
    // The length of the motion in %
    pub motion_length: u32,
    // Maximum velocity in %
    pub velocity: u32,
    // Sensation in %
    pub sensation: u32,
    // Pattern index
    pub pattern: u32,
    // Whether or not to enable the motion
    pub motion_enabled: bool,
}

impl MotionState {
    pub fn as_json(&self) -> String<MAX_STATE_LENGTH> {
        let mut output = String::new();

        let state_name = if self.motion_enabled {
            "strokeEngine"
        } else {
            "menu"
        };

        if write!(
            output,
            r#"{{"state":"{state_name}","depth":{},"stroke":{},"speed":{},"sensation":{},"pattern":{}}}"#,
            self.depth,
            self.motion_length,
            self.velocity,
            self.sensation,
            self.pattern
        )
        .is_err()
        {
            error!("Could not write the state. Too long");
        }

        output
    }
}

/// Set the motion depth in %
pub fn set_motion_depth_pct(mut depth: u32) {
    if depth > 100 {
        depth = 100;
    }
    MOTION_STATE.depth.store(depth, Ordering::Release);
}

/// Set the motion length in %
pub fn set_motion_length_pct(mut length: u32) {
    if length > 100 {
        length = 100;
    }
    MOTION_STATE.motion_length.store(length, Ordering::Release);
}

/// Set the motion velocity in %
pub fn set_motion_velocity_pct(mut velocity: u32) {
    if velocity > 100 {
        velocity = 100;
    }

    let current_velocity = MOTION_STATE.velocity.load(Ordering::Acquire);
    let current_motion_velocity_mm_s = scale(
        current_velocity as f64,
        0.0,
        100.0,
        MOTION_CONTROL_MIN_VELOCITY,
        MOTION_CONTROL_MAX_VELOCITY,
    );

    let new_motion_velocity_mm_s = scale(
        velocity as f64,
        0.0,
        100.0,
        MOTION_CONTROL_MIN_VELOCITY,
        MOTION_CONTROL_MAX_VELOCITY,
    );

    // We need to update the motion control state to react immediately
    // without having to wait for the pattern to send the next move
    set_max_velocity_scaled(current_motion_velocity_mm_s, new_motion_velocity_mm_s);

    MOTION_STATE.velocity.store(velocity, Ordering::Release);
}

/// Set the motion sensation in %
pub fn set_motion_sensation_pct(mut sensation: u32) {
    if sensation > 100 {
        sensation = 100;
    }

    MOTION_STATE.sensation.store(sensation, Ordering::Release);
}

pub fn set_motion_pattern(index: u32) {
    MOTION_STATE.pattern.store(index, Ordering::Release);
}

/// Set whether the motion is enabled
pub fn set_motion_enabled(enabled: bool) {
    MOTION_STATE
        .motion_enabled
        .store(enabled, Ordering::Release);
}

pub fn get_motion_state() -> MotionState {
    MotionState {
        depth: MOTION_STATE.depth.load(Ordering::Acquire),
        motion_length: MOTION_STATE.motion_length.load(Ordering::Acquire),
        velocity: MOTION_STATE.velocity.load(Ordering::Acquire),
        sensation: MOTION_STATE.sensation.load(Ordering::Acquire),
        pattern: MOTION_STATE.pattern.load(Ordering::Acquire),
        motion_enabled: MOTION_STATE.motion_enabled.load(Ordering::Acquire),
    }
}

/// Motion state representation in machine values e.g. mm instead of %
pub struct MachineMotionState {
    // Depth in mm
    pub depth: f64,
    // The length of the motion in mm
    pub motion_length: f64,
    // Maximum velocity in mm/s
    pub velocity: f64,
    // Sensation from -100 to 100
    pub sensation: f64,
    // Pattern index
    pub pattern: u32,
    // Whether or not to enable the motion
    pub motion_enabled: bool,
}

impl From<MotionState> for MachineMotionState {
    fn from(value: MotionState) -> Self {
        Self {
            depth: scale(value.depth as f64, 0.0, 100.0, 0.0, MAX_TRAVEL_MM),
            motion_length: scale(value.motion_length as f64, 0.0, 100.0, 0.0, MAX_TRAVEL_MM),
            velocity: scale(
                value.velocity as f64,
                0.0,
                100.0,
                MOTION_CONTROL_MIN_VELOCITY,
                MOTION_CONTROL_MAX_VELOCITY,
            ),
            sensation: scale(
                value.sensation as f64,
                0.0,
                100.0,
                MIN_SENSATION,
                MAX_SENSATION,
            ),
            pattern: value.pattern,
            motion_enabled: value.motion_enabled,
        }
    }
}

/// Set the motion depth in mm
pub fn set_motion_depth_mm(depth: u32) {
    let depth = saturate_range(depth as f64, 0.0, MAX_TRAVEL_MM);

    let depth_pct = scale(depth, 0.0, MAX_TRAVEL_MM, 0.0, 100.0) as u32;

    set_motion_depth_pct(depth_pct);
}

/// Set the motion length in mm
pub fn set_motion_length_mm(length: u32) {
    let length_pct = scale(length as f64, 0.0, MAX_TRAVEL_MM, 0.0, 100.0) as u32;

    set_motion_length_pct(length_pct);
}

/// Set the motion velocity in mm/s
pub fn set_motion_velocity_mm_s(velocity: u32) {
    let velocity_pct = scale(
        velocity as f64,
        MOTION_CONTROL_MIN_VELOCITY,
        MOTION_CONTROL_MAX_VELOCITY,
        0.0,
        100.0,
    ) as u32;

    set_motion_velocity_pct(velocity_pct);
}

/// Set the motion sensation in a range from -100 to 100
pub fn set_motion_sensation_neg_pos_100(mut sensation: i32) {
    if (sensation as f64) > MAX_SENSATION {
        sensation = MAX_SENSATION.floor() as i32;
    }
    if (sensation as f64) < MIN_SENSATION {
        sensation = MIN_SENSATION.ceil() as i32;
    }

    let sensation_pct = scale(sensation as f64, MIN_SENSATION, MAX_SENSATION, 0.0, 100.0) as u32;

    set_motion_sensation_pct(sensation_pct);
}
