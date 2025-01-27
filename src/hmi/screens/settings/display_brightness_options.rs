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
