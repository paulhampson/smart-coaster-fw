use crate::application::application_state::ApplicationState;
use crate::hmi::messaging::{UiActionChannelPublisher, UiActionsMessage};
use crate::hmi::screens::{UiDrawer, UiInput, UiInputHandler};
use defmt::error;
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
}

pub struct SettingMenu {
    menu: Menu<'static, BinaryColor, SettingMenuIdentifier>,
}

impl SettingMenu {
    pub fn new() -> Self {
        Self {
            menu: Self::build_menu(),
        }
    }

    fn build_menu() -> Menu<'static, BinaryColor, SettingMenuIdentifier> {
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

        let mut menu = Menu::new("Settings", SettingMenuIdentifier::Root, menu_style);
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
