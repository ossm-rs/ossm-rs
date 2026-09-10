use esp_hal::Blocking;
use esp_hal::uart::{Config as UartConfig, Uart};
use ossm::{UartReconfigure, ReadNonBlocking};

/// Newtype wrapper around `Uart<Blocking>` that provides non-blocking
/// reads via `read_buffered()`. The standard `embedded_io::Read` impl
/// on `Uart<Blocking>` blocks until data arrives, which prevents the
/// modbus transport from implementing timeouts.
pub struct NonBlockingUart<'d>(pub Uart<'d, Blocking>);

impl UartReconfigure for NonBlockingUart<'_> {
    type Error = esp_hal::uart::ConfigError;

    async fn reconfigure_baud(&mut self, baud: u32) -> Result<(), Self::Error> {
        self.0
            .apply_config(&UartConfig::default().with_baudrate(baud))
    }
}

impl embedded_io::ErrorType for NonBlockingUart<'_> {
    type Error = esp_hal::uart::IoError;
}

impl ReadNonBlocking for NonBlockingUart<'_> {
    fn read_nb(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        // read_buffered returns immediately with 0 if the FIFO is empty.
        // RX errors are treated as "no data" to avoid stalling the
        // transport on transient UART glitches.
        Ok(self.0.read_buffered(buf).unwrap_or(0))
    }
}

impl embedded_io::Write for NonBlockingUart<'_> {
    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        embedded_io::Write::write(&mut self.0, buf)
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        embedded_io::Write::flush(&mut self.0)
    }
}
