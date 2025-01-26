use crate::application::storage::settings::SettingsManagerMutex;
use defmt::{trace, warn};
use embassy_rp::pio::Instance;
use embassy_rp::pio_programs::ws2812::PioWs2812;
use embassy_time::{Duration, Instant, Timer};
use embedded_storage_async::nor_flash::MultiwriteNorFlash;
use micromath::F32Ext;
use smart_leds::hsv::{hsv2rgb, Hsv};
use smart_leds::{brightness, gamma, RGB8};

const DEFAULT_BRIGHTNESS: u8 = 128;

pub enum LedArrayMode {
    Off,
    RainbowWheel {
        speed: f32,
        repetitions: f32,
    },
    SingleColourWheel {
        colour: RGB8,
        speed: f32,
        repetitions: f32,
    },
    Pulse {
        colour: RGB8,
        speed: f32,
    },
    StaticColour {
        colour: RGB8,
    },
}

/// Input a position 0 to 255 to get a color value associated with that position.
fn rainbow_wheel(wheel_pos: u8) -> RGB8 {
    hsv2rgb(Hsv {
        hue: wheel_pos,
        sat: 255,
        val: 255,
    })
}

/// Input a position in range 0-255 to get the brightness of the colour associated with that
/// position.
fn single_colour_wheel(colour: RGB8, wheel_pos: u8) -> RGB8 {
    let intensity = match wheel_pos < 128 {
        true => 255 - (wheel_pos * 2),
        false => (wheel_pos - 128) * 2,
    } as u32;

    (
        (intensity * colour.r as u32 / 255) as u8,
        (intensity * colour.g as u32 / 255) as u8,
        (intensity * colour.b as u32 / 255) as u8,
    )
        .into()
}

pub trait LedControl {
    fn set_mode(&mut self, mode: LedArrayMode);
    fn set_speed_factor(&mut self, speed: f32);
    fn set_repetition_factor(&mut self, repetition_factor: f32);
    async fn set_brightness(&mut self, brightness: u8);
    async fn led_update(&mut self);
}

pub struct LedController<'a, const LED_COUNT: usize, P: Instance, const S: usize, E, F>
where
    E: defmt::Format,
    F: MultiwriteNorFlash<Error = E>,
{
    ws2812pio: PioWs2812<'a, P, S, LED_COUNT>,
    led_count: usize,
    led_state: [RGB8; LED_COUNT],
    array_mode: LedArrayMode,
    animation_position: f32,
    speed_factor: f32,
    repetition_factor: f32,
    base_colour: RGB8,
    brightness: u8,
    shared_settings: &'a SettingsManagerMutex<E, F>,
    last_update: Instant,
}

impl<'a, const LED_COUNT: usize, P, const S: usize, E, F> LedControl
    for LedController<'a, LED_COUNT, P, S, E, F>
where
    P: Instance,
    E: defmt::Format,
    F: MultiwriteNorFlash<Error = E>,
{
    fn set_mode(&mut self, mode: LedArrayMode) {
        self.array_mode = mode;
        match self.array_mode {
            LedArrayMode::RainbowWheel { speed, repetitions } => {
                self.set_speed_factor(speed);
                self.set_repetition_factor(repetitions);
            }
            LedArrayMode::SingleColourWheel {
                colour,
                speed,
                repetitions,
            } => {
                self.set_speed_factor(speed);
                self.set_repetition_factor(repetitions);
                self.base_colour = colour;
            }
            LedArrayMode::Pulse { colour, speed } => {
                self.set_speed_factor(speed);
                self.base_colour = colour;
            }
            LedArrayMode::StaticColour { colour } => {
                self.base_colour = colour;
            }
            LedArrayMode::Off => self.base_colour = RGB8::new(0, 0, 0),
        }
    }

    /// Speed is in rotations per second
    fn set_speed_factor(&mut self, speed: f32) {
        self.speed_factor = speed;
    }

    fn set_repetition_factor(&mut self, repetition_factor: f32) {
        self.repetition_factor = repetition_factor;
    }

    async fn set_brightness(&mut self, brightness: u8) {
        self.brightness = brightness;
        let mut settings = self.shared_settings.lock().await;
        let _ = settings
            .set_system_led_brightness(brightness)
            .await
            .map_err(|e| warn!("Failed to store LED brightness: {:?}", e));
    }

    async fn led_update(&mut self) {
        match self.array_mode {
            LedArrayMode::Off => self.off(),
            LedArrayMode::RainbowWheel {
                speed: _,
                repetitions: _,
            } => self.rainbow_wheel(),
            LedArrayMode::SingleColourWheel {
                colour: _,
                speed: _,
                repetitions: _,
            } => self.single_colour_wheel(),
            LedArrayMode::Pulse {
                colour: _,
                speed: _,
            } => self.pulse(),
            LedArrayMode::StaticColour { colour: _ } => self.static_colour(),
        };

        let mut scaled_leds = [RGB8::default(); LED_COUNT];
        for (idx, led) in brightness(self.led_state.iter().cloned(), self.brightness).enumerate() {
            scaled_leds[idx] = led;
        }
        let mut final_leds = [RGB8::default(); LED_COUNT];
        for (idx, led) in gamma(scaled_leds.iter().cloned()).enumerate() {
            final_leds[idx] = led;
        }

        self.ws2812pio.write(&final_leds).await;

        let degrees_per_sec = self.speed_factor * 360.0;
        let degrees_to_progress =
            (self.last_update.elapsed().as_millis() as f32 / 1000.0) * degrees_per_sec;
        self.last_update = Instant::now();
        self.animation_position = match self.animation_position {
            0.0..360.0 => self.animation_position + degrees_to_progress,
            _ => 0.0,
        };
    }
}

