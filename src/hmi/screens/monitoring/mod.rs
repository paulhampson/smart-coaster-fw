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

mod monitoring_screen_1;
mod monitoring_screen_2;
mod monitoring_screen_3;
mod monitoring_screen_debug;
mod top_status_bar;

use crate::application::application_state::ApplicationState;
use crate::application::messaging::ApplicationData;
use crate::drink_monitor::drink_monitoring::MonitoringStateSubstates;
use crate::drink_monitor::messaging::DrinkMonitoringUpdate;
use crate::hmi::messaging::{UiActionChannelPublisher, UiRequestMessage};
use crate::hmi::screens::monitoring::monitoring_screen_1::MonitoringScreen1;
use crate::hmi::screens::monitoring::monitoring_screen_2::MonitoringScreen2;
use crate::hmi::screens::monitoring::monitoring_screen_3::MonitoringScreen3;
use crate::hmi::screens::monitoring::monitoring_screen_debug::MonitoringScreenDebug;
use crate::hmi::screens::monitoring::top_status_bar::TopStatusBar;
use crate::hmi::screens::settings_menu::monitoring_options::MonitoringTargetPeriodOptions;
use crate::hmi::screens::{draw_message_screen, UiDrawer, UiInput, UiInputHandler};
use crate::storage::settings::{SettingValue, SettingsAccessor, SettingsAccessorId};
use chrono::NaiveDateTime;
use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::geometry::{AnchorX, AnchorY, Point};
use embedded_graphics::image::Image;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::{Dimensions, DrawTargetExt, OriginDimensions};
use embedded_graphics::Drawable;
use embedded_icon::NewIcon;

struct MonitoringData {
    last_consumption: f32,
    day_consumption_rate: f32,
    day_total_consumed: f32,
    target_rate: f32,
    day_target_consumption: f32,
    target_mode: MonitoringTargetPeriodOptions,
    last_hour_consumption_rate: f32,
}

trait MonitoringScreenContent<D>
where
    D: DrawTarget<Color = BinaryColor>,
{
    fn draw_content(
        &self,
        display: &mut D,
        state: MonitoringStateSubstates,
        data: &MonitoringData,
    ) -> Result<(), D::Error>;
}

static SCREEN_LAYOUT_1: MonitoringScreen1 = MonitoringScreen1 {};
static SCREEN_LAYOUT_2: MonitoringScreen2 = MonitoringScreen2 {};
static SCREEN_LAYOUT_3: MonitoringScreen3 = MonitoringScreen3 {};
static SCREEN_LAYOUT_DEBUG: MonitoringScreenDebug = MonitoringScreenDebug {};

const MAX_SCREENS: u8 = 3;
fn get_screen_layout<D>(index: &u8) -> &dyn MonitoringScreenContent<D>
where
    D: DrawTarget<Color = BinaryColor>,
{
    match index {
        0 => &SCREEN_LAYOUT_1,
        1 => &SCREEN_LAYOUT_2,
        2 => &SCREEN_LAYOUT_3,
        3 => &SCREEN_LAYOUT_DEBUG,
        _ => &SCREEN_LAYOUT_1,
    }
}

pub struct MonitoringScreen<'a, SA> {
    monitoring_data: MonitoringData,
    state: MonitoringStateSubstates,

    active_screen_index: u8,
    datetime: NaiveDateTime,
    settings: &'a SA,
}

impl<'a, SA> MonitoringScreen<'a, SA>
where
    SA: SettingsAccessor,
{
    pub async fn new(settings: &'a SA) -> Self {
        let active_screen_index = if let SettingValue::SmallUInt(retrieved_screen_index) = settings
            .get_setting(SettingsAccessorId::MonitoringDisplayIndex)
            .await
            .unwrap_or(SettingValue::SmallUInt(0))
        {
            retrieved_screen_index
        } else {
            0
        };

        Self {
            monitoring_data: MonitoringData {
                last_consumption: 0.0,
                day_consumption_rate: 0.0,
                day_total_consumed: 0.0,
                target_rate: 0.0,
                day_target_consumption: 0.0,
                target_mode: MonitoringTargetPeriodOptions::Daily,
                last_hour_consumption_rate: 0.0,
            },
            state: MonitoringStateSubstates::WaitingForActivity,
            active_screen_index,
            datetime: NaiveDateTime::default(),
            settings,
        }
    }

    fn process_application_data(&mut self, data: ApplicationData) {
        if let ApplicationData::MonitoringUpdate(update) = data {
            match update {
                DrinkMonitoringUpdate::Consumption(new_consumption) => {
                    self.monitoring_data.last_consumption = new_consumption;
                }
                DrinkMonitoringUpdate::DayAverageHourlyConsumptionRate(new_consumption_rate) => {
                    self.monitoring_data.day_consumption_rate = new_consumption_rate;
                }
                DrinkMonitoringUpdate::TotalConsumed(new_total_consumed) => {
                    self.monitoring_data.day_total_consumed = new_total_consumed;
                }
                DrinkMonitoringUpdate::UpdateMonitoringSubstate(new_state) => {
                    self.state = new_state;
                }
                DrinkMonitoringUpdate::TargetRate(new_target_rate) => {
                    self.monitoring_data.target_rate = new_target_rate;
                }
                DrinkMonitoringUpdate::TargetConsumption(new_target_consumption) => {
                    self.monitoring_data.day_target_consumption = new_target_consumption;
                }
                DrinkMonitoringUpdate::TargetMode(new_target_mode) => {
                    self.monitoring_data.target_mode = new_target_mode;
                }
                DrinkMonitoringUpdate::LastHourConsumptionRate(new_last_hour_consumption_rate) => {
                    self.monitoring_data.last_hour_consumption_rate =
                        new_last_hour_consumption_rate;
                }
            }
        }
    }
}

