use crate::application::application_state::CalibrationStateSubstates;
use crate::application::messaging::ApplicationData;
use crate::hmi::messaging::UiActionChannelPublisher;
use crate::hmi::screens::{draw_message_screen, UiDrawer, UiInput, UiInputHandler};
use core::fmt::Write;
use heapless::String;
use sh1106::interface::DisplayInterface;
use sh1106::mode::GraphicsMode;

pub struct CalibrationScreens {
    state: CalibrationStateSubstates,
}

impl CalibrationScreens {
    pub fn new() -> Self {
        Self {
            state: CalibrationStateSubstates::Tare,
        }
    }
}

impl UiInputHandler for CalibrationScreens {
    fn ui_input_handler(
        &mut self,
        input: UiInput,
        _ui_action_publisher: &UiActionChannelPublisher,
    ) {
        match input {
            UiInput::ApplicationData(app_data) => match app_data {
                ApplicationData::CalibrationSubstate(new_state) => {
                    self.state = new_state;
                }
                _ => {}
            },
            _ => {}
        }
    }
}

impl UiDrawer for CalibrationScreens {
    fn draw<DI>(&self, display: &mut GraphicsMode<DI>)
    where
        DI: DisplayInterface,
    {
        match self.state {
            CalibrationStateSubstates::Tare => {
                draw_message_screen(display, "Remove items from device and press button")
            }
            CalibrationStateSubstates::Wait => {
                draw_message_screen(display, "Please wait...");
            }
            CalibrationStateSubstates::Calibration(calibration_mass_grams) => {
                let mut message_string = String::<40>::new();
                write!(
                    message_string,
                    "Put {}g on device, then press button.",
                    calibration_mass_grams
                )
                .expect("String too long");
                draw_message_screen(display, &message_string);
            }
            CalibrationStateSubstates::CalibrationDone => {
                draw_message_screen(display, "Calibration complete")
            }
        }
    }
}
