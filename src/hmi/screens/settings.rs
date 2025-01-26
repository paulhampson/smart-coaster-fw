use crate::application::application_state::ApplicationState;
use crate::application::storage::settings::{
    wait_for_settings_store_initialisation, SETTINGS_STORE,
};
use crate::hmi::messaging::{UiActionChannelPublisher, UiActionsMessage};
use crate::hmi::screens::{UiDrawer, UiInput, UiInputHandler};
use defmt::Debug2Format;
use defmt::{debug, error};
use embedded_graphics::mono_font::ascii::{FONT_6X10, FONT_7X13_BOLD};
use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::Drawable;
use sh1106::prelude::GraphicsMode;
use simple_embedded_graphics_menu::items::SelectedData;
use simple_embedded_graphics_menu::{Menu, MenuStyle};

#[derive(Copy, Clone, Debug)]
pub enum SettingMenuIdentifier {
    None,
    Root,
    EnterTestScreen,
    EnterHeapStatusScreen,
    DoCalibration,
    SetLedBrightness,
}

pub enum LedBrightnessOptions {
    Off,
    Low,
    Medium,
    High,
    Max,
}

impl LedBrightnessOptions {
    const OFF: u8 = 0;
    const LOW: u8 = 75;
    const MEDIUM: u8 = 128;
    const HIGH: u8 = 200;
    const MAX: u8 = 255;

    pub fn option_strings() -> &'static [&'static str] {
        &["Off", "Low", "Medium", "High", "Max"]
    }

    pub fn brightness_mapping(&self) -> u8 {
        match self {
            LedBrightnessOptions::Off => Self::OFF,
            LedBrightnessOptions::Low => Self::LOW,
            LedBrightnessOptions::Medium => Self::MEDIUM,
            LedBrightnessOptions::High => Self::HIGH,
            LedBrightnessOptions::Max => Self::MAX,
        }
    }

    pub fn option_index_to_brightness(index: usize) -> u8 {
        match index {
            0 => Self::OFF,
            1 => Self::LOW,
            2 => Self::MEDIUM,
            3 => Self::HIGH,
            4 => Self::MAX,
            _ => 0,
        }
    }

    pub fn to_option_index(&self) -> usize {
        match self {
            LedBrightnessOptions::Off => 0,
            LedBrightnessOptions::Low => 1,
            LedBrightnessOptions::Medium => 2,
            LedBrightnessOptions::High => 3,
            LedBrightnessOptions::Max => 4,
        }
    }
}

impl From<u8> for LedBrightnessOptions {
    fn from(value: u8) -> Self {
        match value {
            0..=40 => Self::Off,
            41..=90 => Self::Low,
            91..=150 => Self::Medium,
            151..=254 => Self::High,
            255 => Self::Max,
            _ => Self::Off,
        }
    }
}

pub struct SettingMenu {
    menu: Menu<'static, BinaryColor, SettingMenuIdentifier>,
}

impl SettingMenu {
    pub async fn new() -> Self {
        Self {
            menu: Self::build_menu().await,
        }
    }

    async fn build_menu() -> Menu<'static, BinaryColor, SettingMenuIdentifier> {
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

        wait_for_settings_store_initialisation().await;

        let mut menu = Menu::new("Settings", SettingMenuIdentifier::Root, menu_style);
        menu.add_section("LEDs", SettingMenuIdentifier::None);
        let settings = SETTINGS_STORE.lock().await;
        let led_brightness_option =
            LedBrightnessOptions::from(settings.get_system_led_brightness().await.unwrap_or(128))
                .to_option_index();
        menu.add_selector(
            "LED Brightness",
            SettingMenuIdentifier::SetLedBrightness,
            LedBrightnessOptions::option_strings(),
            Some(led_brightness_option),
        );
        menu.add_section("System", SettingMenuIdentifier::None);
        menu.add_action("Calibration", SettingMenuIdentifier::DoCalibration);
        menu.add_action("Device Test Mode", SettingMenuIdentifier::EnterTestScreen);
        menu.add_action("Heap Status", SettingMenuIdentifier::EnterHeapStatusScreen);
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

impl UiInputHandler for SettingMenu {
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
        }
    }
}

impl UiDrawer for SettingMenu {
    fn draw<DI>(&self, display: &mut GraphicsMode<DI>)
    where
        DI: sh1106::interface::DisplayInterface,
    {
        self.menu
            .draw(display)
            .unwrap_or_else(|_| error!("Setting menu draw failed"));
    }
}
