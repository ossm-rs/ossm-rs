use std::sync::mpsc::Sender;

use embassy_time::{Instant, Ticker};
use ossm_motion::{
    config::MOTION_CONTROL_LOOP_UPDATE_INTERVAL_MS,
    motion_control::{
        MotionControl,
        debug::DebugOut,
        motor::Motor,
        timer::{Timer, TimerDuration, TimerInstant},
    },
};

use crate::plotting::PlotMessage;

pub async fn run_motion_control(tx: Sender<PlotMessage>) {
    let motor = DummyMotor::new();
    let timer = StdTimer::new();
    let debug = PlotDebug::new(tx);
    let mut motion_control = MotionControl::new_with_debug(motor, timer, debug);

    let mut ticker = Ticker::every(embassy_time::Duration::from_millis(
        MOTION_CONTROL_LOOP_UPDATE_INTERVAL_MS,
    ));
    loop {
        motion_control.update_handler();
        ticker.next().await;
    }
}

struct DummyMotor {}

impl DummyMotor {
    pub fn new() -> Self {
        Self {}
    }
}

impl Motor for DummyMotor {
    type MotorError = ();

    fn min_consecutive_write_delay() -> TimerDuration {
        TimerDuration::millis(1)
    }

    fn set_absolute_position(&mut self, _steps: i32) -> Result<(), Self::MotorError> {
        Ok(())
    }

    fn set_max_allowed_output(&mut self, _output: u16) -> Result<(), Self::MotorError> {
        Ok(())
    }

    fn delay(&mut self, duration: TimerDuration) {
        // TODO: Now noop to be compatible with WASM
    }
}

struct StdTimer {}

impl StdTimer {
    pub fn new() -> Self {
        Self {}
    }
}

impl Timer for StdTimer {
    fn now(&self) -> TimerInstant {
        let now = embassy_time::Instant::now();
        TimerInstant::from_ticks(now.as_micros())
    }
}

struct PlotDebug {
    tx: Sender<PlotMessage>,
}

impl PlotDebug {
    pub fn new(tx: Sender<PlotMessage>) -> Self {
        Self { tx }
    }
}

impl DebugOut for PlotDebug {
    fn new_position(&mut self, position: f64) {
        let time = Instant::now().as_micros() as f64 / 1000000.0;
        self.tx
            .send(PlotMessage::new("position", time, position))
            .unwrap();
    }

    fn new_velocity(&mut self, velocity: f64) {
        let time = Instant::now().as_micros() as f64 / 1000000.0;
        self.tx
            .send(PlotMessage::new("velocity", time, velocity))
            .unwrap();
    }

    fn new_acceleration(&mut self, acceleration: f64) {
        let time = Instant::now().as_micros() as f64 / 1000000.0;
        self.tx
            .send(PlotMessage::new("acceleration", time, acceleration))
            .unwrap();
    }

    fn new_jerk(&mut self, jerk: f64) {
        let time = Instant::now().as_micros() as f64 / 1000000.0;
    }
}
