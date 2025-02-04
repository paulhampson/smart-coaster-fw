use crate::application::application_state::ApplicationState;
use crate::application::storage::settings::{SettingValue, SettingsAccessor, SettingsAccessorId};
use crate::hmi::messaging::{UiActionChannelPublisher, UiActionsMessage};
use crate::hmi::screens::settings::display_brightness_options::DisplayBrightnessOptions;
use crate::hmi::screens::{UiDrawer, UiInput, UiInputHandler};
use core::marker::PhantomData;
use defmt::{debug, error};
use defmt::{warn, Debug2Format};
use ds323x::{NaiveDateTime};
use embedded_graphics::mono_font::ascii::{FONT_6X10, FONT_7X13_BOLD};
use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::Drawable;
use led_brightness_options::LedBrightnessOptions;
use sh1106::prelude::GraphicsMode;
use simple_embedded_graphics_menu::items::SelectedData;
use simple_embedded_graphics_menu::{Menu, MenuStyle};

mod display_brightness_options;
mod led_brightness_options;

#[derive(Copy, Clone, Debug)]
pub enum SettingMenuIdentifier {
    None,
    Root,
    EnterTestScreen,
    EnterHeapStatusScreen,
    DoCalibration,
    SetLedBrightness,
    DisplayBrightness,
    SetDateTime,
}

pub struct SettingMenu<SA>
where
    SA: SettingsAccessor,
{
    menu: Menu<'static, BinaryColor, SettingMenuIdentifier>,
    datetime: NaiveDateTime,
    phantom: PhantomData<SA>,
}

