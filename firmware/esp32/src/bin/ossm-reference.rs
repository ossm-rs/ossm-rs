#![no_std]
#![no_main]

use {esp_backtrace as _, esp_println as _};

esp_bootloader_esp_idf::esp_app_desc!();

#[esp_rtos::main]
async fn main(spawner: embassy_executor::Spawner) {
    let p = esp_hal::init(esp_hal::Config::default());

    let config = esp32::Config {
        motor: esp32::MotorConfig {
            rmt: p.RMT,
            step: p.GPIO14.into(),
            dir: p.GPIO27.into(),
            enable: p.GPIO26.into(),
        },
        board: esp32::BoardConfig {
            adc1: p.ADC1,
            current_pin: p.GPIO36,
        },
        bt: p.BT,
        timg0: p.TIMG0,
        sw_int: p.SW_INTERRUPT,
        cpu_ctrl: p.CPU_CTRL,
    };

    esp32::run(spawner, config).await;
}
