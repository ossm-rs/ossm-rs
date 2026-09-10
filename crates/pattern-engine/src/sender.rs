use core::sync::atomic::{AtomicU16, AtomicU8, Ordering};

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::pubsub::{self, Subscriber};

use crate::commands::PatternCommand;
use crate::engine::{EngineCommand, EngineState, PatternEngine, RoutineConfig, RoutineField, StreamMove};
use crate::input::PatternInput;
use crate::owner_limits;

/// Sender half of the pattern engine.
///
/// Holds the capability to issue engine commands (`play`, `pause`,
/// `resume`, `stop`, `home`) and to mutate the live pattern input
/// (`set_speed`, `set_depth`, `set_stroke`, `set_sensation`), plus the
/// read methods from [`PatternObserver`](crate::PatternObserver) -
/// command issuers almost always also need to read state to plan,
/// and reading is harmless. Code that only needs to observe should
/// still take `&PatternObserver` for the tighter contract.
///
/// Produced by [`PatternEngine::split`](crate::PatternEngine::split).
/// Not [`Clone`] and not publicly constructible. The intended pattern
/// is to hand `&PatternSender` (or own a static one) to every
/// subsystem that needs to issue commands - typically the events-bus
/// adapter that bridges `PatternCommand` into the engine.

// XToys firmware-routine settings capture.
//
// Transport v4 keeps v3's order-independent tagged packets and adds VDT +
// randomization settings. Each field is identified by a unique short duration.
static XTOYS_ROUTINE_CAPTURE_PHASE: AtomicU8 = AtomicU8::new(0);
static XTOYS_ROUTINE_FIELD_MASK: AtomicU16 = AtomicU16::new(0);
const XTOYS_ROUTINE_CAPTURE_ARMED: u8 = 1;
const XTOYS_ROUTINE_CAPTURE_LOCKED: u8 = 255;

const XTOYS_ROUTINE_ARM_MS: u32 = 29;
const XTOYS_ROUTINE_HEAD_MS: u32 = 31;
const XTOYS_ROUTINE_SUCK_MS: u32 = 32;
const XTOYS_ROUTINE_DT_MS: u32 = 33;
const XTOYS_ROUTINE_VDT_MS: u32 = 34;
const XTOYS_ROUTINE_SPEED_MS: u32 = 35;
const XTOYS_ROUTINE_COUNT_MIN_MS: u32 = 36;
const XTOYS_ROUTINE_COUNT_MAX_MS: u32 = 37;
const XTOYS_ROUTINE_DT_CHANCE_MS: u32 = 38;
const XTOYS_ROUTINE_VDT_CHANCE_MS: u32 = 39;
const XTOYS_ROUTINE_DT_EVERY_MS: u32 = 40;
const XTOYS_ROUTINE_HOLD_MIN_MS: u32 = 42;
const XTOYS_ROUTINE_HOLD_MAX_MS: u32 = 43;
const XTOYS_ROUTINE_START_MS: u32 = 44;
const XTOYS_ROUTINE_GATE_DOWN_MS: u32 = 45;
const XTOYS_ROUTINE_GATE_UP_MS: u32 = 46;
const XTOYS_ROUTINE_GATE_WARMUP_MS: u32 = 47;
const XTOYS_ROUTINE_PARK_MS: u32 = 3_600_000; // 1 hour XToys transport dwell; never motion
const XTOYS_ROUTINE_RESET_MS: u32 = 49;

const XTOYS_ROUTINE_ARM_POSITION: f64 = 0.9999; // 99.99%
const XTOYS_ROUTINE_POS_EPS: f64 = 0.0002;
const XTOYS_ROUTINE_ALL_FIELDS: u16 = 0x0fff;

pub struct PatternSender {
    engine: &'static PatternEngine,
}

impl PatternSender {
    pub(crate) fn new(engine: &'static PatternEngine) -> Self {
        Self { engine }
    }

    pub fn play(&self, index: usize) {
        if owner_limits::estop_active() {
            return;
        }
        let _ = self.engine.commands.try_send(EngineCommand::Play(index));
    }

    pub fn pause(&self) {
        let _ = self.engine.commands.try_send(EngineCommand::Pause);
    }

    pub fn resume(&self) {
        if owner_limits::estop_active() {
            return;
        }
        let _ = self.engine.commands.try_send(EngineCommand::Resume);
    }

    pub fn stop(&self) {
        let _ = self.engine.commands.try_send(EngineCommand::Stop);
    }

