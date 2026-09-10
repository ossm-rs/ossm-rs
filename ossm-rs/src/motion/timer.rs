use ossm_motion::motion_control::timer::{Instant, Timer};

pub struct EspTimer {}

impl EspTimer {
    pub fn new() -> Self {
        Self {}
    }
}

impl Timer for EspTimer {
    fn now(&self) -> Instant {
        let duration = esp_hal::time::Instant::now().duration_since_epoch();

        Instant::from_ticks(duration.as_micros())
    }
}
