use crate::application::application_state::ApplicationState;
use crate::application::storage::settings::accessor::FlashSettingsAccessor;
use crate::application::storage::settings::{SettingValue, SettingsAccessor, SettingsAccessorId};
use crate::hmi::messaging::{UiActionChannelPublisher, UiActionsMessage};
use crate::hmi::screens::settings_screens::SettingScreen;
use crate::hmi::screens::{UiDrawer, UiInput, UiInputHandler};
use core::cmp::PartialEq;
use defmt::error;
use defmt::Debug2Format;
use embedded_graphics::mono_font::ascii::{FONT_6X10, FONT_6X13_BOLD};
use embedded_graphics::mono_font::MonoTextStyleBuilder;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::text::{Alignment, Baseline, TextStyleBuilder};
use heapless::String;
use sh1106::interface::DisplayInterface;
use sh1106::mode::GraphicsMode;

#[derive(PartialEq)]
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

    pub fn reset_for_entry(&mut self, value: u32) {
        self.current_element = Element::NumberEntryPosition(0);
        self.element_active = false;
        self.value = value;
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
                if let Element::NumberEntryPosition(position) = self.current_element {
                    self.value += 10u32.pow(position as u32);
                } else {
                    self.current_element = self.current_element.next_element(self.num_elements);
                }
            }
            UiInput::EncoderCounterClockwise => {
                if self.element_active {
                    if let Element::NumberEntryPosition(position) = self.current_element {
                        self.value -= 10u32.pow(position as u32);
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

        let label_char_style = MonoTextStyleBuilder::new()
            .font(&FONT_6X13_BOLD)
            .text_color(BinaryColor::On)
            .build();
        let label_alignment_style = TextStyleBuilder::new()
            .alignment(Alignment::Center)
            .baseline(Baseline::Top)
            .build();

        let text_style = MonoTextStyleBuilder::new()
            .font(&FONT_6X10)
            .text_color(BinaryColor::On)
            .build();

        let mut string_buffer = String::<32>::new();
    }
}
