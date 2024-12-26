use crate::weight::interface::AsyncStrainGaugeInterface;
use embassy_time::{Duration, Ticker, Timer};

#[allow(dead_code)]
pub enum Hx711Gain {
    Gain128,
    Gain64,
    Gain32ChannelB,
}

impl Hx711Gain {
    fn tick_count(&self) -> usize {
        match self {
            Hx711Gain::Gain128 => {25}
            Hx711Gain::Gain64 => {27}
            Hx711Gain::Gain32ChannelB => {26}
        }
    }
}

const POWER_MODE_CHANGE_DELAY: Duration = Duration::from_micros(60);
const CLK_HALF_PERIOD: Duration = Duration::from_micros(1);
const VALID_DATA_BITS: usize = 24;

#[derive(Debug)]
pub enum Error<OutPinE, InPinE> {
    OutPin(OutPinE),
    InPin(InPinE)
}

pub struct Hx711Async<CLK, DATA> {
    clock_pin: CLK,
    data_pin: DATA,
    gain_clocks: usize,
    powered_up: bool,
}

#[allow(dead_code)]
impl<CLK, DATA, ClkE, DataE> Hx711Async<CLK, DATA>
where
    CLK: embedded_hal::digital::OutputPin<Error = ClkE>,
    DATA: embedded_hal_async::digital::Wait<Error = DataE> + embedded_hal::digital::InputPin<Error = DataE>,
{
    pub fn new(clock_pin: CLK, data_pin: DATA, gain: Hx711Gain) -> Self {
        Self {
            clock_pin,
            data_pin,
            gain_clocks: gain.tick_count(),
            powered_up: false,
        }
    }

    pub fn set_gain(&mut self, gain: Hx711Gain) {
        self.gain_clocks = gain.tick_count();
    }
}

impl<CLK, DATA, ClkE, DataE> AsyncStrainGaugeInterface for Hx711Async<CLK, DATA>
where
    CLK: embedded_hal::digital::OutputPin<Error = ClkE>,
    DATA: embedded_hal_async::digital::Wait<Error = DataE> + embedded_hal::digital::InputPin<Error = DataE>,
{
    type Error = Error<ClkE, DataE>;

    async fn initialize(&mut self) -> Result<(), Self::Error> {
        self.power_up().await
    }
    
    async fn get_next_reading(&mut self) -> Result<i32, Self::Error> {
        let mut clock_ticker = Ticker::every(CLK_HALF_PERIOD);

        if !self.powered_up {
            self.power_up().await?;
        }

        self.data_pin.wait_for_low().await.map_err(Error::InPin)?; // DOUT goes low when conversion is ready
        let mut data:i32 = 0;

        clock_ticker.next().await;
        for _ in 0..self.gain_clocks {
            data = data << 1;
            self.clock_pin.set_high().map_err(Error::OutPin)?;
            clock_ticker.next().await;
            self.clock_pin.set_low().map_err(Error::OutPin)?;
            let data_sample = self.data_pin.is_high().map_err(Error::InPin)?;
            if data_sample {
                data |= 0x1;
            }
            clock_ticker.next().await;
        }

        let data_bits_to_discard = self.gain_clocks - VALID_DATA_BITS;
        data = data >> data_bits_to_discard;
        let data_mask = (1 << VALID_DATA_BITS) - 1;
        data = data & data_mask;
        // extend sign if bit 24 is 1
        if (data >> 23) & 0x1 == 0x1 {
            data |= 0xFF000000u32 as i32;
        }
        Ok(data)
    }

    async fn power_down(&mut self) -> Result<(), Self::Error> {
        self.clock_pin.set_high().map_err(Error::OutPin)?;
        Timer::after(POWER_MODE_CHANGE_DELAY).await;
        self.powered_up = false;
        Ok(())
    }

    async fn power_up(&mut self) -> Result<(), Self::Error> {
        self.clock_pin.set_low().map_err(Error::OutPin)?;
        Timer::after(POWER_MODE_CHANGE_DELAY).await;
        self.powered_up = true;
        Ok(())
    }
}