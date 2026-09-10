use core::sync::atomic::Ordering;

use embassy_futures::select::{self, Either};
use embedded_hal_async::delay::DelayNs;
use log::info;
use embassy_time::Instant;
use heapless::Deque;
use ossm::{MotionCommand, MotionSender, StateResponse};

use crate::AnyPattern;
use crate::engine::{EngineCommand, EngineState, PatternEngine, RoutineConfig, RoutineField, StreamMove};
use crate::owner_limits;
use crate::pattern::{Pattern, PatternCtx};

/// Internal runner state.
#[derive(Debug, Clone, Copy)]
enum AfterHome {
    Ready,
    Play(usize),
}

#[derive(Debug, Clone, Copy)]
enum RunnerState {
    Idle,
    Homing(AfterHome),
    Ready,
    Playing(usize),
    Streaming,
    StreamingPaused,
    Routine(RoutinePhase),
    RoutinePaused(RoutinePhase),
}

#[derive(Debug, Clone, Copy)]
enum RoutinePhase {
    PrimeHead,
    ToSuck,
    ToHead,
    ToDt,
    DtHold,
    FromDt,
    ToVdt,
    VdtHold,
    FromVdt,
}

#[derive(Debug, Clone, Copy)]
struct RoutineRuntime {
    remaining: u32,
    since_dt: u32,
    hold_ms: u32,
    rng: u32,
    armed: bool,
    gate_down: bool,
}

impl RoutineRuntime {
    const fn new() -> Self {
        Self { remaining: 0, since_dt: 0, hold_ms: 0, rng: 0x6d2b79f5, armed: false, gate_down: false }
    }

    fn next_random(&mut self) -> u32 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        if x == 0 { x = 0x6d2b79f5; }
        self.rng = x;
        x
    }

    fn range_inclusive(&mut self, a: u32, b: u32) -> u32 {
        let lo = a.min(b);
        let hi = a.max(b);
        if lo == hi { return lo; }
        lo + (self.next_random() % (hi - lo + 1))
    }
}

impl RunnerState {
    fn as_engine_state(self) -> EngineState {
        match self {
            Self::Idle => EngineState::Idle,
            Self::Homing(_) => EngineState::Homing,
            Self::Ready => EngineState::Ready,
            Self::Playing(idx) => EngineState::Playing(idx),
            Self::Streaming | Self::StreamingPaused | Self::Routine(_) | Self::RoutinePaused(_) => EngineState::Ready,
        }
    }
}

/// Driver capability for the pattern engine.
///
/// Produced by [`PatternEngine::split`](crate::PatternEngine::split).
/// Drives the engine's main loop via [`run`](Self::run). The loop
/// only returns if the host future is dropped (e.g. a mode switch),
/// at which point the runner is free for another `run` call - each
/// call starts fresh from the engine's current state.
///
/// The runner carries no state of its own; "currently running" is a
/// property of the in-flight future, not the type. Spawning two
/// concurrent `run`s on the same runner would compete for the engine's
/// command channel; by convention only one caller (the active mode, or
/// the firmware boot path) drives a runner at a time.
pub struct PatternRunner {
    engine: &'static PatternEngine,
}

impl PatternRunner {
    pub(crate) fn new(engine: &'static PatternEngine) -> Self {
        Self { engine }
    }

