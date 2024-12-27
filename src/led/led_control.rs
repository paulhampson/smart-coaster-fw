use defmt::debug;
use embassy_futures::select::{select, Either};
use embassy_rp::peripherals::PIO0;
use embassy_rp::pio::Instance;
use embassy_rp::pio_programs::ws2812::PioWs2812;
use embassy_time::{Duration, Ticker};
use smart_leds::{brightness, gamma, RGB8};
use crate::application::application_state::ProductState;
use crate::hmi::event_channels::{HmiEventChannelReceiver, HmiEvents};

pub enum LedSpeed {
    Fast,
    Slow,
}

pub enum LedArrayMode {
    Off,
    RainbowWheel,
    SingleColourWheel(RGB8),
    Pulse(RGB8),
    StaticColour(RGB8),
}

/// Input a value 0 to 255 to get a color value
/// The colours are a transition r - g - b - back to r.
fn rainbow_wheel(mut wheel_pos: u8) -> RGB8 {
    wheel_pos = 255 - wheel_pos;
    if wheel_pos < 85 {
        return (255 - wheel_pos * 3, 0, wheel_pos * 3).into();
    }
    if wheel_pos < 170 {
        wheel_pos -= 85;
        return (0, wheel_pos * 3, 255 - wheel_pos * 3).into();
    }
    wheel_pos -= 170;
    (wheel_pos * 3, 255 - wheel_pos * 3, 0).into()
}

fn single_colour_wheel(colour:RGB8, wheel_pos: u8) -> RGB8 {
    let intensity = match wheel_pos < 128 {
        true => {255 - (wheel_pos*2)},
        false => {(wheel_pos - 128) * 2}
    };
    ((intensity as u32 * colour.r as u32 / 255) as u8,
     (intensity as u32 * colour.g as u32 / 255) as u8,
     (intensity as u32 * colour.b as u32 / 255) as u8).into()
}

pub struct LedControl<'a, const LED_COUNT: usize, P: Instance, const S: usize> {
    ws2812pio: PioWs2812<'a, P, S, LED_COUNT>,
    led_count: usize,
    led_state: [RGB8; LED_COUNT],
    display_state: ProductState,
    array_mode: LedArrayMode,
    animation_step: u32,
}

impl<'a, const LED_COUNT: usize, P, const S: usize> LedControl<'a, LED_COUNT, P, S>
where
    P: Instance,
{
    pub fn new(ws2812pio: PioWs2812<'a, P, S, LED_COUNT>) -> Self {
        Self {
            ws2812pio,
            led_count: LED_COUNT,
            led_state: [RGB8::default(); LED_COUNT],
            display_state: ProductState::Startup,
            array_mode: LedArrayMode::Off,
            animation_step: 0,
        }
    }

    pub fn set_mode(&mut self, mode: LedArrayMode) {
        self.array_mode = mode;
    }

    pub async fn led_update(&mut self) {
        match self.array_mode {
            LedArrayMode::Off => {self.off()}
            LedArrayMode::RainbowWheel => {self.rainbow_wheel()}
            LedArrayMode::SingleColourWheel(c) => {self.single_colour_wheel(c)}
            LedArrayMode::Pulse(c) => {self.pulse(c)}
            LedArrayMode::StaticColour(c) => {self.static_colour(c)}
        };

        let mut leds = [RGB8::default(); LED_COUNT];
        for (idx, led) in gamma(self.led_state.iter().cloned()).enumerate() {
            leds[idx] = led;
        }
        self.ws2812pio.write(&leds).await;
    }

    fn off(&mut self) {
        for i in 0..self.led_count {
            self.led_state[i] = RGB8::new(0,0,0);
        }
    }

    fn rainbow_wheel(&mut self) {
        for i in 0..self.led_count {
            self.led_state[i] = rainbow_wheel((((i * 256) as u16 / self.led_count as u16 + self.animation_step as u16) & 255) as u8);
        }

        self.animation_step += 1;
        if self.animation_step >= 256 * self.led_count as u32 {
            self.animation_step = 0;
        }
    }

    fn single_colour_wheel(&mut self, colour:RGB8) {
        for i in 0..self.led_count {
            let wheel_pos = ((i * 256 / self.led_count) as u32 + self.animation_step) as u8;
            self.led_state[i] = single_colour_wheel(colour, wheel_pos);
        }

        self.animation_step += 1;
        if self.animation_step >= 256 * self.led_count as u32 {
            self.animation_step = 0;
        }
    }

    fn pulse(&mut self, colour:RGB8) {
        let intensity_factor = match self.animation_step > 255 {
            true => 255 - (self.animation_step - 255),
            false => self.animation_step,
        };
        let current_colour:RGB8 = ((intensity_factor * colour.r as u32 / 255) as u8,
                                   (intensity_factor * colour.g as u32 / 255) as u8,
                                    (intensity_factor * colour.b as u32 / 255) as u8 ).into();

        for i in 0..self.led_count {
            self.led_state[i] = current_colour;
        }

        self.animation_step += 1;
        if self.animation_step >= 512 {
            self.animation_step = 0;
        }
    }

    fn static_colour(&mut self, colour:RGB8) {
        for i in 0..self.led_count {
            self.led_state[i] = colour;
        }
    }
}

pub async fn led_update_handler<const LED_COUNT:usize>(pio_ws2812: PioWs2812<'_, PIO0, 0, LED_COUNT>, mut hmi_event_channel: HmiEventChannelReceiver<'_>) {
    let mut led_control = LedControl::new(pio_ws2812);

    let mut ticker = Ticker::every(Duration::from_millis(10));
    loop {
        let timer_or_state_change = select(ticker.next(), hmi_event_channel.next_message_pure()).await;
        match timer_or_state_change {
            Either::First(_) => {
                led_control.led_update().await;
            },
            Either::Second(message) => {
                match message {
                    HmiEvents::ChangeProductState(new_state) => {
                        match new_state {
                            ProductState::Startup => {led_control.set_mode(LedArrayMode::Off);}
                            ProductState::Home => { led_control.set_mode(LedArrayMode::RainbowWheel);}
                            ProductState::Tare => {led_control.set_mode( LedArrayMode::StaticColour(RGB8::new(255, 0, 0)));}
                            ProductState::Calibration(_) => {led_control.set_mode(LedArrayMode::Pulse(RGB8::new(177, 3, 252)));}
                            ProductState::CalibrationDone => {led_control.set_mode(LedArrayMode::SingleColourWheel(RGB8::new(0, 255, 0)));}
                            ProductState::Wait => {}
                        }
                    }
                    _ => {}
                }
                led_control.led_update().await;
            }
        }
    }
}