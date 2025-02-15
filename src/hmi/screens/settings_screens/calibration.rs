use crate::application::application_state::CalibrationStateSubstates;
use crate::application::messaging::ApplicationData;
use crate::hmi::messaging::UiActionChannelPublisher;
use crate::hmi::screens::{draw_message_screen, UiDrawer, UiInput, UiInputHandler};
use core::fmt::Write;
use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::pixelcolor::BinaryColor;
use heapless::String;

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
    async fn ui_input_handler(
        &mut self,
        input: UiInput,
        _ui_action_publisher: &UiActionChannelPublisher<'_>,
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
    fn draw<D>(&self, display: &mut D) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = BinaryColor>,
    {
        match self.state {
            CalibrationStateSubstates::Tare => {
                draw_message_screen(display, "Remove items from device and press button")
            }
            CalibrationStateSubstates::Wait => draw_message_screen(display, "Please wait..."),
            CalibrationStateSubstates::Calibration(calibration_mass_grams) => {
                let mut message_string = String::<40>::new();
                write!(
                    message_string,
                    "Put {}g on device, then press button.",
                    calibration_mass_grams
                )
                .expect("String too long");
                draw_message_screen(display, &message_string)
            }
            CalibrationStateSubstates::CalibrationDone => {
                draw_message_screen(display, "Calibration complete")
            }
        }
    }
}
