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
use crate::application::messaging::{
    ApplicationChannelSubscriber, ApplicationData, ApplicationMessage,
};
use crate::hmi::messaging::{HmiMessage, UiActionChannelPublisher};
use crate::hmi::rotary_encoder::Direction;
use crate::hmi::screens::monitoring::MonitoringScreen;
use crate::hmi::screens::settings_menu::monitoring_options::MonitoringTargetPeriodOptions;
use crate::hmi::screens::settings_menu::SettingMenu;
use crate::hmi::screens::settings_screens::about::AboutScreen;
use crate::hmi::screens::settings_screens::calibration::CalibrationScreens;
use crate::hmi::screens::settings_screens::heap_status::HeapStatusScreen;
use crate::hmi::screens::settings_screens::set_date_time::SetDateTimeScreen;
use crate::hmi::screens::settings_screens::set_number::SetNumberScreen;
use crate::hmi::screens::settings_screens::test_mode::TestModeScreen;
use crate::hmi::screens::{draw_message_screen, UiDrawer, UiInput, UiInputHandler};
use crate::rtc::accessor::RtcAccessor;
use crate::storage::settings::{SettingValue, SettingsAccessor, SettingsAccessorId};
use defmt::{debug, error, trace, warn, Debug2Format};
use embassy_futures::select::{select3, Either3};
use embassy_sync::pubsub::WaitResult;
use embassy_time::{Duration, Instant, Ticker};
use sh1106::mode::GraphicsMode;

const DEFAULT_BRIGHTNESS: u8 = 128;
const DEFAULT_DISPLAY_TIMEOUT_MINUTES: u8 = 15;

pub struct DisplayManager<DI, SA>
where
    DI: sh1106::interface::DisplayInterface,
    SA: SettingsAccessor,
{
    app_channel_subscriber: ApplicationChannelSubscriber<'static>,
    ui_action_publisher: UiActionChannelPublisher<'static>,
    display: GraphicsMode<DI>,

    display_state: ApplicationState,
    last_display_update: Instant,
    display_timeout: Duration,

    settings_screen: SettingMenu<SA>,
    test_mode_screen: TestModeScreen,
    monitoring_screen: MonitoringScreen,
    heap_status_screen: HeapStatusScreen,
    calibration_screens: CalibrationScreens,
    set_date_time_screen: SetDateTimeScreen,
    number_setting_screen: SetNumberScreen,
    about_screen: AboutScreen,

    settings: SA,
    rtc_accessor: RtcAccessor,
}

