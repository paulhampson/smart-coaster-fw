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
use crate::hmi::screens::{UiDrawer, UiInput, UiInputHandler};
use crate::storage::settings::accessor::FlashSettingsAccessor;
use crate::storage::settings::{SettingValue, SettingsAccessor, SettingsAccessorId};
use chrono::{NaiveTime, TimeDelta, Timelike};
use core::cmp::PartialEq;
use core::fmt::Write;
use defmt::error;
use defmt::Debug2Format;
use embedded_graphics::mono_font::ascii::{FONT_6X13_BOLD, FONT_8X13};
use embedded_graphics::mono_font::MonoTextStyleBuilder;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::{DrawTarget, Point};
use embedded_graphics::text::renderer::TextRenderer;
use embedded_graphics::text::{Alignment, Baseline, Text, TextStyleBuilder};
use embedded_graphics::Drawable;
use heapless::String;

#[derive(PartialEq, Debug)]
enum Element {
    Hour,
    Minute,
    Save,
    Cancel,
}

impl Element {
    pub fn next_element(&self) -> Self {
        match self {
            Element::Hour => Element::Minute,
            Element::Minute => Element::Save,
            Element::Save => Element::Cancel,
            Element::Cancel => Element::Hour,
        }
    }

    pub fn previous_element(&self) -> Self {
        match self {
            Element::Hour => Element::Cancel,
            Element::Minute => Element::Hour,
            Element::Cancel => Element::Save,
            Element::Save => Element::Minute,
        }
    }
}

pub struct SetTimeScreen {
    label: &'static str,
    value: NaiveTime,
    setting_id_to_save: SettingsAccessorId,
    current_element: Element,
    element_active: bool,
}

impl SetTimeScreen {
    pub fn new(
        label: &'static str,
        value: NaiveTime,
        setting_id_to_save: SettingsAccessorId,
    ) -> Self {
        Self {
            label,
            value,
            setting_id_to_save,
            current_element: Element::Hour,
            element_active: false,
        }
    }

    fn increase_value(&mut self) {
        match self.current_element {
            Element::Hour => {
                (self.value, _) = self.value.overflowing_add_signed(TimeDelta::hours(1));
            }
            Element::Minute => {
                (self.value, _) = self.value.overflowing_add_signed(TimeDelta::minutes(1));
            }
            _ => {}
        }
    }

    fn decrease_value(&mut self) {
        match self.current_element {
            Element::Hour => {
                (self.value, _) = self.value.overflowing_sub_signed(TimeDelta::hours(1));
            }
            Element::Minute => {
                (self.value, _) = self.value.overflowing_sub_signed(TimeDelta::minutes(1));
            }
            _ => {}
        }
    }
}

impl UiInputHandler for SetTimeScreen {
    async fn ui_input_handler(
        &mut self,
        input: UiInput,
        ui_action_publisher: &UiActionChannelPublisher<'_>,
    ) {
        match input {
            UiInput::EncoderClockwise => {
                if self.element_active {
                    self.increase_value();
                } else {
                    self.current_element = self.current_element.next_element();
                }
            }
            UiInput::EncoderCounterClockwise => {
                if self.element_active {
                    self.decrease_value();
                } else {
                    self.current_element = self.current_element.previous_element();
                }
            }
            UiInput::ButtonPress => match self.current_element {
                Element::Save => {
                    let settings_accessor = FlashSettingsAccessor::new();
                    settings_accessor
                        .save_setting(self.setting_id_to_save, SettingValue::Time(self.value))
                        .await
                        .unwrap_or_else(|e| {
                            error!("Failed to save setting value - {}", Debug2Format(&e))
                        });
                    ui_action_publisher.publish_immediate(UiRequestMessage::ChangeState(
                        ApplicationState::Settings,
                    ));
                }
                Element::Cancel => {
                    ui_action_publisher.publish_immediate(UiRequestMessage::ChangeState(
                        ApplicationState::Settings,
                    ));
                }
                _ => {
                    self.element_active = !self.element_active;
                }
            },
            _ => {}
        }
    }
}

impl UiDrawer for SetTimeScreen {
    fn draw<D>(&self, display: &mut D) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = BinaryColor>,
    {
        let active_element_style = MonoTextStyleBuilder::new()
            .font(&FONT_8X13)
            .text_color(BinaryColor::Off)
            .background_color(BinaryColor::On)
            .build();
        let hover_element_style = MonoTextStyleBuilder::new()
            .font(&FONT_8X13)
            .text_color(BinaryColor::On)
            .background_color(BinaryColor::Off)
            .underline()
            .build();

        let text_style = MonoTextStyleBuilder::new()
            .font(&FONT_8X13)
            .text_color(BinaryColor::On)
            .build();

        let label_char_style = MonoTextStyleBuilder::new()
            .font(&FONT_6X13_BOLD)
            .text_color(BinaryColor::On)
            .build();
        let label_text_style = TextStyleBuilder::new()
            .alignment(Alignment::Center)
            .baseline(Baseline::Top)
            .build();

        let mut string_buffer = String::<32>::new();

        // label
        let mut next_point = Point::new((display.bounding_box().size.width / 2) as i32, 0);
        string_buffer.clear();
        writeln!(&mut string_buffer, "{}", self.label).unwrap();
        next_point = Text::with_text_style(
            string_buffer.as_str(),
            next_point,
            label_char_style,
            label_text_style,
        )
        .draw(display)?;

        // HH:MM
        let setting_character_count = 5;
        let setting_text_width =
            setting_character_count * text_style.font.character_size.width as i32;
        let x_offset = display.bounding_box().center().x - (setting_text_width / 2);

        next_point.x = x_offset;
        next_point.y = (display.bounding_box().center().y) - text_style.line_height() as i32 / 2;

        string_buffer.clear();
        write!(&mut string_buffer, "{:02}", self.value.hour()).unwrap();

        let style_to_use = if let Element::Hour = self.current_element {
            if self.element_active {
                active_element_style
            } else {
                hover_element_style
            }
        } else {
            text_style
        };
        Text::with_baseline(
            string_buffer.as_str(),
            next_point,
            style_to_use,
            Baseline::Top,
        )
        .draw(display)?;

        next_point.x += text_style.font.character_size.width as i32 * 2;
        Text::with_baseline(":", next_point, text_style, Baseline::Top).draw(display)?;

        next_point.x += text_style.font.character_size.width as i32;
        string_buffer.clear();
        write!(&mut string_buffer, "{:02}", self.value.minute()).unwrap();

        let style_to_use = if let Element::Minute = self.current_element {
            if self.element_active {
                active_element_style
            } else {
                hover_element_style
            }
        } else {
            text_style
        };
        Text::with_baseline(
            string_buffer.as_str(),
            next_point,
            style_to_use,
            Baseline::Top,
        )
        .draw(display)?;

        // save & cancel buttons
        next_point.x = 0;
        next_point.y = display.bounding_box().size.height as i32;
        let char_style_to_use = if self.current_element == Element::Save {
            active_element_style
        } else {
            text_style
        };
        string_buffer.clear();
        write!(&mut string_buffer, "[Save]").unwrap();
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
        writeln!(&mut string_buffer, "[Cancel]").unwrap();
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
