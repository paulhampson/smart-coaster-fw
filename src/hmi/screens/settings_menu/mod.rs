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
use crate::hmi::screens::settings_menu::display_options::{
    DisplayBrightnessOptions, DisplayTimeoutOptions,
};
use crate::hmi::screens::{UiDrawer, UiInput, UiInputHandler};
use crate::storage::settings::{SettingValue, SettingsAccessor, SettingsAccessorId};
use defmt::debug;
use defmt::{warn, Debug2Format};
use ds323x::NaiveDateTime;
use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::mono_font::ascii::{FONT_6X10, FONT_7X13_BOLD};
use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::Drawable;
use led_brightness_options::LedBrightnessOptions;
use simple_embedded_graphics_menu::items::SelectedData;
use simple_embedded_graphics_menu::{Menu, MenuStyle};

mod display_options;
mod led_brightness_options;
pub mod monitoring_options;

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
    SetMonitoringTargetType,
    SetMonitoringTargetValue,
    DisplayTimeout,
    AboutScreen,
    SetDailyTargetTime,
}

pub struct SettingMenu<'a, SA>
where
    SA: SettingsAccessor,
{
    menu: Menu<'static, BinaryColor, SettingMenuIdentifier>,
    datetime: NaiveDateTime,
    settings_accessor: &'a SA,
}

