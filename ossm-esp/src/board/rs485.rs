use ossm::MechanicalConfig;
use rs485_board::Rs485Board;

use crate::motor::rs485::Motor;

pub type Board = Rs485Board<Motor>;

pub fn build(motor: Motor, mechanical: &'static MechanicalConfig) -> Board {
    Rs485Board::new(motor, mechanical)
}