    pub fn home(&self) {
        if owner_limits::estop_active() {
            return;
        }
        let _ = self.engine.commands.try_send(EngineCommand::Home);
    }

    /// XToys Custom Firmware `stop`: stop XToys motion without
    /// treating it as a full device/session disable.
    pub fn xtoys_stop(&self) {
        let _ = self.engine.commands.try_send(EngineCommand::XToysStop);
    }

    /// Configure the firmware-owned XToys routine. Positions and speed are
    /// normalized 0.0..=1.0. `dt_every == 0` disables forced DT insertion.
    pub fn routine_configure(
        &self,
        head: f64,
        suck: f64,
        dt: f64,
        speed: f64,
        count: u32,
        dt_every: u32,
        dt_hold_ms: u32,
    ) {
        // Legacy JSON setConfig compatibility: fixed count/hold, no random DT/VDT.
        let cfg = RoutineConfig {
            head: head.clamp(0.0, 1.0),
            suck: suck.clamp(0.0, 1.0),
            dt: dt.clamp(0.0, 1.0),
            vdt: 0.90,
            speed: speed.clamp(0.0, 1.0),
            count_min: count.clamp(1, 10_000),
            count_max: count.clamp(1, 10_000),
            dt_chance: 0,
            vdt_chance: 0,
            dt_every: dt_every.min(10_000),
            hold_min_ms: dt_hold_ms.min(60_000),
            hold_max_ms: dt_hold_ms.min(60_000),
        };
        let _ = self.engine.commands.try_send(EngineCommand::RoutineConfigure(cfg));
    }

    pub fn routine_start(&self) {
        if owner_limits::estop_active() {
            return;
        }
        let _ = self.engine.commands.try_send(EngineCommand::RoutineStart);
    }

    pub fn routine_stop(&self) {
        let _ = self.engine.commands.try_send(EngineCommand::RoutineStop);
    }

    pub fn start_streaming(&self) {
        if owner_limits::estop_active() { return; }
        let _ = self.engine.commands.try_send(EngineCommand::StartStreaming);
    }


