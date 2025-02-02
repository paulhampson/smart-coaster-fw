use crate::application::application_state::{ApplicationState, MonitoringStateSubstates};
use crate::application::messaging::ApplicationData;
use crate::hmi::messaging::{UiActionChannelPublisher, UiActionsMessage};
use crate::hmi::screens::{draw_message_screen, UiDrawer, UiInput, UiInputHandler};
use core::fmt::Write;
use embedded_graphics::geometry::Point;
use embedded_graphics::mono_font::ascii::FONT_6X10;
use embedded_graphics::mono_font::MonoTextStyleBuilder;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::text::renderer::TextRenderer;
use embedded_graphics::text::{Alignment, Baseline, Text, TextStyleBuilder};
use embedded_graphics::Drawable;
use heapless::String;
use micromath::F32Ext;
use sh1106::interface::DisplayInterface;
use sh1106::mode::GraphicsMode;

pub struct MonitoringScreen {
    consumption: f32,
    consumption_rate: f32,
    total_consumed: f32,
    state: MonitoringStateSubstates,
}

impl MonitoringScreen {
    pub fn new() -> Self {
        Self {
            consumption: 0.0,
            consumption_rate: 0.0,
            total_consumed: 0.0,
            state: MonitoringStateSubstates::WaitingForActivity,
        }
    }

    fn process_application_data(&mut self, data: ApplicationData) {
        match data {
            ApplicationData::Consumption(new_consumption) => {
                self.consumption = new_consumption;
            }
            ApplicationData::ConsumptionRate(new_consumption_rate) => {
                self.consumption_rate = new_consumption_rate;
            }
            ApplicationData::TotalConsumed(new_total_consumed) => {
                self.total_consumed = new_total_consumed;
            }
            ApplicationData::MonitoringSubstate(new_state) => {
                self.state = new_state;
            }
            _ => {}
        }
    }
}

impl UiInputHandler for MonitoringScreen {
    fn ui_input_handler(&mut self, input: UiInput, ui_action_publisher: &UiActionChannelPublisher) {
        match input {
            UiInput::EncoderClockwise => {}
            UiInput::EncoderCounterClockwise => {}
            UiInput::ButtonPress => ui_action_publisher.publish_immediate(
                UiActionsMessage::StateChangeRequest(ApplicationState::Settings),
            ),
            UiInput::ButtonRelease => {}
            UiInput::ApplicationData(data) => self.process_application_data(data),
            UiInput::DateTimeUpdate(_) => {}
        }
    }
}

impl UiDrawer for MonitoringScreen {
    fn draw<DI>(&self, display: &mut GraphicsMode<DI>)
    where
        DI: DisplayInterface,
    {
        match self.state {
            MonitoringStateSubstates::WaitingForActivity => {
                draw_message_screen(display, "Waiting for activity");
            }
            MonitoringStateSubstates::VesselRemoved | MonitoringStateSubstates::VesselPlaced => {
                let mut string_buffer = String::<100>::new();
                let text_style = MonoTextStyleBuilder::new()
                    .font(&FONT_6X10)
                    .text_color(BinaryColor::On)
                    .build();
                let centred_text_style = TextStyleBuilder::new()
                    .alignment(Alignment::Center)
                    .baseline(Baseline::Middle)
                    .build();

                let central_x_pos = display.get_dimensions().0 as i32 / 2;
                let central_y_pos = display.get_dimensions().1 as i32 / 2;
                let target_y_pos =
                    central_y_pos - (2f32 * text_style.line_height() as f32).round() as i32;
                let centre_point = Point::new(central_x_pos, target_y_pos);

                match self.state {
                    MonitoringStateSubstates::VesselPlaced => {
                        write!(string_buffer, "Vessel placed\n").unwrap();
                    }
                    MonitoringStateSubstates::VesselRemoved => {
                        write!(string_buffer, "Vessel removed\n").unwrap();
                    }
                    _ => {}
                };
                writeln!(string_buffer, "Rate: {:.0} ml/hr", self.consumption_rate).unwrap();
                writeln!(string_buffer, "Last drink: {:.0} ml", self.consumption).unwrap();
                write!(string_buffer, "Total: {:.0} ml", self.total_consumed).unwrap();
                Text::with_text_style(
                    string_buffer.as_str(),
                    centre_point,
                    text_style,
                    centred_text_style,
                )
                .draw(display)
                .unwrap();
            }
        }
    }
}
