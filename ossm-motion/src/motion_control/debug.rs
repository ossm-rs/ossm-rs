pub trait DebugOut {
    fn new_position(&mut self, position: f64);

    fn new_velocity(&mut self, velocity: f64);

    fn new_acceleration(&mut self, acceleration: f64);

    fn new_jerk(&mut self, jerk: f64);
}

pub struct DummyDebugOut {}

impl DummyDebugOut {
    pub fn new() -> Self {
        Self {}
    }
}

impl DebugOut for DummyDebugOut {
    fn new_position(&mut self, _position: f64) {}

    fn new_velocity(&mut self, _velocity: f64) {}

    fn new_acceleration(&mut self, _acceleration: f64) {}

    fn new_jerk(&mut self, _jerk: f64) {}
}
