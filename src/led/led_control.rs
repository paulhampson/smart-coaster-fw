use crate::application::application_state::ProductState;
use crate::hmi::event_channels::{HmiEventChannelReceiver, HmiEvents};
use embassy_futures::select::{select, Either};
use embassy_rp::peripherals::PIO0;
use embassy_rp::pio::Instance;
use embassy_rp::pio_programs::ws2812::PioWs2812;
use embassy_time::{Duration, Ticker};
use smart_leds::{gamma, RGB8};
use micromath::F32Ext;
use smart_leds::hsv::{hsv2rgb, Hsv};

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

/// Input a position 0 to 255 to get a color value associated with that position.
fn rainbow_wheel(wheel_pos: u8) -> RGB8 {
    hsv2rgb(Hsv { hue: wheel_pos, sat: 255, val: 255 })
}

/// Input a position in range 0-255 to get the brightness of the colour associated with that
/// position.
fn single_colour_wheel(colour:RGB8, wheel_pos: u8) -> RGB8 {
    let intensity = match wheel_pos < 128 {
        true => {255 - (wheel_pos*2)},
        false => {(wheel_pos - 128) * 2}
    } as u32;

    ((intensity * colour.r as u32 / 255) as u8,
     (intensity * colour.g as u32 / 255) as u8,
     (intensity * colour.b as u32 / 255) as u8).into()
}



pub struct LedControl<'a, const LED_COUNT: usize, P: Instance, const S: usize> {
    ws2812pio: PioWs2812<'a, P, S, LED_COUNT>,
    led_count: usize,
    led_state: [RGB8; LED_COUNT],
    display_state: ProductState,
    array_mode: LedArrayMode,
    animation_position: f32,
    speed_factor: f32,
    repetition_factor: f32,
}

impl<'a, const LED_COUNT: usize, P, const S: usize> LedControl<'a, LED_COUNT, P, S>
where
    P: Instance,
{
    const LED_SINGLE_ROTATION_STEPS: f32 = 360.0;

    pub fn new(ws2812pio: PioWs2812<'a, P, S, LED_COUNT>) -> Self {
        Self {
            ws2812pio,
            led_count: LED_COUNT,
            led_state: [RGB8::default(); LED_COUNT],
            display_state: ProductState::Startup,
            array_mode: LedArrayMode::Off,
            animation_position: 0.0,
            speed_factor: 1.0,
            repetition_factor: 1.0,
        }
    }

    pub fn set_mode(&mut self, mode: LedArrayMode) {
        self.array_mode = mode;
    }

    pub fn set_speed_factor(&mut self, speed: f32) { self.speed_factor = speed; }

    pub fn set_repetition_factor(&mut self, repetition_factor: f32) { self.repetition_factor = repetition_factor; }

    pub async fn led_update(&mut self) {
        match self.array_mode {
            LedArrayMode::Off => {self.off()}
            LedArrayMode::RainbowWheel => {self.rainbow_wheel()}
            LedArrayMode::SingleColourWheel(c) => {self.single_colour_wheel(c)}
            LedArrayMode::Pulse(c) => {self.pulse(c)}
            LedArrayMode::StaticColour(c) => {self.static_colour(c)}
        };

        let mut corrected_leds = [RGB8::default(); LED_COUNT];
        for (idx, led) in gamma(self.led_state.iter().cloned()).enumerate() {
            corrected_leds[idx] = led;
        }
        self.ws2812pio.write(&corrected_leds).await;

        self.animation_position = match self.animation_position {
            0.0..360.0 => self.animation_position + self.speed_factor,
            _ => 0.0
        };

    }

    fn animation_position_to_u8(&self) -> u8 {
        (self.animation_position * u8::MAX as f32 / Self::LED_SINGLE_ROTATION_STEPS) as u8
    }

    fn off(&mut self) {
        for i in 0..self.led_count {
            self.led_state[i] = RGB8::new(0,0,0);
        }
    }

    fn rainbow_wheel(&mut self) {
        let scaled_position = self.animation_position_to_u8();

        for i in 0..self.led_count {
            self.led_state[i] = rainbow_wheel((((((i * 256) / self.led_count) as f32 * self.repetition_factor).round() as u16 + scaled_position as u16) & 255) as u8);
        }
    }

    fn single_colour_wheel(&mut self, colour:RGB8) {
        let scaled_position = self.animation_position_to_u8();
        for i in 0..self.led_count {
            let wheel_pos = (((i * 255 / self.led_count) as f32 * self.repetition_factor).round() as u32 + scaled_position as u32) as u8;
            self.led_state[i] = single_colour_wheel(colour, wheel_pos);
        }
    }

    fn pulse(&mut self, colour:RGB8) {
        let normalised_position = self.animation_position / Self::LED_SINGLE_ROTATION_STEPS;
        let brightness_factor = match normalised_position < 0.5 {
            true => normalised_position,
            false => 1.0 - normalised_position,
        } * 2.0;

        let current_colour:RGB8 = ((brightness_factor * colour.r as f32).round() as u8,
                                   (brightness_factor * colour.g as f32).round() as u8,
                                    (brightness_factor * colour.b as f32).round() as u8 ).into();

        self.led_state.fill(current_colour);
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
                            ProductState::Calibration(_) => {
                                led_control.set_mode(LedArrayMode::Pulse(RGB8::new(177, 3, 252)));
                                led_control.set_speed_factor(4.0);
                            }
                            ProductState::CalibrationDone => {
                                led_control.set_mode(LedArrayMode::SingleColourWheel(RGB8::new(0, 255, 0)));
                                led_control.set_repetition_factor(2.0);
                                led_control.set_speed_factor(1.0);
                            }
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