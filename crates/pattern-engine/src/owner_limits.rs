use core::sync::atomic::{AtomicBool, Ordering};
use portable_atomic::AtomicU64;

use crate::PatternInput;

// Conservative fallback retained for diagnostics/legacy compatibility. Persisted owner
// limits are now authoritative whenever the master limit system is enabled.
pub const FALLBACK_MAX_SPEED_MM_S: f64 = 10.0;
pub const FALLBACK_MIN_STROKE_MM: f64 = 0.0;
pub const FALLBACK_MAX_STROKE_MM: f64 = 20.0;
pub const FALLBACK_MIN_DEPTH_MM: f64 = 0.0;
pub const FALLBACK_MAX_DEPTH_MM: f64 = 20.0;

pub const DEFAULT_OWNER_MAX_SPEED_MM_S: f64 = 150.0;
pub const DEFAULT_OWNER_MIN_STROKE_MM: f64 = 0.0;
pub const DEFAULT_OWNER_MAX_STROKE_MM: f64 = 100.0;
pub const DEFAULT_OWNER_MIN_DEPTH_MM: f64 = 0.0;
pub const DEFAULT_OWNER_MAX_DEPTH_MM: f64 = 160.0;

// Absolute firmware/hardware envelope. Owner-page machine settings may reduce
// these values but may never expand beyond the compiled safe MotionLimits.
pub const HARD_MAX_MACHINE_SPEED_MM_S: f64 = 900.0;
pub const HARD_MAX_MACHINE_TRAVEL_MM: f64 = 400.0;

// Conservative migration/default values. Firmware overwrites these at boot
// from persisted machine settings before constructing the motion controller.
// Raising the configurable ceilings must not silently expand an existing machine.
pub const DEFAULT_MACHINE_SPEED_MM_S: f64 = 600.0;
pub const DEFAULT_MACHINE_TRAVEL_MM: f64 = 180.0;
static MACHINE_MAX_SPEED_MM_S: AtomicU64 = AtomicU64::new(DEFAULT_MACHINE_SPEED_MM_S.to_bits());
static MACHINE_TRAVEL_MM: AtomicU64 = AtomicU64::new(DEFAULT_MACHINE_TRAVEL_MM.to_bits());

// Master deliberately starts OFF on every boot. This provides stock-like use
// without a PC while still allowing a local owner to explicitly enable the layer.
static MASTER_ENABLED: AtomicBool = AtomicBool::new(false);
static OWNER_SESSION_ACTIVE: AtomicBool = AtomicBool::new(false);
// Software E-stop latch. This is intentionally independent of the
// owner master/session state so it can stop stock, fallback, and owner modes.
static ESTOP_ACTIVE: AtomicBool = AtomicBool::new(false);

static OWNER_MAX_SPEED: AtomicU64 = AtomicU64::new(DEFAULT_OWNER_MAX_SPEED_MM_S.to_bits());
static OWNER_MIN_STROKE: AtomicU64 = AtomicU64::new(DEFAULT_OWNER_MIN_STROKE_MM.to_bits());
static OWNER_MAX_STROKE: AtomicU64 = AtomicU64::new(DEFAULT_OWNER_MAX_STROKE_MM.to_bits());
static OWNER_MIN_DEPTH: AtomicU64 = AtomicU64::new(DEFAULT_OWNER_MIN_DEPTH_MM.to_bits());
static OWNER_MAX_DEPTH: AtomicU64 = AtomicU64::new(DEFAULT_OWNER_MAX_DEPTH_MM.to_bits());

// Preserve the remote's requested values independently of the effective
// clamped values sent to the pattern engine. This is critical when moving
// FALLBACK -> OWNER or MASTER OFF: otherwise a previously clamped value
// (e.g. 10 mm/s) becomes the new source value and cannot be restored.
static REQUESTED_DEPTH: AtomicU64 = AtomicU64::new(PatternInput::DEFAULT.depth.to_bits());
static REQUESTED_STROKE: AtomicU64 = AtomicU64::new(PatternInput::DEFAULT.stroke.to_bits());
static REQUESTED_VELOCITY: AtomicU64 = AtomicU64::new(PatternInput::DEFAULT.velocity.to_bits());
static REQUESTED_SENSATION: AtomicU64 = AtomicU64::new(PatternInput::DEFAULT.sensation.to_bits());

