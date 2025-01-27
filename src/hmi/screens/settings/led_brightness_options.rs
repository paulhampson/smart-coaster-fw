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

    pub fn brightness_mapping(&self) -> u8 {
        match self {
            LedBrightnessOptions::Off => Self::OFF,
            LedBrightnessOptions::Low => Self::LOW,
            LedBrightnessOptions::Medium => Self::MEDIUM,
            LedBrightnessOptions::High => Self::HIGH,
            LedBrightnessOptions::Max => Self::MAX,
        }
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
