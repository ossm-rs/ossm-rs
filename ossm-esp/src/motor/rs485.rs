use embassy_time::Delay;
use esp_hal::gpio::AnyPin;
use esp_hal::peripherals::UART1;
use esp_hal::uart::{Config as UartConfig, Uart};
use m57aim_motor::{
    provision, Modbus, Motor57AIM, Motor57AIMConfig, DEFAULT_DEVICE_ADDR, TARGET_BAUD_RATE,
};
use rs485_board::Rs485ModbusTransport;

use crate::uart::NonBlockingUart;

pub struct Config {
    pub uart1: UART1<'static>,
    pub uart_tx: AnyPin<'static>,
    pub uart_rx: AnyPin<'static>,
    pub rs485_de: AnyPin<'static>,
}

pub type Transport = Rs485ModbusTransport<NonBlockingUart<'static>, Delay>;
pub type Motor = Motor57AIM<Modbus<Transport>, Delay>;

pub async fn build(config: Config) -> Motor {
    let uart_config = UartConfig::default().with_baudrate(TARGET_BAUD_RATE.as_int());
    let uart = Uart::new(config.uart1, uart_config)
        .expect("Failed to initialize UART")
        .with_tx(config.uart_tx)
        .with_rx(config.uart_rx);

    // Safety: `enable_uart1_rs485` requires UART1 to be initialised; `Uart::new`
    // above satisfies that, and we own `config.uart1` so no other code is
    // touching UART1 registers concurrently.
    unsafe { crate::rs485::enable_uart1_rs485(config.rs485_de) };

    let transport = Rs485ModbusTransport::new(NonBlockingUart(uart), Delay);
    provision(
        transport,
        DEFAULT_DEVICE_ADDR,
        Motor57AIMConfig::default(),
        Delay,
    )
    .await
}
