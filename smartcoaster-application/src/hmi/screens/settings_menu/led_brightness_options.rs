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

pub enum LedBrightnessOptions {
    Off,
    Low,
    Medium,
    High,
    Max,
}

impl LedBrightnessOptions {
    const OFF: u8 = 0;
    const LOW: u8 = 75;
    const MEDIUM: u8 = 128;
    const HIGH: u8 = 200;
    const MAX: u8 = 255;

    pub fn option_strings() -> &'static [&'static str] {
        &["Off", "Low", "Medium", "High", "Max"]
    }

    pub fn option_index_to_brightness(index: usize) -> u8 {
        match index {
            0 => Self::OFF,
            1 => Self::LOW,
            2 => Self::MEDIUM,
            3 => Self::HIGH,
            4 => Self::MAX,
            _ => 0,
        }
    }

    pub fn to_option_index(&self) -> usize {
        match self {
            LedBrightnessOptions::Off => 0,
            LedBrightnessOptions::Low => 1,
            LedBrightnessOptions::Medium => 2,
            LedBrightnessOptions::High => 3,
            LedBrightnessOptions::Max => 4,
        }
    }
}

impl From<u8> for LedBrightnessOptions {
    fn from(value: u8) -> Self {
        match value {
            0..=40 => Self::Off,
            41..=90 => Self::Low,
            91..=150 => Self::Medium,
            151..=254 => Self::High,
            u8::MAX => Self::Max,
        }
    }
}