#[derive(Clone, Copy, Debug)]
pub struct ActiveLimits {
    pub max_speed: f64,
    pub min_stroke: f64,
    pub max_stroke: f64,
    pub min_depth: f64,
    pub max_depth: f64,
}


pub fn set_requested_input(input: PatternInput) {
    REQUESTED_DEPTH.store(input.depth.clamp(0.0, 1.0).to_bits(), Ordering::Release);
    REQUESTED_STROKE.store(input.stroke.clamp(0.0, 1.0).to_bits(), Ordering::Release);
    REQUESTED_VELOCITY.store(input.velocity.clamp(0.0, 1.0).to_bits(), Ordering::Release);
    REQUESTED_SENSATION.store(input.sensation.clamp(-1.0, 1.0).to_bits(), Ordering::Release);
}

pub fn set_requested_speed(value: f64) {
    REQUESTED_VELOCITY.store(value.clamp(0.0, 1.0).to_bits(), Ordering::Release);
}

pub fn set_requested_stroke(value: f64) {
    REQUESTED_STROKE.store(value.clamp(0.0, 1.0).to_bits(), Ordering::Release);
}

pub fn set_requested_depth(value: f64) {
    REQUESTED_DEPTH.store(value.clamp(0.0, 1.0).to_bits(), Ordering::Release);
}

pub fn set_requested_sensation(value: f64) {
    REQUESTED_SENSATION.store(value.clamp(-1.0, 1.0).to_bits(), Ordering::Release);
}

pub fn requested_input() -> PatternInput {
    PatternInput {
        depth: f64::from_bits(REQUESTED_DEPTH.load(Ordering::Acquire)),
        stroke: f64::from_bits(REQUESTED_STROKE.load(Ordering::Acquire)),
        velocity: f64::from_bits(REQUESTED_VELOCITY.load(Ordering::Acquire)),
        sensation: f64::from_bits(REQUESTED_SENSATION.load(Ordering::Acquire)),
    }
}

pub fn configure_machine(max_speed_mm_s: f64, travel_mm: f64) {
    let max_speed_mm_s = max_speed_mm_s.clamp(1.0, HARD_MAX_MACHINE_SPEED_MM_S);
    let travel_mm = travel_mm.clamp(1.0, HARD_MAX_MACHINE_TRAVEL_MM);
    MACHINE_MAX_SPEED_MM_S.store(max_speed_mm_s.to_bits(), Ordering::Release);
    MACHINE_TRAVEL_MM.store(travel_mm.to_bits(), Ordering::Release);
}

pub fn machine_max_speed_mm_s() -> f64 {
    f64::from_bits(MACHINE_MAX_SPEED_MM_S.load(Ordering::Acquire))
}

pub fn machine_travel_mm() -> f64 {
    f64::from_bits(MACHINE_TRAVEL_MM.load(Ordering::Acquire))
}

pub fn set_estop_active(active: bool) {
    ESTOP_ACTIVE.store(active, Ordering::Release);
    if active {
        OWNER_SESSION_ACTIVE.store(false, Ordering::Release);
        // Do not preserve a nonzero requested speed across an E-stop reset.
        REQUESTED_VELOCITY.store(0.0f64.to_bits(), Ordering::Release);
    }
}

pub fn estop_active() -> bool {
    ESTOP_ACTIVE.load(Ordering::Acquire)
}

pub fn set_master_enabled(enabled: bool) {
    MASTER_ENABLED.store(enabled, Ordering::Release);
    if !enabled {
        OWNER_SESSION_ACTIVE.store(false, Ordering::Release);
    }
}

pub fn master_enabled() -> bool {
    MASTER_ENABLED.load(Ordering::Acquire)
}

pub fn activate_owner_session() -> bool {
    if !master_enabled() {
        return false;
    }
    OWNER_SESSION_ACTIVE.store(true, Ordering::Release);
    true
}

pub fn deactivate_owner_session() {
    OWNER_SESSION_ACTIVE.store(false, Ordering::Release);
}

