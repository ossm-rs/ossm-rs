#![no_std]
#![no_main]

use {esp_backtrace as _, esp_println as _};

esp_bootloader_esp_idf::esp_app_desc!();

#[esp_rtos::main]
async fn main(spawner: embassy_executor::Spawner) {
    let p = esp_hal::init(esp_hal::Config::default());

    let config = esp32s3::Config {
        motor: esp32s3::MotorConfig {
            uart1: p.UART1,
            uart_tx: p.GPIO5.into(),
            uart_rx: p.GPIO6.into(),
            rs485_de: p.GPIO3.into(),
        },
        wifi: p.WIFI,
        bt: p.BT,
        flash: p.FLASH,
        timg0: p.TIMG0,
        sw_int: p.SW_INTERRUPT,
        cpu_ctrl: p.CPU_CTRL,
        usb_device: p.USB_DEVICE,
    };

    esp32s3::run(spawner, config).await;
}
