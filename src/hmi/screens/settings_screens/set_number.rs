use crate::application::application_state::ApplicationState;
use crate::application::storage::settings::accessor::FlashSettingsAccessor;
use crate::application::storage::settings::{SettingValue, SettingsAccessor, SettingsAccessorId};
use crate::hmi::messaging::{UiActionChannelPublisher, UiActionsMessage};
use crate::hmi::screens::settings_screens::SettingScreen;
use crate::hmi::screens::{UiDrawer, UiInput, UiInputHandler};
use core::cmp::{max, min, PartialEq};
use core::fmt::Write;
use defmt::error;
use defmt::Debug2Format;
use embedded_graphics::mono_font::ascii::{FONT_6X10, FONT_6X13_BOLD};
use embedded_graphics::mono_font::MonoTextStyleBuilder;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::{OriginDimensions, Point};
use embedded_graphics::text::renderer::TextRenderer;
use embedded_graphics::text::{Alignment, Baseline, Text, TextStyleBuilder};
use embedded_graphics::Drawable;
use embedded_layout::View;
use heapless::String;
use sh1106::interface::DisplayInterface;
use sh1106::mode::GraphicsMode;

#[derive(PartialEq, Debug)]
enum Element {
    NumberEntryPosition(usize),
    Save,
    Cancel,
}

impl Element {
    pub fn next_element(&self, max_elements: usize) -> Self {
        match self {
            Element::NumberEntryPosition(position) => {
                let next_position = position + 1;
                if next_position >= max_elements {
                    Element::Save
                } else {
                    Element::NumberEntryPosition(next_position)
                }
            }
            Element::Save => Element::Cancel,
            Element::Cancel => Element::NumberEntryPosition(0),
        }
    }

    pub fn previous_element(&self, max_elements: usize) -> Self {
        match self {
            Element::NumberEntryPosition(position) => {
                if *position == 0 {
                    Element::Cancel
                } else {
                    Element::NumberEntryPosition(position - 1)
                }
            }
            Element::Cancel => Element::Save,
            Element::Save => Element::NumberEntryPosition(max_elements - 1),
        }
    }
}

pub struct SetNumberScreen {
    label: &'static str,
    units: &'static str,
    value: u32,
    max: u32,
    min: u32,
    setting_id_to_save: SettingsAccessorId,
    num_elements: usize,
    current_element: Element,
    element_active: bool,
}

impl SetNumberScreen {
    pub fn new(
        label: &'static str,
        units: &'static str,
        value: u32,
        min: u32,
        max: u32,
        setting_id_to_save: SettingsAccessorId,
    ) -> Self {
        let num_elements = (max.ilog10() + 1) as usize;
        Self {
            label,
            units,
            value,
            max,
            min,
            setting_id_to_save,
            num_elements,
            current_element: Element::NumberEntryPosition(0),
            element_active: false,
        }
    }

    fn increase_value(&mut self, element: usize) {
        let position = self.num_elements - element - 1;
        let new_value = self.value + 10u32.pow(position as u32);
        self.value = min(new_value, self.max);
    }

    fn decrease_value(&mut self, element: usize) {
        let position = self.num_elements - element - 1;
        let mut new_value = self.value - 10u32.pow(position as u32);
        if new_value > self.max {
            new_value = 0;
        }
        self.value = max(new_value, self.min);
    }
}

impl SettingScreen for SetNumberScreen {}

impl UiInputHandler for SetNumberScreen {
    async fn ui_input_handler(
        &mut self,
        input: UiInput,
        ui_action_publisher: &UiActionChannelPublisher<'_>,
    ) {
        match input {
            UiInput::EncoderClockwise => {
                if self.element_active {
                    if let Element::NumberEntryPosition(position) = self.current_element {
                        self.increase_value(position);
                    }
                } else {
                    self.current_element = self.current_element.next_element(self.num_elements);
                }
            }
            UiInput::EncoderCounterClockwise => {
                if self.element_active {
                    if let Element::NumberEntryPosition(position) = self.current_element {
                        self.decrease_value(position);
                    }
                } else {
                    self.current_element = self.current_element.previous_element(self.num_elements);
                }
            }
            UiInput::ButtonPress => match self.current_element {
                Element::Save => {
                    let mut settings_accessor = FlashSettingsAccessor::new();
                    settings_accessor
                        .save_setting(self.setting_id_to_save, SettingValue::UInt(self.value))
                        .await
                        .unwrap_or_else(|e| {
                            error!("Failed to save setting value - {}", Debug2Format(&e))
                        });
                    ui_action_publisher.publish_immediate(UiActionsMessage::StateChangeRequest(
                        ApplicationState::Settings,
                    ));
                }
                Element::Cancel => {
                    ui_action_publisher.publish_immediate(UiActionsMessage::StateChangeRequest(
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

impl UiDrawer for SetNumberScreen {
    fn draw<DI>(&self, display: &mut GraphicsMode<DI>)
    where
        DI: DisplayInterface,
    {
        let active_element_style = MonoTextStyleBuilder::new()
            .font(&FONT_6X10)
            .text_color(BinaryColor::Off)
            .background_color(BinaryColor::On)
            .build();
        let hover_element_style = MonoTextStyleBuilder::new()
            .font(&FONT_6X10)
            .text_color(BinaryColor::On)
            .background_color(BinaryColor::Off)
            .underline()
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
        let mut next_point = Point::new((display.size().width / 2) as i32, 0);

        string_buffer.clear();
        writeln!(&mut string_buffer, "{}", self.label).unwrap();
        next_point = Text::with_text_style(
            string_buffer.as_str(),
            next_point,
            label_char_style,
            label_text_style,
        )
        .draw(display)
        .unwrap();

        let setting_text_width = (self.num_elements + 1 + self.units.len())
            * text_style.font.character_size.width as usize;
        let x_offset = ((display.size().width - setting_text_width as u32) / 2) as i32;

        next_point.x =
            x_offset + (self.num_elements * text_style.font.character_size.width as usize) as i32;
        next_point.y = ((display.size().height / 2) - text_style.line_height() / 2) as i32;

        // draws right to left because it's easier to mask off each digit
        let mut value_to_display = self.value;
        for element_idx in (0..self.num_elements).rev() {
            let style_to_use = if let Element::NumberEntryPosition(position) = self.current_element
            {
                if position == element_idx {
                    if self.element_active {
                        active_element_style
                    } else {
                        hover_element_style
                    }
                } else {
                    text_style
                }
            } else {
                text_style
            };

            string_buffer.clear();
            write!(&mut string_buffer, "{}", value_to_display % 10).unwrap();
            Text::with_baseline(
                string_buffer.as_str(),
                next_point,
                style_to_use,
                Baseline::Top,
            )
            .draw(display)
            .unwrap();

            value_to_display /= 10;
            next_point.x -= text_style.font.character_size.width as i32;
        }

        next_point.x = x_offset
            + ((self.num_elements + 2) * text_style.font.character_size.width as usize) as i32;

        string_buffer.clear();
        write!(&mut string_buffer, "{}", self.units).unwrap();
        next_point = Text::with_baseline(
            string_buffer.as_str(),
            next_point,
            text_style,
            Baseline::Top,
        )
        .draw(display)
        .unwrap();

        next_point.x = 0;
        next_point.y = display.size().height as i32;
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
        .draw(display)
        .unwrap();

        next_point.x = display.size().width as i32;
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
        .draw(display)
        .unwrap();
    }
}
