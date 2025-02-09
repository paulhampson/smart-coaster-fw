use crate::application::application_state::ApplicationState;
use crate::application::messaging::{
    ApplicationChannelSubscriber, ApplicationData, ApplicationMessage,
};
use crate::application::storage::settings::option_types::MonitoringTargetPeriodOptions;
use crate::application::storage::settings::{SettingValue, SettingsAccessor, SettingsAccessorId};
use crate::hmi::messaging::{HmiMessage, UiActionChannelPublisher};
use crate::hmi::rotary_encoder::Direction;
use crate::hmi::screens::monitoring::MonitoringScreen;
use crate::hmi::screens::settings_menu::SettingMenu;
use crate::hmi::screens::settings_screens::calibration::CalibrationScreens;
use crate::hmi::screens::settings_screens::heap_status::HeapStatusScreen;
use crate::hmi::screens::settings_screens::set_date_time::SetDateTimeScreen;
use crate::hmi::screens::settings_screens::set_number::SetNumberScreen;
use crate::hmi::screens::settings_screens::test_mode::TestModeScreen;
use crate::hmi::screens::{draw_message_screen, settings_menu, UiDrawer, UiInput, UiInputHandler};
use crate::rtc::accessor::RtcAccessor;
use defmt::{debug, error, trace, warn, Debug2Format};
use embassy_futures::select::{select3, Either3};
use embassy_sync::pubsub::WaitResult;
use embassy_time::{Duration, Instant, Ticker};
use sh1106::mode::GraphicsMode;

