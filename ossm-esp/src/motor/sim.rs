use sim_motor::SimMotor;

pub type Motor = SimMotor;

// Generic over the real motor's `Config` so the caller can pass whatever
// they'd pass to the real `build` - fields drop cheaply.
pub fn build<C>(_config: C) -> Motor {
    SimMotor::new()
}
