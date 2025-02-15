mod top_status_bar;

use crate::application::application_state::{ApplicationState, MonitoringStateSubstates};
use crate::application::messaging::ApplicationData;
use crate::hmi::messaging::{UiActionChannelPublisher, UiActionsMessage};
use crate::hmi::screens::monitoring::top_status_bar::TopStatusBar;
use crate::hmi::screens::{draw_message_screen, UiDrawer, UiInput, UiInputHandler};
use chrono::NaiveDateTime;
use core::fmt::Write;
use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::geometry::{AnchorX, AnchorY, Point};
use embedded_graphics::image::Image;
use embedded_graphics::mono_font::ascii::{FONT_5X7, FONT_6X10, FONT_9X15_BOLD};
use embedded_graphics::mono_font::MonoTextStyleBuilder;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::{Dimensions, DrawTargetExt, OriginDimensions, Primitive, Size};
use embedded_graphics::primitives::{PrimitiveStyleBuilder, Rectangle};
use embedded_graphics::text::renderer::TextRenderer;
use embedded_graphics::text::{Alignment, Baseline, Text, TextStyleBuilder};
use embedded_graphics::Drawable;
use embedded_icon::NewIcon;
use heapless::String;
use micromath::F32Ext;

pub struct MonitoringScreen {
    consumption: f32,
    consumption_rate: f32,
    total_consumed: f32,
    state: MonitoringStateSubstates,

    datetime: NaiveDateTime,
}