    /// Run the engine forever, processing commands and driving patterns.
    ///
    /// `motion` is borrowed for the lifetime of the run. `patterns` is
    /// moved in and lives on the runner's stack frame. `delay` must be
    /// `Clone` so a fresh [`PatternCtx`] can be created each time a
    /// pattern starts (all embassy `Delay` types are `Copy`).
    pub async fn run<const N: usize, D: DelayNs + Clone>(
        &self,
        motion: &MotionSender,
        mut patterns: [AnyPattern; N],
        delay: D,
    ) -> ! {
        let engine = self.engine;
        let input = &engine.input;
        let mut state = RunnerState::Idle;
        let mut stream_queue: Deque<StreamMove, 24> = Deque::new();
        let mut routine_cfg = RoutineConfig::DEFAULT;
        let mut routine_rt = RoutineRuntime::new();

        loop {
            match state {
                RunnerState::Idle | RunnerState::Ready => {
                    let cmd = engine.commands.receive().await;
                    handle_command::<N>(engine, motion, cmd, &mut state, &mut stream_queue, &mut routine_cfg, &mut routine_rt).await;
                }
                RunnerState::Homing(maybe_idx) => {
                    if motion.enable().await != StateResponse::Completed {
                        log::error!("Enable failed, returning to idle");
                        set_state(engine, &mut state, RunnerState::Idle);
                        continue;
                    }

                    let home_fut = motion.home();
                    let mut home_fut = core::pin::pin!(home_fut);

                    loop {
                        let result =
                            select::select(home_fut.as_mut(), engine.commands.receive()).await;

                        match result {
                            Either::First(resp) => {
                                if resp != StateResponse::Completed {
                                    log::error!("Home failed, returning to idle");
                                    set_state(engine, &mut state, RunnerState::Idle);
                                } else {
                                    match maybe_idx {
                                        AfterHome::Play(idx) => set_state(engine, &mut state, RunnerState::Playing(idx)),
                                        AfterHome::Ready => set_state(engine, &mut state, RunnerState::Ready),
                                    }
                                }
                                break;
                            }
                            Either::Second(EngineCommand::Stop | EngineCommand::Pause) => {
                                routine_rt.armed = false;
                                routine_rt.gate_down = false;
                                if motion.disable().await == StateResponse::Fault {
                                    log::error!("Board fault during disable");
                                }
                                set_state(engine, &mut state, RunnerState::Idle);
                                break;
                            }
                            Either::Second(_) => {}
                        }
                    }
                }
                RunnerState::Playing(idx) => {
                    let mut ctx = PatternCtx::new(motion, input, delay.clone());
                    let pattern_fut = core::pin::pin!(patterns[idx].run(&mut ctx));
                    let mut pattern_fut = pattern_fut;

                    loop {
                        let result =
                            select::select(pattern_fut.as_mut(), engine.commands.receive()).await;

                        match result {
                            Either::First(_result) => {
                                if matches!(state, RunnerState::Playing(_)) {
                                    state = RunnerState::Idle;
                                    store_and_publish(engine, EngineState::Idle);
                                }
                                break;
                            }
                            Either::Second(cmd) => match cmd {
                                EngineCommand::Pause => {
                                    if motion.pause().await != StateResponse::Completed {
                                        log::error!("Pause failed, stopping engine");
                                        state = RunnerState::Idle;
                                        store_and_publish(engine, EngineState::Idle);
                                        break;
                                    }
                                    store_and_publish(engine, EngineState::Paused(idx));
                                }
                                EngineCommand::Resume => {
                                    if motion.resume().await != StateResponse::Completed {
                                        log::error!("Resume failed, stopping engine");
                                        state = RunnerState::Idle;
                                        store_and_publish(engine, EngineState::Idle);
                                        break;
                                    }
                                    store_and_publish(engine, EngineState::Playing(idx));
                                }
                                EngineCommand::Play(i) if i == idx => {}
                                EngineCommand::Play(new_idx) if new_idx < N => {
                                    state = RunnerState::Playing(new_idx);
                                    store_and_publish(engine, EngineState::Playing(new_idx));
                                    break;
                                }
                                EngineCommand::XToysStop => {
                                    // XToys uses stop as a mode hand-off before startStreaming.
                                    // Do NOT disable the motor here: doing so leaves the low-level
                                    // controller Disabled while the pattern engine claims Ready, so
                                    // all subsequent Position and Speed commands are accepted by BLE
                                    // but cannot move the actuator. Hold the current commanded
                                    // position while keeping the motor enabled. The next normal
                                    // XToys command is accepted immediately when leaving this
                                    // temporary direct-stream hold.
                                    let current = motion.state().position as f64;
                                    motion.update_motion(MotionCommand {
                                        position: current,
                                        speed: 0.0,
                                        jerk: 0.5,
                                        torque: None,
                                        direct_stream: true,
                                    });
                                    state = RunnerState::Ready;
                                    store_and_publish(engine, EngineState::Ready);
                                    info!("XToys pattern stopped; motor held enabled, homing preserved");
                                    break;
                                }
                                EngineCommand::Stop => {
                                    if motion.disable().await == StateResponse::Fault {
                                        log::error!("Board fault during disable");
                                    }
                                    state = RunnerState::Idle;
                                    store_and_publish(engine, EngineState::Idle);
                                    break;
                                }
                                _ => {}
                            },
                        }
                    }
                }

                RunnerState::RoutinePaused(phase) => {
                    match engine.commands.receive().await {
                        EngineCommand::RoutineGate(true) => {
                            routine_rt.gate_down = true;
                            set_state(engine, &mut state, RunnerState::Routine(phase));
                            info!("XToys routine Pawprint pressed; resuming {:?}", phase);
                        }
                        EngineCommand::RoutineGate(false) => {
                            routine_rt.gate_down = false;
                        }
                        EngineCommand::RoutineConfigure(cfg) => {
                            routine_cfg = cfg;
                            info!("XToys routine config updated while paused");
                        }
                        EngineCommand::RoutineSetField(field, value) => {
                            apply_routine_field(&mut routine_cfg, field, value);
                        }
                        EngineCommand::RoutineStop => {
                            routine_rt.armed = false;
                            routine_rt.gate_down = false;
                            routine_rt.remaining = 0;
                            set_state(engine, &mut state, RunnerState::Ready);
                            info!("XToys routine disarmed/stopped; homing preserved");
                        }
                        EngineCommand::XToysStop => {
                            // XToys emits stop while switching patterns. Ignore it for the
                            // firmware-owned routine; Pawprint UP is the motion gate.
                        }
                        EngineCommand::Stop | EngineCommand::Pause => {
                            routine_rt.armed = false;
                            routine_rt.gate_down = false;
                            if motion.disable().await == StateResponse::Fault {
                                log::error!("Board fault during routine paused disable");
                            }
                            set_state(engine, &mut state, RunnerState::Idle);
                        }
                        other => {
                            log::warn!("Ignoring {:?} while XToys routine is waiting for Pawprint", other);
                        }
                    }
                    continue;
                }

                RunnerState::Routine(phase) => {
                    if owner_limits::estop_active() {
                        if motion.disable().await == StateResponse::Fault {
                            log::error!("Board fault during routine E-stop disable");
                        }
                        set_state(engine, &mut state, RunnerState::Idle);
                        continue;
                    }

                    if matches!(phase, RoutinePhase::DtHold | RoutinePhase::VdtHold) {
                        let hold_name = if matches!(phase, RoutinePhase::DtHold) { "DT" } else { "VDT" };
                        info!("XToys routine {} hold {} ms", hold_name, routine_rt.hold_ms);
                        let mut hold_delay = delay.clone();
                        let hold_fut = hold_delay.delay_ms(routine_rt.hold_ms);
                        let mut hold_fut = core::pin::pin!(hold_fut);
                        loop {
                            match select::select(hold_fut.as_mut(), engine.commands.receive()).await {
                                Either::First(_) => {
                                    let next = if matches!(phase, RoutinePhase::DtHold) {
                                        RoutinePhase::FromDt
                                    } else {
                                        RoutinePhase::FromVdt
                                    };
                                    set_state(engine, &mut state, RunnerState::Routine(next));
                                    break;
                                }
                                Either::Second(EngineCommand::RoutineConfigure(cfg)) => {
                                    routine_cfg = cfg;
                                    info!("XToys routine config updated during hold");
                                }
                                Either::Second(EngineCommand::RoutineSetField(field, value)) => {
                                    apply_routine_field(&mut routine_cfg, field, value);
                                }
                                Either::Second(EngineCommand::RoutineGate(false)) => {
                                    routine_rt.gate_down = false;
                                    let (head, _) = owner_limits::clamp_routine_pair_fraction(
                                        routine_cfg.head,
                                        if matches!(phase, RoutinePhase::DtHold) { routine_cfg.dt } else { routine_cfg.vdt },
                                    );
                                    let retract_speed = owner_limits::routine_speed_fraction(routine_cfg.speed);
                                    info!("XToys routine Pawprint released during {} hold; retracting to Head {:.1}%", hold_name, head * 100.0);
                                    motion.begin_motion(MotionCommand {
                                        position: head,
                                        speed: retract_speed,
                                        jerk: 0.5,
                                        torque: None,
                                        direct_stream: false,
                                    });
                                    if motion.await_motion().await.is_err() {
                                        log::warn!("XToys routine retract-to-Head cancelled during {} hold release", hold_name);
                                    }
                                    let resume_phase = if matches!(phase, RoutinePhase::DtHold) {
                                        RoutinePhase::FromDt
                                    } else {
                                        RoutinePhase::FromVdt
                                    };
                                    set_state(engine, &mut state, RunnerState::RoutinePaused(resume_phase));
                                    info!("XToys routine retracted to Head; paused waiting for Pawprint");
                                    break;
                                }
                                Either::Second(EngineCommand::RoutineGate(true)) => {
                                    routine_rt.gate_down = true;
                                }
                                Either::Second(EngineCommand::RoutineStop) => {
                                    routine_rt.armed = false;
                                    routine_rt.gate_down = false;
                                    set_state(engine, &mut state, RunnerState::Ready);
                                    info!("XToys routine stopped; homing preserved");
                                    break;
                                }
                                Either::Second(EngineCommand::XToysStop) => {}
                                Either::Second(EngineCommand::Stop | EngineCommand::Pause) => {
                                    if motion.disable().await == StateResponse::Fault {
                                        log::error!("Board fault during routine disable");
                                    }
                                    set_state(engine, &mut state, RunnerState::Idle);
                                    break;
                                }
                                Either::Second(other) => {
                                    log::warn!("Ignoring {:?} while XToys routine hold is active", other);
                                }
                            }
                        }
                        continue;
                    }

                    let (normal_head, normal_suck) =
                        owner_limits::clamp_routine_pair_fraction(routine_cfg.head, routine_cfg.suck);
                    let (dt_head, dt_target) =
                        owner_limits::clamp_routine_pair_fraction(routine_cfg.head, routine_cfg.dt);
                    let (vdt_head, vdt_target) =
                        owner_limits::clamp_routine_pair_fraction(routine_cfg.head, routine_cfg.vdt);
                    let target = match phase {
                        RoutinePhase::PrimeHead => normal_head,
                        RoutinePhase::ToSuck => normal_suck,
                        RoutinePhase::ToHead => normal_head,
                        RoutinePhase::ToDt => dt_target,
                        RoutinePhase::FromDt => dt_head,
                        RoutinePhase::ToVdt => vdt_target,
                        RoutinePhase::FromVdt => vdt_head,
                        RoutinePhase::DtHold | RoutinePhase::VdtHold => unreachable!(),
                    };
                    let speed = owner_limits::routine_speed_fraction(routine_cfg.speed);

                    info!(
                        "XToys routine {:?}: target={:.1}% speed={:.3} remaining={} since_dt={}",
                        phase, target * 100.0, speed, routine_rt.remaining, routine_rt.since_dt
                    );
                    motion.begin_motion(MotionCommand { position: target, speed, jerk: 0.5, torque: None, direct_stream: false });
                    let mut move_fut = core::pin::pin!(motion.await_motion());
                    let mut interrupted = false;

                    loop {
                        match select::select(move_fut.as_mut(), engine.commands.receive()).await {
                            Either::First(result) => {
                                if result.is_err() {
                                    log::warn!("XToys routine move cancelled");
                                    set_state(engine, &mut state, RunnerState::Idle);
                                    interrupted = true;
                                }
                                break;
                            }
                            Either::Second(EngineCommand::RoutineConfigure(cfg)) => {
                                routine_cfg = cfg;
                                info!("XToys routine config updated; applies from next leg");
                            }
                            Either::Second(EngineCommand::RoutineSetField(field, value)) => {
                                apply_routine_field(&mut routine_cfg, field, value);
                            }
                            Either::Second(EngineCommand::RoutineGate(false)) => {
                                routine_rt.gate_down = false;
                                let current = motion.state().position as f64;
                                motion.update_motion(MotionCommand {
                                    position: current,
                                    speed: owner_limits::stream_speed_fraction(current, current, 0),
                                    jerk: 0.5,
                                    torque: None,
                                        direct_stream: false,
                                });
                                let _ = move_fut.as_mut().await;

                                let (head, _) = owner_limits::clamp_routine_pair_fraction(routine_cfg.head, routine_cfg.suck);
                                let retract_speed = owner_limits::routine_speed_fraction(routine_cfg.speed);
                                info!("XToys routine Pawprint released at {:.1}%; retracting to Head {:.1}%", current * 100.0, head * 100.0);
                                motion.begin_motion(MotionCommand {
                                    position: head,
                                    speed: retract_speed,
                                    jerk: 0.5,
                                    torque: None,
                                        direct_stream: false,
                                });
                                if motion.await_motion().await.is_err() {
                                    log::warn!("XToys routine retract-to-Head cancelled after Pawprint release");
                                }
                                set_state(engine, &mut state, RunnerState::RoutinePaused(phase));
                                info!("XToys routine retracted to Head; paused waiting for Pawprint");
                                interrupted = true;
                                break;
                            }
                            Either::Second(EngineCommand::RoutineGate(true)) => {
                                routine_rt.gate_down = true;
                            }
                            Either::Second(EngineCommand::RoutineStop) => {
                                routine_rt.armed = false;
                                routine_rt.gate_down = false;
                                let current = motion.state().position as f64;
                                motion.update_motion(MotionCommand {
                                    position: current,
                                    speed: owner_limits::stream_speed_fraction(current, current, 0),
                                    jerk: 0.5,
                                    torque: None,
                                        direct_stream: false,
                                });
                                let _ = move_fut.as_mut().await;
                                set_state(engine, &mut state, RunnerState::Ready);
                                info!("XToys routine stopped; homing preserved");
                                interrupted = true;
                                break;
                            }
                            Either::Second(EngineCommand::XToysStop) => {
                                // Ignore XToys scheduler stop while firmware routine owns motion.
                            }
                            Either::Second(EngineCommand::Stop | EngineCommand::Pause) => {
                                routine_rt.armed = false;
                                routine_rt.gate_down = false;
                                if motion.disable().await == StateResponse::Fault {
                                    log::error!("Board fault during routine disable");
                                }
                                set_state(engine, &mut state, RunnerState::Idle);
                                interrupted = true;
                                break;
                            }
                            Either::Second(other) => {
                                log::warn!("Ignoring {:?} while XToys routine motion is active", other);
                            }
                        }
                    }
                    if interrupted {
                        continue;
                    }

                    match phase {
                        RoutinePhase::PrimeHead => {
                            set_state(engine, &mut state, RunnerState::Routine(RoutinePhase::ToSuck));
                        }
                        RoutinePhase::ToSuck => {
                            set_state(engine, &mut state, RunnerState::Routine(RoutinePhase::ToHead));
                        }
                        RoutinePhase::ToHead => {
                            routine_rt.remaining = routine_rt.remaining.saturating_sub(1);
                            routine_rt.since_dt = routine_rt.since_dt.saturating_add(1);
                            info!("XToys routine suck complete; remaining={}", routine_rt.remaining);
                            if routine_rt.remaining == 0 {
                                set_state(engine, &mut state, RunnerState::Ready);
                                info!("XToys routine complete");
                            } else if routine_cfg.dt_every > 0 && routine_rt.since_dt >= routine_cfg.dt_every {
                                routine_rt.hold_ms = routine_rt.range_inclusive(routine_cfg.hold_min_ms, routine_cfg.hold_max_ms);
                                info!("XToys routine forced DT after {} sucks; hold={}ms", routine_rt.since_dt, routine_rt.hold_ms);
                                set_state(engine, &mut state, RunnerState::Routine(RoutinePhase::ToDt));
                            } else {
                                let total_chance = (routine_cfg.dt_chance + routine_cfg.vdt_chance).min(100);
                                let roll = routine_rt.next_random() % 100;
                                if roll < routine_cfg.vdt_chance.min(100) {
                                    routine_rt.hold_ms = routine_rt.range_inclusive(routine_cfg.hold_min_ms, routine_cfg.hold_max_ms);
                                    info!("XToys routine random VDT roll={} chance={}%; hold={}ms", roll, routine_cfg.vdt_chance, routine_rt.hold_ms);
                                    set_state(engine, &mut state, RunnerState::Routine(RoutinePhase::ToVdt));
                                } else if roll < total_chance {
                                    routine_rt.hold_ms = routine_rt.range_inclusive(routine_cfg.hold_min_ms, routine_cfg.hold_max_ms);
                                    info!("XToys routine random DT roll={} combined={}%; hold={}ms", roll, total_chance, routine_rt.hold_ms);
                                    set_state(engine, &mut state, RunnerState::Routine(RoutinePhase::ToDt));
                                } else {
                                    set_state(engine, &mut state, RunnerState::Routine(RoutinePhase::ToSuck));
                                }
                            }
                        }
                        RoutinePhase::ToDt => {
                            set_state(engine, &mut state, RunnerState::Routine(RoutinePhase::DtHold));
                        }
                        RoutinePhase::FromDt => {
                            routine_rt.since_dt = 0;
                            set_state(engine, &mut state, RunnerState::Routine(RoutinePhase::ToSuck));
                        }
                        RoutinePhase::ToVdt => {
                            set_state(engine, &mut state, RunnerState::Routine(RoutinePhase::VdtHold));
                        }
                        RoutinePhase::FromVdt => {
                            set_state(engine, &mut state, RunnerState::Routine(RoutinePhase::ToSuck));
                        }
                        RoutinePhase::DtHold | RoutinePhase::VdtHold => unreachable!(),
                    }
                }


                RunnerState::Streaming => {
                    // XToys timed position-stream scheduler.
                    if let Some(next) = stream_queue.pop_front() {
                        let target = owner_limits::clamp_stream_position_fraction(next.position);
                        let current = motion.state().position as f64;
                        let speed = owner_limits::stream_speed_fraction(current, target, next.time_ms);

                        if next.time_ms >= 100_000_000 {
                            info!(
                                "XToys HOLD target={:.1}% raw_time={}ms speed={:.3}",
                                target * 100.0,
                                next.time_ms,
                                speed
                            );
                        } else {
                            info!(
                                "XToys MOVE current={:.1}% target={:.1}% time={}ms speed={:.3}",
                                current * 100.0,
                                target * 100.0,
                                next.time_ms,
                                speed
                            );
                        }

                        motion.begin_motion(MotionCommand { position: target, speed, jerk: 0.5, torque: None, direct_stream: false });
                        let mut move_fut = core::pin::pin!(motion.await_motion());
                        let mut leave_streaming = false;
                        loop {
                            match select::select(move_fut.as_mut(), engine.commands.receive()).await {
                                Either::First(_) => break,
                                Either::Second(cmd) => match cmd {
                                    EngineCommand::StreamMove(item) => {
                                        if item.replace { stream_queue.clear(); }
                                        if stream_queue.push_back(item).is_err() { log::warn!("XToys streaming queue full"); }
                                    }
                                    EngineCommand::RoutineSetField(field, value) => {
                                        apply_routine_field(&mut routine_cfg, field, value);
                                    }
                                    EngineCommand::StartStreaming => {}
                                    EngineCommand::XToysStop => {
                                        // Hold current position immediately instead of waiting for
                                        // the previous streamed point-to-point move to complete.
                                        let current = motion.state().position as f64;
                                        motion.update_motion(MotionCommand {
                                            position: current,
                                            speed: 0.0,
                                            jerk: 0.5,
                                            torque: None,
                                            direct_stream: true,
                                        });
                                        stream_queue.clear();
                                        set_state(engine, &mut state, RunnerState::Ready);
                                        info!("XToys streaming stopped immediately; homing preserved");
                                        leave_streaming = true;
                                        break;
                                    }
                                    EngineCommand::Pause => {
                                        let current = motion.state().position as f64;
                                        motion.update_motion(MotionCommand {
                                            position: current,
                                            speed: 0.0,
                                            jerk: 0.5,
                                            torque: None,
                                            direct_stream: true,
                                        });
                                        stream_queue.clear();
                                        set_state(engine, &mut state, RunnerState::StreamingPaused);
                                        info!("XToys Position streaming paused; motor held enabled");
                                        leave_streaming = true;
                                        break;
                                    }
                                    EngineCommand::Stop => {
                                        if motion.disable().await == StateResponse::Fault { log::error!("Board fault during XToys streaming disable"); }
                                        set_state(engine, &mut state, RunnerState::Idle);
                                        stream_queue.clear();
                                        leave_streaming = true;
                                        break;
                                    }
                                    EngineCommand::Play(idx) if idx < N => {
                                        state = RunnerState::Playing(idx);
                                        store_and_publish(engine, EngineState::Playing(idx));
                                        stream_queue.clear();
                                        leave_streaming = true;
                                        break;
                                    }
                                    EngineCommand::Home => log::warn!("Ignoring Home while XToys streaming is active"),
                                    EngineCommand::Resume | EngineCommand::Play(_) | EngineCommand::RoutineStart |
                                    EngineCommand::RoutineStop | EngineCommand::RoutineConfigure(_) | EngineCommand::RoutineGate(_) => {}
                                },
                            }
                        }
                        if leave_streaming { continue; }
                    } else {
                        match engine.commands.receive().await {
                            EngineCommand::StreamMove(item) => {
                                if item.replace { stream_queue.clear(); }
                                if stream_queue.push_back(item).is_err() { log::warn!("XToys streaming queue full"); }
                            }
                            EngineCommand::RoutineSetField(field, value) => {
                                apply_routine_field(&mut routine_cfg, field, value);
                            }
                            EngineCommand::StartStreaming => {}
                            EngineCommand::XToysStop => {
                                stream_queue.clear();
                                set_state(engine, &mut state, RunnerState::Ready);
                                info!("XToys streaming stopped; homing preserved");
                            }
                            EngineCommand::Pause => {
                                let current = motion.state().position as f64;
                                motion.update_motion(MotionCommand { position: current, speed: 0.0, jerk: 0.5, torque: None, direct_stream: true });
                                stream_queue.clear();
                                set_state(engine, &mut state, RunnerState::StreamingPaused);
                                info!("XToys Position streaming paused; motor held enabled");
                            }
                            EngineCommand::Stop => {
                                if motion.disable().await == StateResponse::Fault { log::error!("Board fault during XToys streaming disable"); }
                                set_state(engine, &mut state, RunnerState::Idle);
                                stream_queue.clear();
                            }
                            EngineCommand::Play(idx) if idx < N => {
                                state = RunnerState::Playing(idx);
                                store_and_publish(engine, EngineState::Playing(idx));
                                stream_queue.clear();
                            }
                            EngineCommand::Home => log::warn!("Ignoring Home while XToys streaming is active"),
                            EngineCommand::Resume | EngineCommand::Play(_) | EngineCommand::RoutineStart |
                            EngineCommand::RoutineStop | EngineCommand::RoutineConfigure(_) | EngineCommand::RoutineGate(_) => {}
                        }
                    }
                }

                RunnerState::StreamingPaused => {
                    // XToys Position-mode pause. Keep the servo enabled and hold
                    // position. While paused retain only the newest streamed target
                    // so Resume cannot replay a stale backlog.
                    match engine.commands.receive().await {
                        EngineCommand::Resume | EngineCommand::StartStreaming => {
                            set_state(engine, &mut state, RunnerState::Streaming);
                            info!("XToys Position streaming resumed");
                        }
                        EngineCommand::StreamMove(item) => {
                            stream_queue.clear();
                            let _ = stream_queue.push_back(item);
                        }
                        EngineCommand::XToysStop => {
                            stream_queue.clear();
                            set_state(engine, &mut state, RunnerState::Ready);
                            info!("XToys paused Position streaming stopped; homing preserved");
                        }
                        EngineCommand::Stop => {
                            if motion.disable().await == StateResponse::Fault { log::error!("Board fault during XToys paused streaming disable"); }
                            stream_queue.clear();
                            set_state(engine, &mut state, RunnerState::Idle);
                        }
                        EngineCommand::Pause => {}
                        EngineCommand::Play(idx) if idx < N => {
                            stream_queue.clear();
                            set_state(engine, &mut state, RunnerState::Playing(idx));
                        }
                        EngineCommand::RoutineSetField(field, value) => apply_routine_field(&mut routine_cfg, field, value),
                        EngineCommand::Home => log::warn!("Ignoring Home while XToys Position streaming is paused"),
                        EngineCommand::Play(_) | EngineCommand::RoutineStart | EngineCommand::RoutineStop |
                        EngineCommand::RoutineConfigure(_) | EngineCommand::RoutineGate(_) => {}
                    }
                }


            }
        }
    }
}

