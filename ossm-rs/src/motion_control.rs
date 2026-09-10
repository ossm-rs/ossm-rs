use core::cell::RefCell;

use critical_section::Mutex;
use log::{debug, error, info};
use esp_hal::{handler, interrupt::Priority, time::Duration, timer::PeriodicTimer, Blocking};

use crate::{motion::timer::EspTimer, motor::m57aimxx::Motor57AIMxx};
use ossm_motion::{config::MOTION_CONTROL_LOOP_UPDATE_INTERVAL_MS, motion_control::{MotionControl, debug::DummyDebugOut}};

pub static UPDATE_TIMER: Mutex<RefCell<Option<PeriodicTimer<'static, Blocking>>>> =
    Mutex::new(RefCell::new(None));
static MOTION_CONTROL: Mutex<RefCell<Option<MotionControl<Motor57AIMxx, EspTimer, DummyDebugOut>>>> =
    Mutex::new(RefCell::new(None));

// Timer interrupt
#[handler(priority = Priority::Priority2)]
pub fn motion_control_interrupt() {
    critical_section::with(|cs| {
        UPDATE_TIMER
            .borrow_ref_mut(cs)
            .as_mut()
            .unwrap()
            .clear_interrupt();
        MOTION_CONTROL
            .borrow_ref_mut(cs)
            .as_mut()
            .unwrap()
            .update_handler();
    });
}

pub struct EspMotionControl {}

impl EspMotionControl {
    /// Initialises the MotionControl and allows the use of attached functions
    pub fn init(mut update_timer: PeriodicTimer<'static, Blocking>, mut motor: Motor57AIMxx) {
        info!("ESP Motion Control Init");

        // Motion control over modbus
        motor.enable_modbus(true).expect("Failed to enable modbus");

        update_timer.set_interrupt_handler(motion_control_interrupt);
        update_timer.listen();

        let esp_timer = EspTimer::new();

        let motion_control = MotionControl::new(motor, esp_timer);

        update_timer
            .start(Duration::from_millis(
                MOTION_CONTROL_LOOP_UPDATE_INTERVAL_MS,
            ))
            .expect("Failed to start periodic timer");

        critical_section::with(|cs| {
            MOTION_CONTROL.borrow_ref_mut(cs).replace(motion_control);
            UPDATE_TIMER.borrow_ref_mut(cs).replace(update_timer);
        });
    }
}