const DEFAULT_BRIGHTNESS: u8 = 128;

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

    settings_screen: SettingMenu<SA>,
    test_mode_screen: TestModeScreen,
    monitoring_screen: MonitoringScreen,
    heap_status_screen: HeapStatusScreen,
    calibration_screens: CalibrationScreens,
    set_date_time_screen: SetDateTimeScreen,
    number_entry_screen: SetNumberScreen,

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

        let display_brightness: u8 = if let Some(result) = settings
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

        let _ = display
            .set_contrast(display_brightness)
            .map_err(|_| warn!("Failed to set display brightness"));
        let rtc_accessor =
            RtcAccessor::new().unwrap_or_else(|_| panic!("Failed to get RTC accessor"));

        Self {
            app_channel_subscriber,
            ui_action_publisher,
            display,

            display_state: ApplicationState::Startup,
            last_display_update: Instant::MIN,

            settings_screen: SettingMenu::new(&settings).await,
            test_mode_screen: TestModeScreen::new(),
            monitoring_screen: MonitoringScreen::new(),
            heap_status_screen: HeapStatusScreen::new(),
            calibration_screens: CalibrationScreens::new(),
            set_date_time_screen: SetDateTimeScreen::new(),
            number_entry_screen: SetNumberScreen::new(
                "Default",
                "X",
                0,
                0,
                1000,
                SettingsAccessorId::MonitoringTargetValue,
            ),

            settings,
            rtc_accessor,
        }
    }

    async fn setup_monitoring_target_value_selection(&mut self) {
        let properties = SettingsAccessorId::MonitoringTargetValue
            .get_numeric_properties()
            .unwrap();
        let value = if let SettingValue::UInt(value) = self
            .settings
            .get_setting(SettingsAccessorId::MonitoringTargetValue)
            .await
            .unwrap_or(SettingValue::UInt(0))
        {
            value
        } else {
            0u32
        };

        let units = if let Some(SettingValue::SmallUInt(monitoring_target_idx)) = self
            .settings
            .get_setting(SettingsAccessorId::MonitoringTargetType)
            .await
        {
            // TODO - fix this, it shouldn't be here - should be part of the stored option, but that got a bit messy
            match settings_menu::monitoring_options::MonitoringTargetPeriodOptions::monitoring_mode_to_storage_option_mapping(monitoring_target_idx as usize) {
                    MonitoringTargetPeriodOptions::Daily => {"ml/day"}
                    MonitoringTargetPeriodOptions::Hourly => {"ml/hour"}
                }
        } else {
            "ml/day"
        };

        self.number_entry_screen = SetNumberScreen::new(
            "Target",
            units,
            value,
            properties.minimum_value,
            properties.maximum_value,
            SettingsAccessorId::MonitoringTargetValue,
        );
    }

    pub async fn set_display_state(&mut self, display_state: ApplicationState) {
        debug!("Display state: {:?}", display_state);

        if let ApplicationState::NumberEntry(setting_id) = display_state {
            match setting_id {
                SettingsAccessorId::MonitoringTargetValue => {
                    self.setup_monitoring_target_value_selection().await
                }
                _ => {}
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
            ApplicationState::Startup => draw_message_screen(&mut self.display, "Starting up..."),

            ApplicationState::ErrorScreenWithMessage(s) => {
                draw_message_screen(&mut self.display, s)
            }

            ApplicationState::TestScreen => self.test_mode_screen.draw(&mut self.display),
            ApplicationState::Settings => self.settings_screen.draw(&mut self.display),
            ApplicationState::Monitoring => self.monitoring_screen.draw(&mut self.display),
            ApplicationState::HeapStatus => self.heap_status_screen.draw(&mut self.display),
            ApplicationState::Calibration => self.calibration_screens.draw(&mut self.display),
            ApplicationState::SetDateTime => self.set_date_time_screen.draw(&mut self.display),
            ApplicationState::NumberEntry(_) => self.number_entry_screen.draw(&mut self.display),
        }

        let _ = self
            .display
            .flush()
            .map_err(|_| error!("Display flush failed"));
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
                self.number_entry_screen
                    .ui_input_handler(input, &self.ui_action_publisher)
                    .await
            }
        }
    }

    pub async fn run(&mut self) {
        self.update_screen().await;
        let mut update_ticker = Ticker::every(Duration::from_millis(200));

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
                        ApplicationMessage::HmiInput(hmi_message) => match hmi_message {
                            HmiMessage::EncoderUpdate(direction) => {
                                trace!("Encoder update: {:?}", Debug2Format(&direction));
                                match direction {
                                    Direction::Clockwise => {
                                        self.route_ui_input(UiInput::EncoderClockwise).await
                                    }
                                    Direction::CounterClockwise => {
                                        self.route_ui_input(UiInput::EncoderCounterClockwise).await
                                    }
                                    Direction::None => {}
                                }
                            }
                            HmiMessage::PushButtonPressed(is_pressed) => {
                                debug!("Button pressed {:?} ", is_pressed);
                                match is_pressed {
                                    true => self.route_ui_input(UiInput::ButtonPress).await,
                                    false => self.route_ui_input(UiInput::ButtonRelease).await,
                                }
                            }
                        },
                        ApplicationMessage::ApplicationStateUpdate(new_state) => {
                            trace!("Set display state");
                            self.set_display_state(new_state).await;
                        }
                        ApplicationMessage::ApplicationDataUpdate(data_update) => {
                            if let ApplicationData::DisplayBrightness(new_display_brightness) =
                                data_update
                            {
                                trace!("Brightness update: {:?}", new_display_brightness);
                                let _ = self
                                    .display
                                    .set_contrast(new_display_brightness)
                                    .map_err(|_| warn!("Failed to set display brightness"));
                                let _ = self
                                    .settings
                                    .save_setting(
                                        SettingsAccessorId::SystemDisplayBrightness,
                                        SettingValue::SmallUInt(new_display_brightness),
                                    )
                                    .await
                                    .map_err(|e| {
                                        warn!(
                                            "Failed to save display brightness: {}",
                                            Debug2Format(&e)
                                        );
                                    });
                            } else {
                                trace!("App data update");
                                self.route_ui_input(UiInput::ApplicationData(data_update))
                                    .await;
                            }
                        }
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
            self.update_screen().await;
        }
    }
}
