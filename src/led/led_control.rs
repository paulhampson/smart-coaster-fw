// Copyright (C) 2025 Paul Hampson
//
// This program is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License version 3 as  published by the
// Free Software Foundation.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more
// details.
//
// You should have received a copy of the GNU General Public License along with
// this program.  If not, see <https://www.gnu.org/licenses/>.

use crate::storage::settings::{SettingValue, SettingsAccessor, SettingsAccessorId};
use defmt::{warn, Debug2Format};
use embassy_rp::pio::Instance;
use embassy_rp::pio_programs::ws2812::PioWs2812;
use embassy_time::Instant;
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
    Test {},
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

pub struct LedController<'a, const LED_COUNT: usize, P: Instance, const S: usize, SA>
where
    SA: SettingsAccessor,
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
    last_update: Instant,
    settings: SA,
}

impl<'a, const LED_COUNT: usize, P, const S: usize, SA> LedControl
    for LedController<'a, LED_COUNT, P, S, SA>
where
    P: Instance,
    SA: SettingsAccessor,
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
            LedArrayMode::Test {} => self.set_speed_factor(0.1),
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
        let _ = self
            .settings
            .save_setting(
                SettingsAccessorId::SystemLedBrightness,
                SettingValue::SmallUInt(brightness),
            )
            .await
            .map_err(|e| warn!("Failed to store LED brightness: {:?}", Debug2Format(&e)));
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
            LedArrayMode::Test {} => self.test(),
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

impl<'a, const LED_COUNT: usize, P, const S: usize, SA> LedController<'a, LED_COUNT, P, S, SA>
where
    P: Instance,
    SA: SettingsAccessor,
{
    const LED_SINGLE_ROTATION_STEPS: f32 = 360.0;

    pub async fn new(ws2812pio: PioWs2812<'a, P, S, LED_COUNT>, settings: SA) -> Self {
        let mut s = Self {
            ws2812pio,
            led_count: LED_COUNT,
            led_state: [RGB8::default(); LED_COUNT],
            array_mode: LedArrayMode::Off,
            animation_position: 0.0,
            speed_factor: 1.0,
            repetition_factor: 1.0,
            base_colour: RGB8::new(0, 0, 0),
            brightness: DEFAULT_BRIGHTNESS,
            last_update: Instant::now(),
            settings,
        };
        s.brightness = s.get_stored_brightness().await;
        s
    }

    async fn get_stored_brightness(&self) -> u8 {
        if let Some(result) = self
            .settings
            .get_setting(SettingsAccessorId::SystemLedBrightness)
            .await
        {
            return match result {
                SettingValue::SmallUInt(v) => v,
                _ => DEFAULT_BRIGHTNESS,
            };
        }
        DEFAULT_BRIGHTNESS
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
                (((((i * 256) / (self.led_count - 1)) as f32 * self.repetition_factor).round()
                    as u16
                    + scaled_position as u16)
                    & 255) as u8,
            );
        }
    }

    fn single_colour_wheel(&mut self) {
        let scaled_position = self.animation_position_to_u8();
        for i in 0..self.led_count {
            let wheel_pos = (((i * 255 / (self.led_count - 1)) as f32 * self.repetition_factor)
                .round() as u32
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

    fn test(&mut self) {
        let normalised_position = self.animation_position / Self::LED_SINGLE_ROTATION_STEPS;
        match normalised_position {
            0.0..0.3333333 => {
                self.led_state.fill(RGB8::new(0, 0, 0));
                let index_as_float = (LED_COUNT - 1) as f32 * normalised_position / 0.3333333;
                let index = index_as_float.round() as usize;
                self.led_state[index] = RGB8::new(255, 0, 0);
            }
            0.3333333..0.6666666 => {
                self.led_state.fill(RGB8::new(0, 0, 0));
                let index_as_float =
                    (LED_COUNT - 1) as f32 * (normalised_position - 0.3333333) / 0.3333333;
                let index = index_as_float.round() as usize;
                self.led_state[index] = RGB8::new(0, 255, 0);
            }
            0.6666666..1.0 => {
                self.led_state.fill(RGB8::new(0, 0, 0));
                let index_as_float =
                    (LED_COUNT - 1) as f32 * (normalised_position - 0.666666) / 0.3333333;
                let index = index_as_float.round() as usize;
                self.led_state[index] = RGB8::new(0, 0, 255);
            }
            _ => {}
        }
    }
}
