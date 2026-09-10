use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::watch::Watch;

#[derive(Debug, Clone, Copy)]
pub struct PatternInput {
    /// Maximum depth as a fraction of the machine range (0.0–1.0).
    pub depth: f64,
    /// Stroke length as a fraction of the full machine travel (0.0–1.0).
    /// Shallowest point = `depth - stroke`.
    pub stroke: f64,
    /// Velocity as a fraction of max velocity (0.0–1.0).
    pub velocity: f64,
    /// Sensation value (-1.0 to 1.0). Meaning is pattern-specific.
    pub sensation: f64,
}

impl PatternInput {
    pub const DEFAULT: Self = Self {
        depth: 0.5,
        stroke: 0.5,
        velocity: 0.0,
        sensation: 0.0,
    };
}

impl Default for PatternInput {
    fn default() -> Self {
        Self::DEFAULT
    }
}

pub type SharedPatternInput = Watch<CriticalSectionRawMutex, PatternInput, 1>;