impl<SA> SettingMenu<SA>
where
    SA: SettingsAccessor,
{
    pub async fn new(settings: &SA) -> Self {
        Self {
            menu: Self::build_menu(settings).await,
            datetime: NaiveDateTime::default(),
            phantom: PhantomData,
        }
    }

    async fn build_menu(settings: &SA) -> Menu<'static, BinaryColor, SettingMenuIdentifier> {
        let heading_style = MonoTextStyle::new(&FONT_7X13_BOLD, BinaryColor::On);
        let item_style = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);
        let highlighted_item_style = MonoTextStyle::new(&FONT_6X10, BinaryColor::Off);

        let menu_style = MenuStyle::new(
            BinaryColor::Off,
            heading_style,
            item_style,
            BinaryColor::On,
            BinaryColor::On,
            highlighted_item_style,
            BinaryColor::Off,
        );

        let mut device_and_system =
            Menu::new("Device & System", SettingMenuIdentifier::None, menu_style);
        device_and_system.add_section("LEDs", SettingMenuIdentifier::None);

        {
            let led_brightness: u8 = if let Some(result) = settings
                .get_setting(SettingsAccessorId::SystemLedBrightness)
                .await
            {
                match result {
                    SettingValue::SmallUInt(v) => v,
                    _ => {
                        warn!("Unable to retrieve LED brightness setting");
                        128
                    }
                }
            } else {
                128
            };

            let led_brightness_option =
                LedBrightnessOptions::from(led_brightness).to_option_index();
            device_and_system.add_selector(
                "LED Brightness",
                SettingMenuIdentifier::SetLedBrightness,
                LedBrightnessOptions::option_strings(),
                Some(led_brightness_option),
            );
        }

        device_and_system.add_section("Display", SettingMenuIdentifier::None);
        {
            let display_brightness: u8 = if let Some(result) = settings
                .get_setting(SettingsAccessorId::SystemDisplayBrightness)
                .await
            {
                match result {
                    SettingValue::SmallUInt(v) => v,
                    _ => {
                        warn!("Unable to retrieve display brightness setting");
                        128
                    }
                }
            } else {
                128
            };

            let display_brightness_option =
                DisplayBrightnessOptions::from(display_brightness).to_option_index();

            device_and_system.add_selector(
                "Brightness",
                SettingMenuIdentifier::DisplayBrightness,
                DisplayBrightnessOptions::option_strings(),
                Some(display_brightness_option),
            );
        }
        device_and_system.add_section("System", SettingMenuIdentifier::None);
        device_and_system.add_action("Set Date/Time", SettingMenuIdentifier::SetDateTime);
        device_and_system.add_action("Calibration", SettingMenuIdentifier::DoCalibration);
        device_and_system.add_action("Test Mode", SettingMenuIdentifier::EnterTestScreen);
        device_and_system.add_action("Heap Status", SettingMenuIdentifier::EnterHeapStatusScreen);
        device_and_system.add_back("Back", SettingMenuIdentifier::None);

        let mut menu = Menu::new("Settings", SettingMenuIdentifier::Root, menu_style);
        menu.add_submenu(device_and_system);
        menu.add_exit("Exit", SettingMenuIdentifier::None);

        menu
    }

    fn process_selection(
        &self,
        selection_data: SelectedData<SettingMenuIdentifier>,
        ui_action_publisher: &UiActionChannelPublisher,
    ) {
        match selection_data {
            SelectedData::Action { id: identifier } => match identifier {
                SettingMenuIdentifier::EnterTestScreen => {
                    ui_action_publisher.publish_immediate(UiActionsMessage::StateChangeRequest(
                        ApplicationState::TestScreen,
                    ));
                }
                SettingMenuIdentifier::EnterHeapStatusScreen => {
                    ui_action_publisher.publish_immediate(UiActionsMessage::StateChangeRequest(
                        ApplicationState::HeapStatus,
                    ));
                }
                SettingMenuIdentifier::DoCalibration => ui_action_publisher.publish_immediate(
                    UiActionsMessage::StateChangeRequest(ApplicationState::Calibration),
                ),
                SettingMenuIdentifier::SetDateTime => ui_action_publisher.publish_immediate(
                    UiActionsMessage::StateChangeRequest(ApplicationState::SetDateTime),
                ),
                _ => {}
            },
            SelectedData::MultiOption { id, option_id } => {
                debug!(
                    "MultiOption selected: {:?} => {:?}",
                    Debug2Format(&id),
                    Debug2Format(&option_id)
                );
                match id {
                    SettingMenuIdentifier::SetLedBrightness => {
                        ui_action_publisher.publish_immediate(
                            UiActionsMessage::LedBrightnessChangeRequest(
                                LedBrightnessOptions::option_index_to_brightness(option_id),
                            ),
                        );
                    }
                    SettingMenuIdentifier::DisplayBrightness => {
                        ui_action_publisher.publish_immediate(
                            UiActionsMessage::DisplayBrightnessChangeRequest(
                                DisplayBrightnessOptions::option_index_to_brightness(option_id),
                            ),
                        );
                    }
                    _ => {}
                }
            }
            SelectedData::Checkbox { id: _, state: _ } => {}
            SelectedData::Exit { id: _ } => {
                ui_action_publisher.publish_immediate(UiActionsMessage::StateChangeRequest(
                    ApplicationState::Monitoring,
                ));
            }
            _ => {}
        }
    }
}

impl<SA> UiInputHandler for SettingMenu<SA>
where
    SA: SettingsAccessor,
{
    fn ui_input_handler(&mut self, input: UiInput, ui_action_publisher: &UiActionChannelPublisher) {
        match input {
            UiInput::EncoderClockwise => {
                self.menu.navigate_down();
            }
            UiInput::EncoderCounterClockwise => {
                self.menu.navigate_up();
            }
            UiInput::ButtonPress => {
                if let Some(select_result) = self.menu.select_item() {
                    self.process_selection(select_result, ui_action_publisher);
                }
            }
            UiInput::ButtonRelease => {}
            UiInput::ApplicationData(_) => {}
            UiInput::DateTimeUpdate(dt) => {self.datetime = dt;}
        }
    }
}

impl<SA> UiDrawer for SettingMenu<SA>
where
    SA: SettingsAccessor,
{
    fn draw<DI>(&self, display: &mut GraphicsMode<DI>)
    where
        DI: sh1106::interface::DisplayInterface,
    {
        self.menu
            .draw(display)
            .unwrap_or_else(|_| error!("Setting menu draw failed"));
    }
}