impl<'a, SA> UiInputHandler for MonitoringScreen<'a, SA>
where
    SA: SettingsAccessor,
{
    async fn ui_input_handler(
        &mut self,
        input: UiInput,
        ui_action_publisher: &UiActionChannelPublisher<'_>,
    ) {
        match input {
            UiInput::EncoderClockwise => {
                if self.state == MonitoringStateSubstates::VesselPlaced
                    || self.state == MonitoringStateSubstates::VesselRemoved
                {
                    self.active_screen_index += 1;
                    if self.active_screen_index >= MAX_SCREENS {
                        self.active_screen_index = 0;
                    }
                    self.settings
                        .save_setting(
                            SettingsAccessorId::MonitoringDisplayIndex,
                            SettingValue::SmallUInt(self.active_screen_index),
                        )
                        .await
                        .unwrap();
                }
            }
            UiInput::EncoderCounterClockwise => {
                if self.state == MonitoringStateSubstates::VesselPlaced
                    || self.state == MonitoringStateSubstates::VesselRemoved
                {
                    self.active_screen_index -= 1;
                    if self.active_screen_index >= MAX_SCREENS {
                        self.active_screen_index = MAX_SCREENS - 1;
                    }
                    self.settings
                        .save_setting(
                            SettingsAccessorId::MonitoringDisplayIndex,
                            SettingValue::SmallUInt(self.active_screen_index),
                        )
                        .await
                        .unwrap();
                }
            }
            UiInput::ButtonPress => ui_action_publisher
                .publish_immediate(UiRequestMessage::ChangeState(ApplicationState::Settings)),
            UiInput::ButtonRelease => {}
            UiInput::ApplicationData(data) => self.process_application_data(data),
            UiInput::DateTimeUpdate(dt) => self.datetime = dt,
        }
    }
}

impl<'a, SA> MonitoringScreen<'a, SA> {
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

    fn draw_error<D>(&self, display: &mut D, message: &str) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = BinaryColor>,
    {
        let left_icon_area_width = display.bounding_box().size.width / 3;
        let mut left_icon_display = display.cropped(
            &display
                .bounding_box()
                .resized_width(left_icon_area_width, AnchorX::Left),
        );

        let icon = embedded_icon::mdi::size32px::AlertCircleOutline::new(BinaryColor::On);
        let mut icon_location = left_icon_display.bounding_box().center();
        icon_location.x -= (icon.size().width / 2) as i32;
        icon_location.y -= (icon.size().height / 2) as i32;
        Image::new(&icon, icon_location).draw(&mut left_icon_display)?;

        let text_to_icon_padding = 5;
        let right_text_area_width =
            display.bounding_box().size.width - left_icon_area_width - text_to_icon_padding;
        let mut right_text_display = display.cropped(
            &display
                .bounding_box()
                .resized_width(right_text_area_width, AnchorX::Right),
        );

        draw_message_screen(&mut right_text_display, message)?;

        Ok(())
    }
}

impl<'a, SA> UiDrawer for MonitoringScreen<'a, SA> {
    fn draw<D>(&self, display: &mut D) -> Result<(), D::Error>
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
                let active_screen = get_screen_layout(&self.active_screen_index);
                active_screen.draw_content(
                    &mut content_display,
                    self.state,
                    &self.monitoring_data,
                )?
            }
            MonitoringStateSubstates::Error(message) => {
                self.draw_error(&mut content_display, message)?
            }
        }
        Ok(())
    }
}
