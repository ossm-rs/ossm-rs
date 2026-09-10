use ossm::MechanicalConfig;
use sim_board::SimBoard;

use crate::motor::sim::Motor;

pub type Board = SimBoard;

// Generic over the real board's `Config` so callers can pass whatever
// they'd pass to the real `build` - fields drop cheaply.
pub fn build<C>(motor: Motor, _config: C, mechanical: &'static MechanicalConfig) -> Board {
    SimBoard::new(motor, mechanical)
}