    pub fn stream_move(&self, position: f64, time_ms: u32, replace: bool) {
        if owner_limits::estop_active() { return; }
        let position = position.clamp(0.0, 1.0);

        // Reliable XToys settings transport for the firmware-owned routine.
        //
        // v3 uses duration-tagged packets, so fields may arrive in any order.
        // XToys is free to stop/restart/resume the underlying position pattern.
        let phase = XTOYS_ROUTINE_CAPTURE_PHASE.load(Ordering::Acquire);

        // Pawprint gate/warm-up tags deliberately use short move durations (45/46/47 ms).
        // Those durations are also perfectly valid ordinary XToys Position-mode moves, so
        // they must NEVER be interpreted globally. Only recognize them after the dedicated
        // firmware-owned routine transport has completed its tagged setup and is LOCKED.
        // This prevents normal Position motion from falsely generating Pawprint UP/DOWN and
        // dropping the runner back to Ready.
        if phase == XTOYS_ROUTINE_CAPTURE_LOCKED {
            if time_ms == XTOYS_ROUTINE_GATE_WARMUP_MS {
                return;
            }
            if time_ms == XTOYS_ROUTINE_GATE_DOWN_MS {
                let _ = self.engine.commands.try_send(EngineCommand::XToysStop);
                let _ = self.engine.commands.try_send(EngineCommand::RoutineGate(true));
                log::info!("XToys routine Pawprint DOWN");
                return;
            }
            if time_ms == XTOYS_ROUTINE_GATE_UP_MS {
                let _ = self.engine.commands.try_send(EngineCommand::XToysStop);
                let _ = self.engine.commands.try_send(EngineCommand::RoutineGate(false));
                log::info!("XToys routine Pawprint UP");
                return;
            }
        }

        // XToys transport parking packet. The settings pattern dwells here for one hour
        // after START so it does not continuously replay the tagged configuration.
        // This value is outside ordinary move timing, but keep it transport-only.
        if time_ms == XTOYS_ROUTINE_PARK_MS {
            return;
        }

        if phase == XTOYS_ROUTINE_CAPTURE_LOCKED && time_ms == XTOYS_ROUTINE_RESET_MS {
            XTOYS_ROUTINE_CAPTURE_PHASE.store(0, Ordering::Release);
            XTOYS_ROUTINE_FIELD_MASK.store(0, Ordering::Release);
            let _ = self.engine.commands.try_send(EngineCommand::RoutineStop);
            log::info!("XToys routine transport reset / stop requested");
            return;
        }

        if time_ms == XTOYS_ROUTINE_ARM_MS
            && phase == 0
            && (position - XTOYS_ROUTINE_ARM_POSITION).abs() <= XTOYS_ROUTINE_POS_EPS
        {
            XTOYS_ROUTINE_FIELD_MASK.store(0, Ordering::Release);
            XTOYS_ROUTINE_CAPTURE_PHASE.store(XTOYS_ROUTINE_CAPTURE_ARMED, Ordering::Release);
            let _ = self.engine.commands.try_send(EngineCommand::XToysStop);
            log::info!("XToys routine settings capture armed (tagged v3)");
            return;
        }

        if phase == XTOYS_ROUTINE_CAPTURE_ARMED {
            let tagged = match time_ms {
                XTOYS_ROUTINE_HEAD_MS => Some((RoutineField::Head, 1u16 << 0)),
                XTOYS_ROUTINE_SUCK_MS => Some((RoutineField::Suck, 1u16 << 1)),
                XTOYS_ROUTINE_DT_MS => Some((RoutineField::Dt, 1u16 << 2)),
                XTOYS_ROUTINE_VDT_MS => Some((RoutineField::Vdt, 1u16 << 3)),
                XTOYS_ROUTINE_SPEED_MS => Some((RoutineField::Speed, 1u16 << 4)),
                XTOYS_ROUTINE_COUNT_MIN_MS => Some((RoutineField::CountMin, 1u16 << 5)),
                XTOYS_ROUTINE_COUNT_MAX_MS => Some((RoutineField::CountMax, 1u16 << 6)),
                XTOYS_ROUTINE_DT_CHANCE_MS => Some((RoutineField::DtChance, 1u16 << 7)),
                XTOYS_ROUTINE_VDT_CHANCE_MS => Some((RoutineField::VdtChance, 1u16 << 8)),
                XTOYS_ROUTINE_DT_EVERY_MS => Some((RoutineField::DtEvery, 1u16 << 9)),
                XTOYS_ROUTINE_HOLD_MIN_MS => Some((RoutineField::HoldMin, 1u16 << 10)),
                XTOYS_ROUTINE_HOLD_MAX_MS => Some((RoutineField::HoldMax, 1u16 << 11)),
                _ => None,
            };

            if let Some((field, bit)) = tagged {
                if self.engine.commands.try_send(EngineCommand::RoutineSetField(field, position)).is_err() {
                    log::warn!("XToys routine settings queue full; dropping {:?}", field);
                    return;
                }
                let old_mask = XTOYS_ROUTINE_FIELD_MASK.fetch_or(bit, Ordering::AcqRel);
                let new_mask = old_mask | bit;
                log::info!("XToys routine tagged field {:?} received; mask=0x{:03x}", field, new_mask);
                return;
            }

            if time_ms == XTOYS_ROUTINE_START_MS {
                let mask = XTOYS_ROUTINE_FIELD_MASK.load(Ordering::Acquire);
                if mask == XTOYS_ROUTINE_ALL_FIELDS {
                    XTOYS_ROUTINE_CAPTURE_PHASE.store(XTOYS_ROUTINE_CAPTURE_LOCKED, Ordering::Release);
                    let _ = self.engine.commands.try_send(EngineCommand::XToysStop);
                    let _ = self.engine.commands.try_send(EngineCommand::RoutineStart);
                    log::info!("XToys routine tagged settings complete; start requested");
                } else {
                    log::warn!(
                        "XToys routine START seen before all settings; mask=0x{:03x}; waiting for repeat",
                        mask
                    );
                }
                return;
            }

            if time_ms == XTOYS_ROUTINE_ARM_MS {
                return;
            }
        }

        if phase == XTOYS_ROUTINE_CAPTURE_LOCKED
            && matches!(
                time_ms,
                XTOYS_ROUTINE_ARM_MS
                    | XTOYS_ROUTINE_HEAD_MS
                    | XTOYS_ROUTINE_SUCK_MS
                    | XTOYS_ROUTINE_DT_MS
                    | XTOYS_ROUTINE_VDT_MS
                    | XTOYS_ROUTINE_SPEED_MS
                    | XTOYS_ROUTINE_COUNT_MIN_MS
                    | XTOYS_ROUTINE_COUNT_MAX_MS
                    | XTOYS_ROUTINE_DT_CHANCE_MS
                    | XTOYS_ROUTINE_VDT_CHANCE_MS
                    | XTOYS_ROUTINE_DT_EVERY_MS
                    | XTOYS_ROUTINE_HOLD_MIN_MS
                    | XTOYS_ROUTINE_HOLD_MAX_MS
                    | XTOYS_ROUTINE_START_MS
            )
        {
            return;
        }

        let cmd = EngineCommand::StreamMove(StreamMove {
            position,
            time_ms,
            replace,
        });
        if self.engine.commands.try_send(cmd).is_err() {
            log::warn!("XToys streaming queue full; dropping position command");
        }
    }


