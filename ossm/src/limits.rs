/// Motion limits in physical units.
///
/// These are enforced by the motion controller via ruckig. No motion command
/// can exceed these limits regardless of what the upstream software requests.
///
/// Position limits define the safe travel range. The controller clamps all
/// position commands to this range before feeding them to ruckig.
#[derive(Debug, Clone)]
pub struct MotionLimits {
    /// Closest point to the home position the machine should move to (mm).
    pub min_position_mm: f64,
    /// Furthest point from the home position the machine should move to (mm).
    pub max_position_mm: f64,
    /// Maximum linear velocity (mm/s).
    pub max_velocity_mm_s: f64,
    /// Maximum linear acceleration (mm/s²).
    pub max_acceleration_mm_s2: f64,
    /// Maximum linear jerk (mm/s³).
    pub max_jerk_mm_s3: f64,
}

impl MotionLimits {
    pub const DEFAULT: Self = Self {
        min_position_mm: 10.0,
        max_position_mm: 190.0,
        max_velocity_mm_s: 600.0,
        max_acceleration_mm_s2: 30_000.0,
        max_jerk_mm_s3: 2_800_000.0,
    };
}

impl Default for MotionLimits {
    fn default() -> Self {
        Self::DEFAULT
    }
}
