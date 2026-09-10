use embassy_time::{Duration, Ticker};

use ossm_motion::motion::motion_state::set_motion_enabled;

use crate::remote::{ble::is_ble_connected, esp_now::is_m5_connected};

pub mod ble;
pub mod esp_now;

#[embassy_executor::task]
pub async fn remote_connection_task() {
    let mut ticker = Ticker::every(Duration::from_millis(1000));

    loop {
        if !(is_m5_connected() || is_ble_connected()) {
            set_motion_enabled(false);
        }

        ticker.next().await;
    }
}