impl<'a, SA> SettingMenu<'a, SA>
where
    SA: SettingsAccessor,
{
    pub async fn new(settings: &'a SA) -> Self {
        Self {
            menu: Self::build_menu(settings).await,
            datetime: NaiveDateTime::default(),
            settings_accessor: settings,
        }
    }

    async fn build_consumption_monitoring_menu(
        menu: &mut Menu<'_, BinaryColor, SettingMenuIdentifier>,
        settings: &SA,
    ) {
        let monitoring_target_type: u8 = {
            if let Some(result) = settings
                .get_setting(SettingsAccessorId::MonitoringTargetType)
                .await
            {
                match result {
                    SettingValue::SmallUInt(v) => v,
                    _ => {
                        warn!("Unable to retrieve monitoring target type");
                        0
                    }
                }
            } else {
                0
            }
        };
        menu.add_selector(
            "Target type",
            SettingMenuIdentifier::SetMonitoringTargetType,
            monitoring_options::MonitoringTargetPeriodOptions::option_strings(),
            Some(monitoring_target_type as usize),
        );

        menu.add_action("Target", SettingMenuIdentifier::SetMonitoringTargetValue);
        menu.add_action(
            "Daily Target Time",
            SettingMenuIdentifier::SetDailyTargetTime,
        );
        menu.add_back("Back", SettingMenuIdentifier::None);
    }

    async fn build_device_and_system_menu(
        menu: &mut Menu<'_, BinaryColor, SettingMenuIdentifier>,
        settings: &SA,
    ) {
        menu.add_section("LEDs", SettingMenuIdentifier::None);

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
            menu.add_selector(
                "LED Brightness",
                SettingMenuIdentifier::SetLedBrightness,
                LedBrightnessOptions::option_strings(),
                Some(led_brightness_option),
            );
        }

        menu.add_section("Display", SettingMenuIdentifier::None);
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

            menu.add_selector(
                "Brightness",
                SettingMenuIdentifier::DisplayBrightness,
                DisplayBrightnessOptions::option_strings(),
                Some(display_brightness_option),
            );
        }

        {
            let display_timeout: u8 = if let Some(result) = settings
                .get_setting(SettingsAccessorId::DisplayTimeoutMinutes)
                .await
            {
                match result {
                    SettingValue::SmallUInt(v) => v,
                    _ => {
                        warn!("Unable to retrieve display timeout setting");
                        DisplayTimeoutOptions::DEFAULT
                    }
                }
            } else {
                DisplayTimeoutOptions::DEFAULT
            };

            let display_timeout_option =
                DisplayTimeoutOptions::minutes_to_option_index(display_timeout);

            menu.add_selector(
                "Display timeout",
                SettingMenuIdentifier::DisplayTimeout,
                DisplayTimeoutOptions::option_strings(),
                Some(display_timeout_option),
            );
        }

        menu.add_section("System", SettingMenuIdentifier::None);
        menu.add_action("Set Date/Time", SettingMenuIdentifier::SetDateTime);
        menu.add_action("Calibration", SettingMenuIdentifier::DoCalibration);
        menu.add_action("Test Mode", SettingMenuIdentifier::EnterTestScreen);
        menu.add_action("Heap Status", SettingMenuIdentifier::EnterHeapStatusScreen);
        menu.add_back("Back", SettingMenuIdentifier::None);
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

        let mut consumption_monitoring =
            Menu::new("Drink Monitoring", SettingMenuIdentifier::None, menu_style);
        Self::build_consumption_monitoring_menu(&mut consumption_monitoring, settings).await;

        let mut device_and_system =
            Menu::new("Device & System", SettingMenuIdentifier::None, menu_style);
        Self::build_device_and_system_menu(&mut device_and_system, settings).await;

        let mut menu = Menu::new("Settings", SettingMenuIdentifier::Root, menu_style);
        menu.add_submenu(consumption_monitoring);
        menu.add_submenu(device_and_system);
        menu.add_action("About", SettingMenuIdentifier::AboutScreen);
        menu.add_exit("Exit", SettingMenuIdentifier::None);

        menu
    }

    async fn process_multi_options(
        &self,
        ui_action_publisher: &UiActionChannelPublisher<'static>,
        id: SettingMenuIdentifier,
        option_id: usize,
    ) {
        debug!(
            "MultiOption selected: {:?} => {:?}",
            Debug2Format(&id),
            Debug2Format(&option_id)
        );
        match id {
            SettingMenuIdentifier::SetLedBrightness => {
                // TODO change this to a direct setting write and then publish change through settings channel
                ui_action_publisher.publish_immediate(UiRequestMessage::ChangeLedBrightness(
                    LedBrightnessOptions::option_index_to_brightness(option_id),
                ));
            }
            SettingMenuIdentifier::DisplayBrightness => {
                // TODO change this to a direct setting write and then publish change through settings channel
                ui_action_publisher.publish_immediate(UiRequestMessage::ChangeDisplayBrightness(
                    DisplayBrightnessOptions::option_index_to_brightness(option_id),
                ));
            }
            SettingMenuIdentifier::SetMonitoringTargetType => {
                self.settings_accessor
                    .save_setting(
                        SettingsAccessorId::MonitoringTargetType,
                        SettingValue::SmallUInt(option_id as u8),
                    )
                    .await
                    .unwrap_or_else(|e| {
                        warn!("Failed to store monitoring type: {:?}", Debug2Format(&e))
                    });
            }
            SettingMenuIdentifier::DisplayTimeout => {
                // TODO change this to a direct setting write and then publish change through settings channel
                ui_action_publisher.publish_immediate(UiRequestMessage::ChangeDisplayTimeout(
                    DisplayTimeoutOptions::option_index_to_minutes(option_id),
                ))
            }

            SettingMenuIdentifier::None => {}
            SettingMenuIdentifier::Root => {}
            SettingMenuIdentifier::EnterTestScreen => {}
            SettingMenuIdentifier::EnterHeapStatusScreen => {}
            SettingMenuIdentifier::DoCalibration => {}
            SettingMenuIdentifier::SetDateTime => {}
            SettingMenuIdentifier::SetMonitoringTargetValue => {}
            SettingMenuIdentifier::AboutScreen => {}
            SettingMenuIdentifier::SetDailyTargetTime => {}
        }
    }

    async fn process_selection(
        &self,
        selection_data: SelectedData<SettingMenuIdentifier>,
        ui_action_publisher: &UiActionChannelPublisher<'static>,
    ) {
        match selection_data {
            SelectedData::Action { id: identifier } => match identifier {
                SettingMenuIdentifier::EnterTestScreen => {
                    ui_action_publisher.publish_immediate(UiRequestMessage::ChangeState(
                        ApplicationState::TestScreen,
                    ));
                }
                SettingMenuIdentifier::EnterHeapStatusScreen => {
                    ui_action_publisher.publish_immediate(UiRequestMessage::ChangeState(
                        ApplicationState::HeapStatus,
                    ));
                }
                SettingMenuIdentifier::DoCalibration => ui_action_publisher.publish_immediate(
                    UiRequestMessage::ChangeState(ApplicationState::Calibration),
                ),
                SettingMenuIdentifier::SetDateTime => ui_action_publisher.publish_immediate(
                    UiRequestMessage::ChangeState(ApplicationState::SetSystemDateTime),
                ),
                SettingMenuIdentifier::SetMonitoringTargetValue => ui_action_publisher
                    .publish_immediate(UiRequestMessage::ChangeState(
                        ApplicationState::NumberEntry(SettingsAccessorId::MonitoringTargetDaily),
                    )),
                SettingMenuIdentifier::AboutScreen => ui_action_publisher.publish_immediate(
                    UiRequestMessage::ChangeState(ApplicationState::AboutScreen),
                ),
                SettingMenuIdentifier::SetDailyTargetTime => {
                    ui_action_publisher.publish_immediate(UiRequestMessage::ChangeState(
                        ApplicationState::TimeEntry(SettingsAccessorId::MonitoringDailyTargetTime),
                    ))
                }
                _ => {}
            },
            SelectedData::MultiOption { id, option_id } => {
                self.process_multi_options(ui_action_publisher, id, option_id)
                    .await;
            }
            SelectedData::Checkbox { id: _, state: _ } => {}
            SelectedData::Exit { id: _ } => {
                ui_action_publisher
                    .publish_immediate(UiRequestMessage::ChangeState(ApplicationState::Monitoring));
            }
            _ => {}
        }
    }
}

impl<'a, SA> UiInputHandler for SettingMenu<'a, SA>
where
    SA: SettingsAccessor,
{
    async fn ui_input_handler(
        &mut self,
        input: UiInput,
        ui_action_publisher: &UiActionChannelPublisher<'static>,
    ) {
        match input {
            UiInput::EncoderClockwise => {
                self.menu.navigate_down();
            }
            UiInput::EncoderCounterClockwise => {
                self.menu.navigate_up();
            }
            UiInput::ButtonPress => {
                if let Some(select_result) = self.menu.select_item() {
                    self.process_selection(select_result, ui_action_publisher)
                        .await;
                }
            }
            UiInput::ButtonRelease => {}
            UiInput::ApplicationData(_) => {}
            UiInput::DateTimeUpdate(dt) => {
                self.datetime = dt;
            }
        }
    }
}

impl<'a, SA> UiDrawer for SettingMenu<'a, SA>
where
    SA: SettingsAccessor,
{
    fn draw<D>(&self, display: &mut D) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = BinaryColor>,
    {
        self.menu.draw(display)
    }
}
