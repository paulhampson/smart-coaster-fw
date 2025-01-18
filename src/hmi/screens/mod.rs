use crate::application::messaging::ApplicationData;
use crate::hmi::messaging::UiActionChannelPublisher;
use sh1106::mode::GraphicsMode;

pub mod settings;
pub mod test_mode;

pub enum UiInput {
    EncoderClockwise,
    EncoderCounterClockwise,
    ButtonPress,
    ButtonRelease,
    ApplicationData(ApplicationData),
}

pub trait UiInputHandler {
    fn ui_input_handler(&mut self, input: UiInput, ui_action_publisher: &UiActionChannelPublisher);
}

pub trait UiDrawer {
    fn draw<DI>(&self, display: &mut GraphicsMode<DI>)
    where
        DI: sh1106::interface::DisplayInterface;
}
