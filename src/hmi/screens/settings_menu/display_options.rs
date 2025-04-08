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

pub struct DisplayTimeoutOptions {}

impl DisplayTimeoutOptions {
    const OPTION_LIST: [u8; 5] = [5, 10, 15, 30, 60];
    pub const DEFAULT: u8 = 30;

    pub fn option_strings() -> &'static [&'static str] {
        &["5 min", "10 min", "15 min", "30 min", "60 min"]
    }

    pub fn option_index_to_minutes(index: usize) -> u8 {
        if index > Self::OPTION_LIST.len() {
            Self::DEFAULT
        } else {
            Self::OPTION_LIST[index]
        }
    }
    pub fn minutes_to_option_index(minutes: u8) -> usize {
        Self::OPTION_LIST
            .iter()
            .position(|&r| r == minutes)
            .unwrap_or(4)
    }
}

pub enum DisplayBrightnessOptions {
    Low,
    Medium,
    High,
}

impl DisplayBrightnessOptions {
    const LOW: u8 = 0;
    const MEDIUM: u8 = 128;
    const HIGH: u8 = 255;

    pub fn option_strings() -> &'static [&'static str] {
        &["Low", "Medium", "High"]
    }

    pub fn brightness_mapping(&self) -> u8 {
        match self {
            DisplayBrightnessOptions::Low => Self::LOW,
            DisplayBrightnessOptions::Medium => Self::MEDIUM,
            DisplayBrightnessOptions::High => Self::HIGH,
        }
    }

    pub fn option_index_to_brightness(index: usize) -> u8 {
        match index {
            0 => Self::LOW,
            1 => Self::MEDIUM,
            2 => Self::HIGH,
            _ => 0,
        }
    }

    pub fn to_option_index(&self) -> usize {
        match self {
            DisplayBrightnessOptions::Low => 0,
            DisplayBrightnessOptions::Medium => 1,
            DisplayBrightnessOptions::High => 2,
        }
    }
}

impl From<u8> for DisplayBrightnessOptions {
    fn from(value: u8) -> Self {
        match value {
            0..=90 => Self::Low,
            91..=150 => Self::Medium,
            151..=u8::MAX => Self::High,
        }
    }
}
