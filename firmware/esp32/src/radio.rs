use embassy_executor::Spawner;
use esp_hal::peripherals::BT;
use esp_radio::ble::controller::BleConnector;
use pattern_engine::PatternSender;

use crate::mk_static;

pub fn start(spawner: &Spawner, bt: BT<'static>, patterns: &'static PatternSender) {
    let radio = &*mk_static!(
        esp_radio::Controller<'static>,
        esp_radio::init().expect("Failed to initialize radio controller")
    );

    let connector = BleConnector::new(radio, bt, Default::default())
        .expect("Could not create BleConnector");
    ble_remote::start(spawner, connector, patterns);
}
