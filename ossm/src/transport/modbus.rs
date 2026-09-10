use heapless::Vec;

/// Async Modbus wire protocol abstraction.
///
/// RS-485 is the physical layer — a differential serial bus that allows
/// multiple devices on the same pair of wires. Modbus RTU runs on top of
/// RS-485 and defines how registers are addressed, read, and written using
/// function codes and CRC-checked frames.
///
/// Implementations of this trait handle framing, CRC, and physical transport
/// (RS-485 UART, TCP, etc.). Motor drivers use it to read/write registers
/// without knowing the wire details.
#[allow(async_fn_in_trait)]
pub trait ModbusTransport {
    type Error: core::fmt::Debug;

    /// Write a single holding register (function code 0x06).
    async fn write_holding(
        &mut self,
        device_addr: u8,
        register: u16,
        value: u16,
    ) -> Result<(), Self::Error>;

    /// Read one or more holding registers (function code 0x03).
    async fn read_holding(
        &mut self,
        device_addr: u8,
        register: u16,
        count: u16,
    ) -> Result<Vec<u16, 8>, Self::Error>;

    /// Send a raw frame and read the response.
    ///
    /// Used for vendor-specific function codes (e.g. the 57AIM's 0x7B
    /// absolute position command) that don't fit standard Modbus functions.
    async fn raw_transaction(
        &mut self,
        request: &[u8],
        response: &mut [u8],
    ) -> Result<usize, Self::Error>;
}

/// Modbus interface: wraps a transport + device address.
pub struct Modbus<T: ModbusTransport> {
    pub transport: T,
    pub device_addr: u8,
}

impl<T: ModbusTransport> Modbus<T> {
    pub fn new(transport: T, device_addr: u8) -> Self {
        Self {
            transport,
            device_addr,
        }
    }
}