pub fn owner_session_active() -> bool {
    OWNER_SESSION_ACTIVE.load(Ordering::Acquire)
}

pub fn set_owner_limits(
    max_speed: f64,
    min_stroke: f64,
    max_stroke: f64,
    min_depth: f64,
    max_depth: f64,
) {
    let machine_speed = machine_max_speed_mm_s();
    let travel = machine_travel_mm();

    let max_speed = max_speed.clamp(0.0, machine_speed);
    let min_stroke = min_stroke.clamp(0.0, travel);
    let max_stroke = max_stroke.clamp(min_stroke, travel);
    let min_depth = min_depth.clamp(0.0, travel);
    let max_depth = max_depth.clamp(min_depth, travel);

    OWNER_MAX_SPEED.store(max_speed.to_bits(), Ordering::Release);
    OWNER_MIN_STROKE.store(min_stroke.to_bits(), Ordering::Release);
    OWNER_MAX_STROKE.store(max_stroke.to_bits(), Ordering::Release);
    OWNER_MIN_DEPTH.store(min_depth.to_bits(), Ordering::Release);
    OWNER_MAX_DEPTH.store(max_depth.to_bits(), Ordering::Release);
}

pub fn owner_limits() -> ActiveLimits {
    ActiveLimits {
        max_speed: f64::from_bits(OWNER_MAX_SPEED.load(Ordering::Acquire)),
        min_stroke: f64::from_bits(OWNER_MIN_STROKE.load(Ordering::Acquire)),
        max_stroke: f64::from_bits(OWNER_MAX_STROKE.load(Ordering::Acquire)),
        min_depth: f64::from_bits(OWNER_MIN_DEPTH.load(Ordering::Acquire)),
        max_depth: f64::from_bits(OWNER_MAX_DEPTH.load(Ordering::Acquire)),
    }
}

pub fn fallback_limits() -> ActiveLimits {
    ActiveLimits {
        max_speed: FALLBACK_MAX_SPEED_MM_S.min(machine_max_speed_mm_s()),
        min_stroke: FALLBACK_MIN_STROKE_MM.min(machine_travel_mm()),
        max_stroke: FALLBACK_MAX_STROKE_MM.min(machine_travel_mm()),
        min_depth: FALLBACK_MIN_DEPTH_MM.min(machine_travel_mm()),
        max_depth: FALLBACK_MAX_DEPTH_MM.min(machine_travel_mm()),
    }
}

pub fn active_limits() -> Option<ActiveLimits> {
    if !master_enabled() {
        return None;
    }
    // Persisted owner limits remain active after boot even with no owner-page
    // heartbeat. The owner session flag is now UI/session state only; it no
    // longer selects a 10 mm/s fallback envelope.
    Some(owner_limits())
}

/// Clamp stroke/depth in millimetres while preserving stroke length whenever
/// possible. `depth` is the deep endpoint and `depth - stroke` is the shallow
/// endpoint.
pub fn clamp_geometry_mm(requested_stroke: f64, requested_depth: f64, limits: ActiveLimits) -> (f64, f64) {
    let allowed_width = (limits.max_depth - limits.min_depth).max(0.0);
    let mut stroke = requested_stroke.clamp(limits.min_stroke, limits.max_stroke);
    if stroke > allowed_width {
        stroke = allowed_width;
    }

    let mut deep = requested_depth.clamp(limits.min_depth, limits.max_depth);
    if deep - stroke < limits.min_depth {
        deep = limits.min_depth + stroke;
    }
    if deep > limits.max_depth {
        deep = limits.max_depth;
    }
    if deep - stroke < limits.min_depth {
        stroke = (deep - limits.min_depth).max(0.0);
    }

    (stroke, deep)
}