fn set_state(engine: &PatternEngine, current: &mut RunnerState, new_state: RunnerState) {
    *current = new_state;
    let engine_state = new_state.as_engine_state();
    info!("Engine state: {:?}", engine_state);
    store_and_publish(engine, engine_state);
}

fn store_and_publish(engine: &PatternEngine, state: EngineState) {
    engine.state.store(state.encode(), Ordering::Relaxed);
    engine
        .state_channel
        .immediate_publisher()
        .publish_immediate(state);
}

fn apply_routine_field(cfg: &mut RoutineConfig, field: RoutineField, value: f64) {
    let pct = (value.clamp(0.0, 1.0) * 100.0 + 0.5) as u32;
    match field {
        RoutineField::Head => cfg.head = value.clamp(0.0, 1.0),
        RoutineField::Suck => cfg.suck = value.clamp(0.0, 1.0),
        RoutineField::Dt => cfg.dt = value.clamp(0.0, 1.0),
        RoutineField::Vdt => cfg.vdt = value.clamp(0.0, 1.0),
        RoutineField::Speed => cfg.speed = value.clamp(0.0, 1.0),
        RoutineField::CountMin => cfg.count_min = pct.clamp(1, 100),
        RoutineField::CountMax => cfg.count_max = pct.clamp(1, 100),
        RoutineField::DtChance => cfg.dt_chance = pct.min(100),
        RoutineField::VdtChance => cfg.vdt_chance = pct.min(100),
        RoutineField::DtEvery => cfg.dt_every = pct,
        RoutineField::HoldMin => cfg.hold_min_ms = pct.min(60) * 1000,
        RoutineField::HoldMax => cfg.hold_max_ms = pct.min(60) * 1000,
    }
    info!(
        "XToys routine field {:?}={:.1}; head={:.1}% suck={:.1}% dt={:.1}% vdt={:.1}% speed={:.1}% count={}-{} dt={}% vdt={}% forced_dt={} hold={}-{}ms",
        field, value * 100.0, cfg.head * 100.0, cfg.suck * 100.0, cfg.dt * 100.0, cfg.vdt * 100.0,
        cfg.speed * 100.0, cfg.count_min, cfg.count_max, cfg.dt_chance, cfg.vdt_chance,
        cfg.dt_every, cfg.hold_min_ms, cfg.hold_max_ms
    );
}

