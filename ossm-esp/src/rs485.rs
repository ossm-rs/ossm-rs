use esp_hal::gpio::OutputPin;
use esp_hal::peripherals::GPIO;

#[cfg(not(any(feature = "esp32s3")))]
compile_error!(
    "Hardware RS485 DE control has only been validated on ESP32-S3. \
     Enable the `esp32s3` feature, or add your chip after verifying \
     the U1DTR signal index in its TRM."
);

/// ESP32-S3 GPIO matrix output signal index for U1DTR_OUT.
#[cfg(feature = "esp32s3")]
const UART1_DTR_SIGNAL: u16 = 17;

/// Enable hardware RS485 mode on UART1, routing DTR to the given DE pin.
///
/// Enables RS485 mode on UART1 and routes the DTR output signal to the
/// specified GPIO via the GPIO matrix. The UART peripheral then drives
/// the DE pin in lockstep with the shift register, asserting it for the
/// exact duration of each transmitted byte including stop bits. This
/// eliminates the timing races inherent in software DE toggling, where
/// dropping DE after `flush()` can corrupt the final byte because the
/// shift register hasn't finished clocking it out.
///
/// Consumes the DE pin to prevent other code from reconfiguring it.
/// Configures the pin's IO_MUX registers directly (push-pull, 20mA
/// drive, no pulls) so the pin is in a known state regardless
/// of how the caller obtained it.
///
/// # Safety
///
/// Writes directly to UART1, IO_MUX, and GPIO matrix registers. UART1
/// must be initialised before calling this function.
pub unsafe fn enable_uart1_rs485(de_pin: impl OutputPin) {
    let gpio_num = de_pin.number() as usize;

    // Verify UART1 peripheral clock is enabled, which implies it has
    // been initialised. Without this, register writes silently fail.
    let system = unsafe { &*esp_hal::peripherals::SYSTEM::ptr() };
    assert!(
        system.perip_clk_en0().read().uart1_clk_en().bit_is_set(),
        "UART1 peripheral clock is not enabled - call Uart::new() first"
    );

    // Configure the pin's IO_MUX so it's in a known electrical state
    // before handing it to the GPIO matrix.
    let io_mux = unsafe { &*esp_hal::peripherals::IO_MUX::ptr() };
    io_mux.gpio(gpio_num).modify(|_, w| unsafe {
        w.fun_drv().bits(2); // ~20mA drive
        w.fun_wpu().clear_bit();
        w.fun_wpd().clear_bit();
        w
    });

    let gpio = GPIO::regs();
    gpio.pin(gpio_num).modify(|_, w| w.pad_driver().clear_bit()); // push-pull

    let uart = unsafe { &*esp_hal::peripherals::UART1::ptr() };
    uart.rs485_conf().modify(|_, w| {
        // Enable RS485 half-duplex mode. The UART peripheral will
        // assert DTR for the duration of each transmitted byte.
        w.rs485_en().set_bit();

        // No extra stop-bit delay.
        w.dl0_en().clear_bit();
        w.dl1_en().clear_bit();

        // No TX-to-RX loopback.
        w.rs485tx_rx_en().clear_bit();

        // Don't start TX while RX is active.
        w.rs485rxby_tx_en().clear_bit();

        // No additional delay on the internal RX data signal (1-bit field).
        w.rs485_rx_dly_num().bit(false);

        // No additional delay on the internal TX data signal (4-bit field).
        unsafe { w.rs485_tx_dly_num().bits(0) }
    });

    assert!(
        uart.rs485_conf().read().rs485_en().bit_is_set(),
        "RS485 mode enable bit did not stick on UART1"
    );

    // Route the UART1 DTR output signal to the DE pin via the GPIO matrix.
    // oen_sel=0: the UART peripheral controls output enable, not software.
    gpio.func_out_sel_cfg(gpio_num).modify(|_, w| unsafe {
        w.out_sel().bits(UART1_DTR_SIGNAL);
        w.inv_sel().clear_bit();
        w.oen_sel().clear_bit();
        w.oen_inv_sel().clear_bit()
    });

    assert!(
        gpio.func_out_sel_cfg(gpio_num).read().out_sel().bits() == UART1_DTR_SIGNAL,
        "GPIO matrix DTR signal routing did not stick for GPIO {}",
        gpio_num
    );
}