impl<'a, const LED_COUNT: usize, P, const S: usize, E, F> LedController<'a, LED_COUNT, P, S, E, F>
where
    P: Instance,
    E: defmt::Format,
    F: MultiwriteNorFlash<Error = E>,
{
    const LED_SINGLE_ROTATION_STEPS: f32 = 360.0;

    pub async fn new(
        ws2812pio: PioWs2812<'a, P, S, LED_COUNT>,
        shared_settings: &'static SettingsManagerMutex<E, F>,
    ) -> Self {
        Self {
            ws2812pio,
            led_count: LED_COUNT,
            led_state: [RGB8::default(); LED_COUNT],
            array_mode: LedArrayMode::Off,
            animation_position: 0.0,
            speed_factor: 1.0,
            repetition_factor: 1.0,
            base_colour: RGB8::new(0, 0, 0),
            brightness: Self::get_stored_brightness(shared_settings).await,
            shared_settings,
            last_update: Instant::now(),
        }
    }

    async fn get_stored_brightness(shared_settings: &'static SettingsManagerMutex<E, F>) -> u8 {
        let brightness;
        loop {
            {
                let settings = shared_settings.lock().await;
                if settings.is_initialized() {
                    brightness = settings
                        .get_system_led_brightness()
                        .await
                        .unwrap_or(DEFAULT_BRIGHTNESS);
                    trace!("Loaded LED brightness: {}", brightness,);
                    return brightness;
                }
            }
            Timer::after(Duration::from_millis(200)).await;
        }
    }

    fn animation_position_to_u8(&self) -> u8 {
        (self.animation_position * u8::MAX as f32 / Self::LED_SINGLE_ROTATION_STEPS) as u8
    }

    fn off(&mut self) {
        for i in 0..self.led_count {
            self.led_state[i] = RGB8::new(0, 0, 0);
        }
    }

    fn rainbow_wheel(&mut self) {
        let scaled_position = self.animation_position_to_u8();

        for i in 0..self.led_count {
            self.led_state[i] = rainbow_wheel(
                (((((i * 256) / self.led_count) as f32 * self.repetition_factor).round() as u16
                    + scaled_position as u16)
                    & 255) as u8,
            );
        }
    }

    fn single_colour_wheel(&mut self) {
        let scaled_position = self.animation_position_to_u8();
        for i in 0..self.led_count {
            let wheel_pos = (((i * 255 / self.led_count) as f32 * self.repetition_factor).round()
                as u32
                + scaled_position as u32) as u8;
            self.led_state[i] = single_colour_wheel(self.base_colour, wheel_pos);
        }
    }

    fn pulse(&mut self) {
        let normalised_position = self.animation_position / Self::LED_SINGLE_ROTATION_STEPS;
        let brightness_factor = match normalised_position < 0.5 {
            true => normalised_position,
            false => 1.0 - normalised_position,
        } * 2.0;

        let current_colour: RGB8 = (
            (brightness_factor * self.base_colour.r as f32).round() as u8,
            (brightness_factor * self.base_colour.g as f32).round() as u8,
            (brightness_factor * self.base_colour.b as f32).round() as u8,
        )
            .into();

        self.led_state.fill(current_colour);
    }

    fn static_colour(&mut self) {
        for i in 0..self.led_count {
            self.led_state[i] = self.base_colour;
        }
    }
}
