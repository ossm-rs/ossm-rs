#![no_std]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
#![deny(clippy::large_stack_frames)]

#[cfg(not(feature = "motor-rs485"))]
compile_error!(
    "This crate currently requires the motor-rs485 feature. Add --features motor-sim to \
    overlay a simulated motor for bench testing."
);

mod board;
mod motor;
mod owner_control;
mod owner_auth;
mod owner_web;
mod owner_settings;
mod radio;
mod wifi_settings;

pub use motor::Config as MotorConfig;

use embassy_executor::Spawner;
use embassy_time::{Delay, Duration, Ticker};
use esp_hal::{
    peripherals::{BT, CPU_CTRL, FLASH, SW_INTERRUPT, TIMG0, USB_DEVICE, WIFI},
    timer::timg::TimerGroup,
};
use log::info;
use ossm::{MechanicalConfig, MotionController, MotionLimits, Ossm};
use esp_hal::usb_serial_jtag::UsbSerialJtag;
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

pub struct Config {
    pub motor: motor::Config,
    pub wifi: WIFI<'static>,
    pub bt: BT<'static>,
    pub flash: FLASH<'static>,
    pub timg0: TIMG0<'static>,
    pub sw_int: SW_INTERRUPT<'static>,
    pub cpu_ctrl: CPU_CTRL<'static>,
    pub usb_device: USB_DEVICE<'static>,
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
    ossm::build_info!();

    esp_alloc::heap_allocator!(size: 168 * 1024);

    let timg0 = TimerGroup::new(config.timg0);
    esp_rtos::start(timg0.timer0);

    let motor = motor::build(config.motor).await;

    static MECHANICAL: MechanicalConfig = MechanicalConfig {
        pulley_teeth: 20,
        belt_pitch_mm: 2.0,
        reverse_direction: false, // verified direction for this machine
    };
    let mut limits = MotionLimits::default();
    pattern_engine::owner_limits::configure_machine(
        limits.max_velocity_mm_s,
        limits.max_position_mm - limits.min_position_mm,
    );

    let settings_flash: &'static wifi_settings::WifiFlash = mk_static!(
        wifi_settings::WifiFlash,
        embassy_sync::mutex::Mutex::new(esp_storage::FlashStorage::new(config.flash))
    );
    owner_settings::load_and_apply(settings_flash).await;
    // Machine settings are reboot-applied to the actual low-level controller,
    // not merely to owner/XToys scaling. The compiled min position remains the
    // home-side safety offset; configurable length changes the usable span.
    limits.max_velocity_mm_s = pattern_engine::owner_limits::machine_max_speed_mm_s();
    limits.max_position_mm = limits.min_position_mm + pattern_engine::owner_limits::machine_travel_mm();
    owner_auth::load(settings_flash).await;

    let (receiver, _motion_observer, motion) = OSSM_CELL.init(Ossm::new()).split();

    let board = board::build(motor, &MECHANICAL);
    let controller = receiver.into_controller(board, limits.clone(), UPDATE_INTERVAL_SECS);

    // Run the 10 ms motion task on the main/ProCpu Embassy executor.
    // The dedicated AppCpu executor is intentionally not used in this baseline.
    spawner.must_spawn(motion_task(controller));

    // These peripherals are unused because the dedicated AppCpu executor is disabled.
    let _sw_int_unused = config.sw_int;
    let _cpu_ctrl_unused = config.cpu_ctrl;

    info!(
        "Motion task started on ProCpu/main executor at {}ms interval",
        UPDATE_INTERVAL_SECS * 1000.0
    );

    let (runner, observer, patterns) = PATTERNS_CELL.init(PatternEngine::new()).split();
    let observer: &'static pattern_engine::PatternObserver = mk_static!(pattern_engine::PatternObserver, observer);
    let patterns: &'static PatternSender = mk_static!(PatternSender, patterns);

    // Local owner USB RX remains available alongside the Wi-Fi owner page.
    let owner_usb = UsbSerialJtag::new(config.usb_device);
    spawner.must_spawn(owner_control::owner_usb_task(owner_usb, patterns));
    spawner.must_spawn(owner_control::owner_supervisor_task(patterns));
    spawner.must_spawn(owner_web::bpm_control_task(patterns));

    radio::start(&spawner, config.wifi, config.bt, patterns, &limits, settings_flash).await;

    runner.run(&motion, AnyPattern::all_builtin(), Delay).await
}
