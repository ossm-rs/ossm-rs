use esp_hal::gpio::AnyPin;

pub struct Pins {
    pub rs485_rx: AnyPin<'static>,
    pub rs485_tx: AnyPin<'static>,
    pub rs485_transmit_enable: Option<AnyPin<'static>>,
    pub rs485_receive_enable_inv: Option<AnyPin<'static>>,
    pub i2c_sda: Option<AnyPin<'static>>,
    pub i2c_scl: Option<AnyPin<'static>>,
}

impl Pins {
    pub fn new(rs485_rx: AnyPin<'static>, rs485_tx: AnyPin<'static>) -> Self {
        Self {
            rs485_rx,
            rs485_tx,
            rs485_transmit_enable: None,
            rs485_receive_enable_inv: None,
            i2c_sda: None,
            i2c_scl: None,
        }
    }
    pub fn with_rs485_transmit_enable(mut self, pin: AnyPin<'static>) -> Self {
        self.rs485_transmit_enable = Some(pin);
        self
    }
    // Not all boards have this
    #[allow(dead_code)]
    pub fn with_rs485_receive_enable_inv(mut self, pin: AnyPin<'static>) -> Self {
        self.rs485_receive_enable_inv = Some(pin);
        self
    }
    pub fn with_i2c_sda(mut self, pin: AnyPin<'static>) -> Self {
        self.i2c_sda = Some(pin);
        self
    }
    pub fn with_i2c_scl(mut self, pin: AnyPin<'static>) -> Self {
        self.i2c_scl = Some(pin);
        self
    }
}