impl<DI, SA> DisplayManager<DI, SA>
where
    DI: sh1106::interface::DisplayInterface,
    SA: SettingsAccessor,
{
    const FRAME_TIMING_MS: u32 = 1000 / 30;

    pub async fn new(
        mut display: GraphicsMode<DI>,
        app_channel_subscriber: ApplicationChannelSubscriber<'static>,
        ui_action_publisher: UiActionChannelPublisher<'static>,
        settings: SA,
    ) -> Self {
        let _ = display.init().map_err(|_| error!("Failed to init display"));
        let _ = display
            .flush()
            .map_err(|_| error!("Failed to flush display"));

        let rtc_accessor =
            RtcAccessor::new().unwrap_or_else(|_| panic!("Failed to get RTC accessor"));

        let mut s = Self {
            app_channel_subscriber,
            ui_action_publisher,
            display,

            display_state: ApplicationState::Startup,
            last_display_update: Instant::MIN,
            display_timeout: Duration::from_secs(30 * 60),

            settings_screen: SettingMenu::new(&settings).await,
            test_mode_screen: TestModeScreen::new(),
            monitoring_screen: MonitoringScreen::new(),
            heap_status_screen: HeapStatusScreen::new(),
            calibration_screens: CalibrationScreens::new(),
            set_date_time_screen: SetDateTimeScreen::new(),
            number_setting_screen: SetNumberScreen::new(
                // set some default values, these are changed as required
                "Default",
                "X",
                0,
                0,
                1000,
                SettingsAccessorId::MonitoringTargetDaily,
            ),
            about_screen: AboutScreen::new(),

            settings,
            rtc_accessor,
        };
        Self::set_display_brightness_from_settings(&mut s).await;
        s.display_timeout = Self::get_display_timeout_from_settings(&mut s).await;
        s
    }

    async fn set_display_brightness_from_settings(&mut self) {
        let display_brightness: u8 = if let Some(result) = self
            .settings
            .get_setting(SettingsAccessorId::SystemDisplayBrightness)
            .await
        {
            match result {
                SettingValue::SmallUInt(v) => v,
                _ => DEFAULT_BRIGHTNESS,
            }
        } else {
            DEFAULT_BRIGHTNESS
        };

        self.display
            .set_contrast(display_brightness)
            .unwrap_or_else(|_| warn!("Failed to set display brightness"));
    }

    async fn get_display_timeout_from_settings(&mut self) -> Duration {
        let display_timeout: u8 = if let Some(result) = self
            .settings
            .get_setting(SettingsAccessorId::DisplayTimeoutMinutes)
            .await
        {
            match result {
                SettingValue::SmallUInt(v) => v,
                _ => DEFAULT_DISPLAY_TIMEOUT_MINUTES,
            }
        } else {
            DEFAULT_DISPLAY_TIMEOUT_MINUTES
        };
        debug!("Restored display_timeout={:?}", display_timeout);
        Duration::from_secs(display_timeout as u64 * 60)
    }

    async fn setup_monitoring_target_value_selection(&mut self) {
        let monitoring_target_id = if let SettingValue::SmallUInt(value) = self
            .settings
            .get_setting(SettingsAccessorId::MonitoringTargetType)
            .await
            .unwrap_or(SettingValue::SmallUInt(0))
        {
            value
        } else {
            0u8
        };
        let monitoring_target =
            MonitoringTargetPeriodOptions::try_from(monitoring_target_id as usize).unwrap();

        let accessor_id = match monitoring_target {
            MonitoringTargetPeriodOptions::Daily => SettingsAccessorId::MonitoringTargetDaily,
            MonitoringTargetPeriodOptions::Hourly => SettingsAccessorId::MonitoringTargetHourly,
        };

        let properties = accessor_id.get_numeric_properties().unwrap();
        let value = if let SettingValue::UInt(value) = self
            .settings
            .get_setting(accessor_id)
            .await
            .unwrap_or(SettingValue::UInt(0))
        {
            value
        } else {
            0u32
        };

        self.number_setting_screen = SetNumberScreen::new(
            monitoring_target.title(),
            monitoring_target.units(),
            value,
            properties.minimum_value,
            properties.maximum_value,
            accessor_id,
        );
    }

    pub async fn set_display_state(&mut self, display_state: ApplicationState) {
        debug!("Display state: {:?}", display_state);

        if let ApplicationState::NumberEntry(setting_id) = display_state {
            if setting_id == SettingsAccessorId::MonitoringTargetDaily {
                self.setup_monitoring_target_value_selection().await
            }
        }

        self.display_state = display_state;
        let dt = self.rtc_accessor.get_date_time();
        self.route_ui_input(UiInput::DateTimeUpdate(dt)).await;
        self.update_now().await;
    }

    pub async fn update_screen(&mut self) {
        if self.last_display_update.elapsed().as_millis() < Self::FRAME_TIMING_MS as u64 {
            return;
        }
        self.update_now().await;
    }

    async fn update_now(&mut self) {
        self.display.clear();
        match self.display_state {
            ApplicationState::Startup => {
                draw_message_screen(&mut self.display, "Starting up...").unwrap()
            }

            ApplicationState::ErrorScreenWithMessage(s) => {
                draw_message_screen(&mut self.display, s).unwrap()
            }

            ApplicationState::TestScreen => self.test_mode_screen.draw(&mut self.display).unwrap(),
            ApplicationState::Settings => self.settings_screen.draw(&mut self.display).unwrap(),
            ApplicationState::Monitoring => self.monitoring_screen.draw(&mut self.display).unwrap(),
            ApplicationState::HeapStatus => {
                self.heap_status_screen.draw(&mut self.display).unwrap()
            }
            ApplicationState::Calibration => {
                self.calibration_screens.draw(&mut self.display).unwrap()
            }
            ApplicationState::SetDateTime => {
                self.set_date_time_screen.draw(&mut self.display).unwrap()
            }
            ApplicationState::NumberEntry(_) => {
                self.number_setting_screen.draw(&mut self.display).unwrap()
            }
            ApplicationState::AboutScreen => {
                self.about_screen.update_pre_draw_actions(&self.display);
                self.about_screen.draw(&mut self.display).unwrap()
            }
        }

        self.display
            .flush()
            .unwrap_or_else(|_| error!("Display flush failed"));
        self.last_display_update = Instant::now();
    }

    async fn route_ui_input(&mut self, input: UiInput) {
        match self.display_state {
            ApplicationState::Startup => {}
            ApplicationState::ErrorScreenWithMessage(_) => {}

            ApplicationState::Settings => {
                self.settings_screen
                    .ui_input_handler(input, &self.ui_action_publisher)
                    .await
            }
            ApplicationState::TestScreen => {
                self.test_mode_screen
                    .ui_input_handler(input, &self.ui_action_publisher)
                    .await
            }
            ApplicationState::Monitoring => {
                self.monitoring_screen
                    .ui_input_handler(input, &self.ui_action_publisher)
                    .await
            }
            ApplicationState::HeapStatus => {
                self.heap_status_screen
                    .ui_input_handler(input, &self.ui_action_publisher)
                    .await
            }
            ApplicationState::Calibration => {
                self.calibration_screens
                    .ui_input_handler(input, &self.ui_action_publisher)
                    .await
            }
            ApplicationState::SetDateTime => {
                self.set_date_time_screen
                    .ui_input_handler(input, &self.ui_action_publisher)
                    .await
            }
            ApplicationState::NumberEntry(_) => {
                self.number_setting_screen
                    .ui_input_handler(input, &self.ui_action_publisher)
                    .await
            }
            ApplicationState::AboutScreen => {
                self.about_screen
                    .ui_input_handler(input, &self.ui_action_publisher)
                    .await
            }
        }
    }

    pub async fn run(&mut self) {
        self.update_screen().await;
        let mut update_ticker = Ticker::every(Duration::from_millis(200));
        let mut last_activity = Instant::now();
        let mut screen_off = false;

        loop {
            let wait_result = select3(
                self.app_channel_subscriber.next_message(),
                update_ticker.next(),
                self.rtc_accessor.wait_for_next_second(),
            )
            .await;
            match wait_result {
                Either3::First(w) => match w {
                    WaitResult::Lagged(count) => {
                        warn! {"Display lost {} messages from HMI channel", count}
                    }
                    WaitResult::Message(message) => match message {
                        ApplicationMessage::HmiInput(hmi_message) => {
                            last_activity = Instant::now();
                            match hmi_message {
                                HmiMessage::EncoderUpdate(direction) => {
                                    trace!("Encoder update: {:?}", Debug2Format(&direction));
                                    match direction {
                                        Direction::Clockwise => {
                                            self.route_ui_input(UiInput::EncoderClockwise).await
                                        }
                                        Direction::CounterClockwise => {
                                            self.route_ui_input(UiInput::EncoderCounterClockwise)
                                                .await
                                        }
                                        Direction::None => {}
                                    }
                                }
                                HmiMessage::PushButtonPressed(is_pressed) => {
                                    trace!("Button pressed {:?} ", is_pressed);
                                    match is_pressed {
                                        true => self.route_ui_input(UiInput::ButtonPress).await,
                                        false => self.route_ui_input(UiInput::ButtonRelease).await,
                                    }
                                }
                            }
                        }
                        ApplicationMessage::ApplicationStateUpdate(new_state) => {
                            trace!("Set display state");
                            self.set_display_state(new_state).await;
                            last_activity = Instant::now();
                        }
                        ApplicationMessage::ApplicationDataUpdate(data_update) => match data_update
                        {
                            ApplicationData::DisplayBrightness(new_display_brightness) => {
                                last_activity = Instant::now();
                                trace!("Brightness update: {:?}", new_display_brightness);
                                self.display
                                    .set_contrast(new_display_brightness)
                                    .unwrap_or_else(|_| warn!("Failed to set display brightness"));
                                self.settings
                                    .save_setting(
                                        SettingsAccessorId::SystemDisplayBrightness,
                                        SettingValue::SmallUInt(new_display_brightness),
                                    )
                                    .await
                                    .unwrap_or_else(|e| {
                                        warn!(
                                            "Failed to save display brightness: {}",
                                            Debug2Format(&e)
                                        );
                                    });
                            }
                            ApplicationData::DisplayTimeout(new_display_timeout) => {
                                trace!("New display timeout: {:?}", new_display_timeout);
                                self.display_timeout =
                                    Duration::from_secs((new_display_timeout * 60) as u64);
                                self.settings
                                    .save_setting(
                                        SettingsAccessorId::DisplayTimeoutMinutes,
                                        SettingValue::SmallUInt(new_display_timeout),
                                    )
                                    .await
                                    .unwrap_or_else(|e| {
                                        warn!(
                                            "Failed to save display timeout: {}",
                                            Debug2Format(&e)
                                        );
                                    });
                            }
                            _ => {
                                trace!("App data update");
                                if let ApplicationData::MonitoringUpdate(_) = data_update {
                                    last_activity = Instant::now();
                                }
                                self.route_ui_input(UiInput::ApplicationData(data_update))
                                    .await;
                            }
                        },
                        ApplicationMessage::WeighSystemRequest(..) => {}
                    },
                },
                Either3::Second(_) => {}
                Either3::Third(dt) => {
                    if self.display_state != ApplicationState::SetDateTime {
                        trace!("DateTime update");
                        self.route_ui_input(UiInput::DateTimeUpdate(dt)).await;
                    }
                }
            }
            if last_activity.elapsed() > self.display_timeout {
                if !screen_off {
                    debug!(
                        "Display timeout reached - turning display off - {} sec",
                        self.display_timeout.as_secs()
                    );
                    self.display.clear();
                    self.display
                        .flush()
                        .unwrap_or_else(|_| warn!("Display flush failed"));
                    screen_off = true;
                }
            } else {
                screen_off = false;
                self.update_screen().await;
            }
        }
    }
}
