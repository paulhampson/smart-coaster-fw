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

use crate::application::application_state::ApplicationState;
use crate::hmi::messaging::{UiActionChannelPublisher, UiRequestMessage};
use crate::hmi::screens::{add_newlines_to_string, UiDrawer, UiInput, UiInputHandler};
use core::cmp::PartialEq;
use core::fmt::Write;
use embedded_graphics::mono_font::ascii::{FONT_6X10, FONT_6X13_BOLD, FONT_8X13};
use embedded_graphics::mono_font::MonoTextStyleBuilder;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::{DrawTarget, Point};
use embedded_graphics::text::renderer::TextRenderer;
use embedded_graphics::text::{Alignment, Baseline, Text, TextStyleBuilder};
use embedded_graphics::Drawable;
use heapless::String;

#[derive(PartialEq, Debug)]
enum Element {
    Confirm,
    Cancel,
}

impl Element {
    pub fn next_element(&self) -> Self {
        match self {
            Element::Confirm => Element::Cancel,
            Element::Cancel => Element::Confirm,
        }
    }

    pub fn previous_element(&self) -> Self {
        match self {
            Element::Cancel => Element::Confirm,
            Element::Confirm => Element::Cancel,
        }
    }
}

pub struct ConfirmationScreen {
    label: &'static str,
    message: &'static str,
    affirmative_message: UiRequestMessage,
    current_element: Element,
}

impl ConfirmationScreen {
    pub fn new(
        label: &'static str,
        message: &'static str,
        affirmative_message: UiRequestMessage,
    ) -> Self {
        Self {
            label,
            message,
            affirmative_message,
            current_element: Element::Cancel,
        }
    }
}

impl UiInputHandler for ConfirmationScreen {
    async fn ui_input_handler(
        &mut self,
        input: UiInput,
        ui_action_publisher: &UiActionChannelPublisher<'_>,
    ) {
        match input {
            UiInput::EncoderClockwise => {
                self.current_element = self.current_element.next_element();
            }
            UiInput::EncoderCounterClockwise => {
                self.current_element = self.current_element.previous_element();
            }
            UiInput::ButtonPress => match self.current_element {
                Element::Confirm => {
                    ui_action_publisher.publish(self.affirmative_message).await;
                    ui_action_publisher
                        .publish(UiRequestMessage::ChangeState(ApplicationState::Settings))
                        .await;
                }
                Element::Cancel => {
                    ui_action_publisher
                        .publish(UiRequestMessage::ChangeState(ApplicationState::Settings))
                        .await;
                }
            },
            _ => {}
        }
    }
}

impl UiDrawer for ConfirmationScreen {
    fn draw<D>(&self, display: &mut D) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = BinaryColor>,
    {
        let active_element_style = MonoTextStyleBuilder::new()
            .font(&FONT_8X13)
            .text_color(BinaryColor::Off)
            .background_color(BinaryColor::On)
            .build();

        let label_char_style = MonoTextStyleBuilder::new()
            .font(&FONT_6X13_BOLD)
            .text_color(BinaryColor::On)
            .build();
        let label_text_style = TextStyleBuilder::new()
            .alignment(Alignment::Center)
            .baseline(Baseline::Top)
            .build();

        let text_style = MonoTextStyleBuilder::new()
            .font(&FONT_6X10)
            .text_color(BinaryColor::On)
            .build();

        let mut string_buffer = String::<32>::new();
        let mut next_point = Point::new((display.bounding_box().size.width / 2) as i32, 0);

        string_buffer.clear();
        writeln!(&mut string_buffer, "{}", self.label).unwrap();
        Text::with_text_style(
            string_buffer.as_str(),
            next_point,
            label_char_style,
            label_text_style,
        )
        .draw(display)?;

        next_point = display.bounding_box().center();

        let max_line_length = (display.bounding_box().size.width as usize
            / text_style.font.character_size.width as usize)
            - 1;
        let formatted_message = add_newlines_to_string::<100>(self.message, max_line_length);
        let number_of_lines = formatted_message.lines().count() as i32;
        next_point.y -= (number_of_lines * text_style.line_height() as i32) / 2;

        Text::with_text_style(
            formatted_message.as_str(),
            next_point,
            text_style,
            TextStyleBuilder::new()
                .alignment(Alignment::Center)
                .baseline(Baseline::Middle)
                .build(),
        )
        .draw(display)?;

        next_point.x = 0;
        next_point.y = display.bounding_box().size.height as i32;
        let char_style_to_use = if self.current_element == Element::Confirm {
            active_element_style
        } else {
            text_style
        };
        string_buffer.clear();
        write!(&mut string_buffer, "[Yes]").unwrap();
        next_point = Text::with_text_style(
            string_buffer.as_str(),
            next_point,
            char_style_to_use,
            TextStyleBuilder::new()
                .alignment(Alignment::Left)
                .baseline(Baseline::Bottom)
                .build(),
        )
        .draw(display)?;

        next_point.x = display.bounding_box().size.width as i32;
        let char_style_to_use = if self.current_element == Element::Cancel {
            active_element_style
        } else {
            text_style
        };
        string_buffer.clear();
        writeln!(&mut string_buffer, "[No]").unwrap();
        Text::with_text_style(
            string_buffer.as_str(),
            next_point,
            char_style_to_use,
            TextStyleBuilder::new()
                .alignment(Alignment::Right)
                .baseline(Baseline::Bottom)
                .build(),
        )
        .draw(display)?;
        Ok(())
    }
}
