use crate::hmi::screens::{UiDrawer, UiInputHandler};

pub mod calibration;
pub mod heap_status;
pub mod set_date_time;
pub mod set_number;
pub mod test_mode;

trait SettingScreen: UiInputHandler + UiDrawer {}