    pub fn set_speed(&self, value: f64) {
        if owner_limits::estop_active() {
            return;
        }
        owner_limits::set_requested_speed(value);
        let requested = owner_limits::requested_input();
        self.engine.input.sender().send_modify(|opt| {
            if let Some(input) = opt {
                *input = owner_limits::clamp_input(requested);
            }
        });
    }

    /// Set stroke length as a fraction of full machine travel.
    pub fn set_stroke(&self, value: f64) {
        owner_limits::set_requested_stroke(value);
        let requested = owner_limits::requested_input();
        self.engine.input.sender().send_modify(|opt| {
            if let Some(input) = opt {
                *input = owner_limits::clamp_input(requested);
            }
        });
    }

    /// Set depth as a fraction of machine range. Clamped to `0.0..=1.0`.
    pub fn set_depth(&self, value: f64) {
        owner_limits::set_requested_depth(value);
        let requested = owner_limits::requested_input();
        self.engine.input.sender().send_modify(|opt| {
            if let Some(input) = opt {
                *input = owner_limits::clamp_input(requested);
            }
        });
    }


    /// Immediately re-apply the current owner/fallback envelope to the live input.
    /// Used when the master/session state or configured limits change.
    pub fn reapply_owner_limits(&self) {
        // Always derive the effective input from the REMOTE-REQUESTED values,
        // never from the already-clamped live input.
        let requested = owner_limits::requested_input();
        self.engine.input.sender().send_modify(|opt| {
            if let Some(input) = opt {
                *input = owner_limits::clamp_input(requested);
            }
        });
    }

    /// Set sensation (pattern-specific). Clamped to `-1.0..=1.0`.
    pub fn set_sensation(&self, value: f64) {
        owner_limits::set_requested_sensation(value);
        let requested = owner_limits::requested_input();
        self.engine.input.sender().send_modify(|opt| {
            if let Some(input) = opt {
                input.sensation = requested.sensation;
            }
        });
    }

    /// Apply a single [`PatternCommand`] - the bridge from the events bus.
    ///
    /// A long-running task subscribes to `PatternCommand` events and calls
    /// `apply` for each one.
    pub fn apply(&self, cmd: PatternCommand) {
        match cmd {
            PatternCommand::Play(idx) => self.play(idx),
            PatternCommand::Pause => self.pause(),
            PatternCommand::Resume => self.resume(),
            PatternCommand::Stop => self.stop(),
            PatternCommand::Home => self.home(),
            PatternCommand::SetSpeed(v) => self.set_speed(v),
            PatternCommand::SetStroke(v) => self.set_stroke(v),
            PatternCommand::SetDepth(v) => self.set_depth(v),
            PatternCommand::SetSensation(v) => self.set_sensation(v),
        }
    }

    /// Current engine state (idle, homing, playing, paused, ready).
    ///
    /// `PatternSender` carries [`PatternObserver`](crate::PatternObserver)'s
    /// read capability so command-issuing code can plan against current
    /// state without also being handed an observer.
    pub fn state(&self) -> EngineState {
        EngineState::decode(self.engine.state.load(Ordering::Relaxed))
    }

    /// Current pattern input (depth, stroke, velocity, sensation).
    pub fn input(&self) -> PatternInput {
        self.engine.input.try_get().unwrap_or(PatternInput::DEFAULT)
    }

    /// Subscribe to [`EngineState`] transitions.
    ///
    /// Returns `Err` if all subscriber slots are in use.
    pub fn subscribe(
        &self,
    ) -> Result<Subscriber<'static, CriticalSectionRawMutex, EngineState, 1, 8, 0>, pubsub::Error>
    {
        self.engine.state_channel.subscriber()
    }
}