impl MonitoringScreen {
    pub fn new() -> Self {
        Self {
            consumption: 0.0,
            consumption_rate: 0.0,
            total_consumed: 0.0,
            state: MonitoringStateSubstates::WaitingForActivity,
            datetime: NaiveDateTime::default(),
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
    async fn ui_input_handler(
        &mut self,
        input: UiInput,
        ui_action_publisher: &UiActionChannelPublisher<'_>,
    ) {
        match input {
            UiInput::EncoderClockwise => {}
            UiInput::EncoderCounterClockwise => {}
            UiInput::ButtonPress => ui_action_publisher.publish_immediate(
                UiActionsMessage::StateChangeRequest(ApplicationState::Settings),
            ),
            UiInput::ButtonRelease => {}
            UiInput::ApplicationData(data) => self.process_application_data(data),
            UiInput::DateTimeUpdate(dt) => self.datetime = dt,
        }
    }
}

impl MonitoringScreen {
    fn draw_simple_layout<D>(&self, display: &mut D) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = BinaryColor>,
    {
        match self.state {
            MonitoringStateSubstates::WaitingForActivity => {
                draw_message_screen(display, "Waiting for activity")?;
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

                let central_x_pos = display.bounding_box().size.width as i32 / 2;
                let central_y_pos = display.bounding_box().size.height as i32 / 2;
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
                .draw(display)?;
            }
        }
        Ok(())
    }

    fn draw_layout1<D>(&self, display: &mut D) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = BinaryColor>,
    {
        let status_bar_height = 10;
        let mut status_bar_display_area = display.cropped(
            &display
                .bounding_box()
                .resized_height(status_bar_height, AnchorY::Top),
        );

        let status_bar =
            TopStatusBar::new(self.datetime, Point::zero(), status_bar_display_area.size());
        status_bar.draw(&mut status_bar_display_area)?;

        let mut content_display = display.cropped(&display.bounding_box().resized_height(
            display.bounding_box().size.height - status_bar_height,
            AnchorY::Bottom,
        ));

        match self.state {
            MonitoringStateSubstates::WaitingForActivity => {
                self.draw_waiting_content(&mut content_display)?
            }
            MonitoringStateSubstates::VesselRemoved | MonitoringStateSubstates::VesselPlaced => {
                self.draw_active_content(&mut content_display)?
            }
        }
        Ok(())
    }

    fn draw_waiting_content<D>(&self, display: &mut D) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = BinaryColor>,
    {
        let message_height = 20;
        let content_height = display.bounding_box().size.height - message_height;

        let mut icon_display = display.cropped(
            &display
                .bounding_box()
                .resized_height(content_height, AnchorY::Top),
        );

        let mut icon_location = icon_display.bounding_box().center();
        let icon = embedded_icon::mdi::size32px::BeakerQuestion::new(BinaryColor::On);
        icon_location.x -= (icon.size().width / 2) as i32;
        icon_location.y -= (icon.size().height / 2) as i32;
        Image::new(&icon, icon_location)
            .draw(&mut icon_display)
            .ok();

        let mut message_display_area = display.cropped(
            &display
                .bounding_box()
                .resized_height(message_height, AnchorY::Bottom),
        );

        draw_message_screen(&mut message_display_area, "Waiting for activity")?;
        Ok(())
    }

    fn draw_active_content<D>(&self, display: &mut D) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = BinaryColor>,
    {
        let bottom_bar_height = 0;
        let mut main_area_display = display.cropped(&display.bounding_box().resized_height(
            display.bounding_box().size.height - bottom_bar_height,
            AnchorY::Top,
        ));

        let left_icon_area_width = main_area_display.bounding_box().size.width / 3;
        let mut left_icon_display = main_area_display.cropped(
            &main_area_display
                .bounding_box()
                .resized_width(left_icon_area_width, AnchorX::Left),
        );

        let icon = embedded_icon::mdi::size32px::Cup::new(BinaryColor::On);
        if self.state == MonitoringStateSubstates::VesselPlaced {
            let mut icon_location = left_icon_display.bounding_box().center();
            icon_location.x -= (icon.size().width / 2) as i32;
            icon_location.y -= (icon.size().height / 2) as i32;
            Image::new(&icon, icon_location).draw(&mut left_icon_display)?;
        }

        let icon_base_height = 3;
        let icon_base_space_from_edge = 5;
        let padding_from_bottom_of_icon = 0;
        let icon_base_origin = Point::new(
            icon_base_space_from_edge,
            left_icon_display.size().height as i32 / 2
                + icon.size().height as i32 / 2
                + padding_from_bottom_of_icon,
        );
        let base_style = PrimitiveStyleBuilder::new()
            .fill_color(BinaryColor::On)
            .build();
        Rectangle::new(
            icon_base_origin,
            Size::new(
                left_icon_area_width - (icon_base_space_from_edge * 2) as u32,
                icon_base_height as u32,
            ),
        )
        .into_styled(base_style)
        .draw(&mut left_icon_display)?;

        let mut right_display_area =
            main_area_display.cropped(&main_area_display.bounding_box().resized_width(
                main_area_display.bounding_box().size.width - left_icon_area_width,
                AnchorX::Right,
            ));

        let value_char_style = MonoTextStyleBuilder::new()
            .font(&FONT_9X15_BOLD)
            .text_color(BinaryColor::On)
            .build();
        let unit_char_style = MonoTextStyleBuilder::new()
            .font(&FONT_5X7)
            .text_color(BinaryColor::On)
            .build();
        let text_style = TextStyleBuilder::new()
            .alignment(Alignment::Left)
            .baseline(Baseline::Bottom)
            .build();
        let line_padding = value_char_style.line_height() as i32 / 4;

        let mut string_buffer = String::<10>::new();
        write!(string_buffer, "{:4.0}", self.consumption_rate).unwrap();
        let mut pos = right_display_area.bounding_box().center();
        pos.x = 0;
        pos.y -= line_padding / 2;
        let mut next_pos =
            Text::with_text_style(string_buffer.as_str(), pos, value_char_style, text_style)
                .draw(&mut right_display_area)?;

        next_pos.x += unit_char_style.font.character_size.width as i32;
        next_pos.y -= 2; // expected baseline to manage this, but doesn't seem to for different font sizes.

        string_buffer.clear();
        write!(string_buffer, "ml/hr").unwrap();
        next_pos = Text::with_text_style(
            string_buffer.as_str(),
            next_pos,
            unit_char_style,
            text_style,
        )
        .draw(&mut right_display_area)?;

        next_pos.y += value_char_style.line_height() as i32 + line_padding;
        next_pos.x = 0;

        string_buffer.clear();
        write!(string_buffer, "{:4.0}", self.total_consumed).unwrap();
        let mut next_pos = Text::with_text_style(
            string_buffer.as_str(),
            next_pos,
            value_char_style,
            text_style,
        )
        .draw(&mut right_display_area)?;

        next_pos.x += unit_char_style.font.character_size.width as i32;
        next_pos.y -= 2; // expected text baseline to manage this, but doesn't seem to for different font sizes

        string_buffer.clear();
        write!(string_buffer, "ml today").unwrap();
        Text::with_text_style(
            string_buffer.as_str(),
            next_pos,
            unit_char_style,
            text_style,
        )
        .draw(&mut right_display_area)?;

        Ok(())
    }
}

impl UiDrawer for MonitoringScreen {
    fn draw<D>(&self, display: &mut D) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = BinaryColor>,
    {
        self.draw_layout1(display)?;
        Ok(())
    }
}