async fn handle_command<const N: usize>(
    engine: &PatternEngine,
    motion: &MotionSender,
    cmd: EngineCommand,
    state: &mut RunnerState,
    stream_queue: &mut Deque<StreamMove, 24>,
    routine_cfg: &mut RoutineConfig,
    routine_rt: &mut RoutineRuntime,
) {
    match cmd {
        EngineCommand::Play(idx) if idx < N => match *state {
            RunnerState::Idle => set_state(engine, state, RunnerState::Homing(AfterHome::Play(idx))),
            RunnerState::Homing(_) => log::warn!("Ignoring Play command while homing"),
            _ => set_state(engine, state, RunnerState::Playing(idx)),
        },
        EngineCommand::Play(_) => {}
        EngineCommand::Stop => {
            if motion.disable().await == StateResponse::Fault {
                log::error!("Board fault during disable");
            }
            set_state(engine, state, RunnerState::Idle);
        }
        EngineCommand::Home => {
            if let RunnerState::Idle = *state {
                set_state(engine, state, RunnerState::Homing(AfterHome::Ready));
            }
        }
        EngineCommand::StartStreaming => {
            if let RunnerState::Ready = *state {
                set_state(engine, state, RunnerState::Streaming);
                info!("XToys streaming active");
            } else {
                log::warn!("XToys streaming requires a completed home first");
            }
        }
        EngineCommand::XToysStop => {
            if let RunnerState::Ready = *state {
                stream_queue.clear();
                info!("XToys stop while Ready; pre-stream queue cleared; homing preserved");
            }
        }
        EngineCommand::StreamMove(item) => {
            if let RunnerState::Ready = *state {
                if item.replace {
                    stream_queue.clear();
                }
                if stream_queue.push_back(item).is_err() {
                    log::warn!("XToys pre-stream queue full; dropping move");
                } else {
                    info!(
                        "XToys move queued while Ready: target={:.1}% time={}ms",
                        item.position * 100.0,
                        item.time_ms
                    );
                }
            } else {
                log::warn!("Ignoring XToys move while not homed/Ready");
            }
        }
        EngineCommand::RoutineSetField(field, value) => {
            apply_routine_field(routine_cfg, field, value);
        }
        EngineCommand::RoutineConfigure(cfg) => {
            *routine_cfg = cfg;
            info!(
                "XToys routine config: head={:.1}% suck={:.1}% dt={:.1}% vdt={:.1}% speed={:.1}% count={}-{} dt={}% vdt={}% forced_dt={} hold={}-{}ms",
                cfg.head * 100.0, cfg.suck * 100.0, cfg.dt * 100.0, cfg.vdt * 100.0, cfg.speed * 100.0,
                cfg.count_min, cfg.count_max, cfg.dt_chance, cfg.vdt_chance, cfg.dt_every, cfg.hold_min_ms, cfg.hold_max_ms
            );
        }
        EngineCommand::RoutineStart => {
            if owner_limits::estop_active() {
                log::warn!("Ignoring XToys routine arm while E-stop is active");
            } else if let RunnerState::Ready = *state {
                routine_rt.armed = true;
                routine_rt.remaining = 0;
                info!("XToys firmware-owned routine armed; hold Pawprint to start/run");
                if routine_rt.gate_down {
                    routine_rt.rng = (Instant::now().as_millis() as u32) ^ 0xa5a5_5a5a;
                    if routine_rt.rng == 0 { routine_rt.rng = 0x6d2b79f5; }
                    routine_rt.remaining = routine_rt.range_inclusive(routine_cfg.count_min, routine_cfg.count_max);
                    routine_rt.since_dt = 0;
                    routine_rt.hold_ms = 0;
                    set_state(engine, state, RunnerState::Routine(RoutinePhase::PrimeHead));
                    info!("Pawprint already down; routine started; randomized count={}", routine_rt.remaining);
                }
            } else {
                log::warn!("XToys routine arm requires completed home / Ready state");
            }
        }
        EngineCommand::RoutineGate(down) => {
            routine_rt.gate_down = down;
            if down && routine_rt.armed {
                if owner_limits::estop_active() {
                    log::warn!("Ignoring Pawprint DOWN while E-stop is active");
                } else if let RunnerState::Ready = *state {
                    routine_rt.rng = (Instant::now().as_millis() as u32) ^ 0xa5a5_5a5a;
                    if routine_rt.rng == 0 { routine_rt.rng = 0x6d2b79f5; }
                    routine_rt.remaining = routine_rt.range_inclusive(routine_cfg.count_min, routine_cfg.count_max);
                    routine_rt.since_dt = 0;
                    routine_rt.hold_ms = 0;
                    set_state(engine, state, RunnerState::Routine(RoutinePhase::PrimeHead));
                    info!("XToys routine Pawprint DOWN -> started; randomized count={} (range {}-{})", routine_rt.remaining, routine_cfg.count_min, routine_cfg.count_max);
                }
            } else if !down {
                info!("XToys routine Pawprint UP while Ready");
            }
        }
        EngineCommand::RoutineStop => {
            routine_rt.armed = false;
            routine_rt.gate_down = false;
            routine_rt.remaining = 0;
            if let RunnerState::Ready = *state {
                info!("XToys routine disarmed while Ready");
            }
        }
        EngineCommand::Pause | EngineCommand::Resume => {
            // Only handled inside active motion loops.
        }
    }
}
