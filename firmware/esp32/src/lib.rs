#![no_std]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
#![deny(clippy::large_stack_frames)]

#[cfg(not(feature = "motor-stepdir"))]
compile_error!(
    "This crate currently requires the motor-stepdir feature. Add --features motor-sim to \
    overlay a simulated motor for bench testing."
);

mod board;
mod motor;
mod radio;

pub use board::Config as BoardConfig;
pub use motor::Config as MotorConfig;

use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;
use embassy_time::{Delay, Duration, Ticker};
use esp_hal::{
    interrupt::{Priority, software::SoftwareInterruptControl},
    peripherals::{BT, CPU_CTRL, SW_INTERRUPT, TIMG0},
    system::Stack,
    timer::timg::TimerGroup,
};
use esp_rtos::embassy::InterruptExecutor;
use log::info;
use ossm::{MechanicalConfig, MotionController, MotionLimits, Ossm};
use pattern_engine::{AnyPattern, PatternEngine, PatternSender};
use static_cell::StaticCell;

extern crate alloc;

#[macro_export]
macro_rules! mk_static {
    ($t:ty, $val:expr) => {{
        static STATIC_CELL: ::static_cell::StaticCell<$t> = ::static_cell::StaticCell::new();
        STATIC_CELL.init($val)
    }};
}

const UPDATE_INTERVAL_SECS: f64 = 0.01;

static OSSM_CELL: StaticCell<Ossm> = StaticCell::new();
static PATTERNS_CELL: StaticCell<PatternEngine> = StaticCell::new();

static EXECUTOR_CORE_1: StaticCell<InterruptExecutor<2>> = StaticCell::new();
// ESP32 DRAM is tighter than ESP32-S3. 16KB stack is needed for ruckig's
// trajectory calculator (heavy float math). 64KB heap is needed for the BLE
// radio stack's internal allocations. ESP-NOW was dropped to fit within
// the ESP32's memory constraints.
static APP_CORE_STACK: StaticCell<Stack<32768>> = StaticCell::new();
static MOTION_READY: Signal<CriticalSectionRawMutex, bool> = Signal::new();

pub struct Config {
    pub motor: motor::Config,
    pub board: board::Config,
    pub bt: BT<'static>,
    pub timg0: TIMG0<'static>,
    pub sw_int: SW_INTERRUPT<'static>,
    pub cpu_ctrl: CPU_CTRL<'static>,
}

#[embassy_executor::task]
async fn motion_task(mut controller: MotionController<'static, board::Board>) {
    let interval_us = (UPDATE_INTERVAL_SECS * 1_000_000.0) as u64;
    let mut ticker = Ticker::every(Duration::from_micros(interval_us));

    loop {
        if let Err(e) = controller.update().await {
            log::error!("Motion controller fault: {:?}", e);
        }
        ticker.next().await;
    }
}

pub async fn run(spawner: Spawner, config: Config) {
    ossm::logging::init(log::LevelFilter::Info, |line| {
        esp_println::println!("{}", line);
    });

    ossm::build_info!();

    esp_alloc::heap_allocator!(size: 64 * 1024);

    let timg0 = TimerGroup::new(config.timg0);
    esp_rtos::start(timg0.timer0);

    let motor = motor::build(config.motor);

    static MECHANICAL: MechanicalConfig = MechanicalConfig {
        pulley_teeth: 20,
        belt_pitch_mm: 2.0,
        reverse_direction: false,
    };
    let limits = MotionLimits::default();

    let (receiver, _observer, motion) = OSSM_CELL.init(Ossm::new()).split();

    let board = board::build(motor, config.board, &MECHANICAL);
    let controller = receiver.into_controller(board, limits.clone(), UPDATE_INTERVAL_SECS);

    let sw_int = SoftwareInterruptControl::new(config.sw_int);
    let app_core_stack = APP_CORE_STACK.init(Stack::new());

    let second_core = move || {
        let executor = InterruptExecutor::new(sw_int.software_interrupt2);
        let executor = EXECUTOR_CORE_1.init(executor);
        let spawner = executor.start(Priority::Priority2);

        spawner.spawn(motion_task(controller)).unwrap();

        MOTION_READY.signal(true);

        loop {}
    };

    esp_rtos::start_second_core(
        config.cpu_ctrl,
        sw_int.software_interrupt0,
        sw_int.software_interrupt1,
        app_core_stack,
        second_core,
    );

    MOTION_READY.wait().await;

    info!(
        "Motion task started on core 1 at {}ms interval",
        UPDATE_INTERVAL_SECS * 1000.0
    );

    let (runner, _observer, patterns) = PATTERNS_CELL.init(PatternEngine::new()).split();
    let patterns: &'static PatternSender = mk_static!(PatternSender, patterns);

    radio::start(&spawner, config.bt, patterns);

    runner.run(&motion, AnyPattern::all_builtin(), Delay).await
}