/// Clamp a complete pattern input. When the master is off this only applies
/// the normal normalized 0..1 bounds. When enabled, owner/fallback limits are
/// converted from physical units to pattern-engine fractions.
pub fn clamp_input(mut input: PatternInput) -> PatternInput {
    input.velocity = input.velocity.clamp(0.0, 1.0);
    input.stroke = input.stroke.clamp(0.0, 1.0);
    input.depth = input.depth.clamp(0.0, 1.0);
    input.sensation = input.sensation.clamp(-1.0, 1.0);

    if estop_active() {
        input.velocity = 0.0;
        return input;
    }

    let Some(limits) = active_limits() else {
        return input;
    };

    let max_speed = machine_max_speed_mm_s();
    let travel = machine_travel_mm();

    let requested_speed = input.velocity * max_speed;
    input.velocity = requested_speed.min(limits.max_speed) / max_speed;

    let requested_stroke = input.stroke * travel;
    let requested_depth = input.depth * travel;
    let (stroke, depth) = clamp_geometry_mm(requested_stroke, requested_depth, limits);
    input.stroke = stroke / travel;
    input.depth = depth / travel;

    input
}


pub fn clamp_stream_position_fraction(position: f64) -> f64 {
    // XToys Position mode uses the same effective stroke/depth geometry as
    // normal XToys pattern (Speed) mode. This keeps 0..100% Position within
    // the exact same physical envelope:
    //   0%   = shallow endpoint (depth - stroke)
    //   100% = deep endpoint (depth)
    //
    // Derive the effective input from the remote-requested values through the
    // normal owner-limit clamp so owner stroke/depth limits remain authoritative.
    let position = position.clamp(0.0, 1.0);
    let input = clamp_input(requested_input());
    let stroke = input.stroke.clamp(0.0, input.depth);
    let shallow = input.depth - stroke;
    (shallow + position * stroke).clamp(0.0, 1.0)
}


pub fn stream_speed_fraction(current: f64, target: f64, time_ms: u32) -> f64 {
    let machine_speed = machine_max_speed_mm_s().max(0.001);
    let travel = machine_travel_mm().max(0.001);

    // XToys-generated "hold" patterns use an intentionally enormous segment
    // time (observed: 500_000_000 ms). That is not intended to produce a
    // multi-day crawl toward the target. Treat it as "settle at this absolute
    // position, then remain there until the next XToys command".
    //
    // Keep ordinary long/timed moves intact. 100_000_000 ms (~27.8 h) is far
    // outside normal scripted motion but safely below the XToys sentinel.
    const XTOYS_HOLD_SENTINEL_MS: u32 = 100_000_000;
    let is_hold = time_ms >= XTOYS_HOLD_SENTINEL_MS;

    let requested_mm_s = if time_ms == 0 || is_hold {
        machine_speed
    } else {
        let distance_mm = (target - current).abs() * travel;
        distance_mm / (time_ms as f64 / 1000.0).max(0.001)
    };
    let ceiling = active_limits()
        .map(|limits| limits.max_speed)
        .unwrap_or(machine_speed)
        .clamp(0.0, machine_speed);
    requested_mm_s.min(ceiling) / machine_speed
}


/// Clamp a firmware-owned routine's shallow/deep endpoints against the
/// currently active owner/fallback geometry envelope. Returns normalized
/// machine positions `(shallow, deep)`.
pub fn clamp_routine_pair_fraction(a: f64, b: f64) -> (f64, f64) {
    let travel = machine_travel_mm().max(0.001);
    let mut shallow = a.min(b).clamp(0.0, 1.0);
    let mut deep = a.max(b).clamp(0.0, 1.0);

    if let Some(limits) = active_limits() {
        let requested_stroke = (deep - shallow) * travel;
        let requested_depth = deep * travel;
        let (stroke_mm, depth_mm) = clamp_geometry_mm(requested_stroke, requested_depth, limits);
        deep = (depth_mm / travel).clamp(0.0, 1.0);
        shallow = ((depth_mm - stroke_mm) / travel).clamp(0.0, 1.0);
    }
    (shallow, deep)
}

/// Apply stock + owner/fallback speed ceilings to a requested normalized
/// routine speed.
pub fn routine_speed_fraction(requested: f64) -> f64 {
    let machine_speed = machine_max_speed_mm_s().max(0.001);
    let requested_mm_s = requested.clamp(0.0, 1.0) * machine_speed;
    let ceiling = active_limits()
        .map(|limits| limits.max_speed)
        .unwrap_or(machine_speed)
        .clamp(0.0, machine_speed);
    requested_mm_s.min(ceiling) / machine_speed
}
