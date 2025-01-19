use crate::application::application_state::ApplicationState;
use crate::application::messaging::{ApplicationChannelSubscriber, ApplicationMessage};
use crate::hmi::messaging::{HmiMessage, UiActionChannelPublisher};
use crate::hmi::rotary_encoder::Direction;
use crate::hmi::screens::calibration::CalibrationScreens;
use crate::hmi::screens::heap_status::HeapStatusScreen;
use crate::hmi::screens::monitoring::MonitoringScreen;
use crate::hmi::screens::settings::SettingMenu;
use crate::hmi::screens::test_mode::TestModeScreen;
use crate::hmi::screens::{draw_message_screen, UiDrawer, UiInput, UiInputHandler};
use core::fmt::Write;
use defmt::{debug, error, trace, warn};
use embassy_futures::select::{select, Either};
use embassy_sync::pubsub::WaitResult;
use embassy_time::{Duration, Instant, Ticker};
use heapless::String;
use sh1106::mode::GraphicsMode;

pub struct DisplayManager<DI>
where
    DI: sh1106::interface::DisplayInterface,
{
    app_channel_subscriber: ApplicationChannelSubscriber<'static>,
    ui_action_publisher: UiActionChannelPublisher<'static>,
    display: GraphicsMode<DI>,

    display_state: ApplicationState,
    last_display_update: Instant,

    settings_screen: SettingMenu,
    test_mode_screen: TestModeScreen,
    monitoring_screen: MonitoringScreen,
    heap_status_screen: HeapStatusScreen,
    calibration_screens: CalibrationScreens,
}

impl<DI> DisplayManager<DI>
where
    DI: sh1106::interface::DisplayInterface,
{
    const FRAME_TIMING_MS: u32 = 1000 / 30;

    pub fn new(
        mut display: GraphicsMode<DI>,
        app_channel_subscriber: ApplicationChannelSubscriber<'static>,
        ui_action_publisher: UiActionChannelPublisher<'static>,
    ) -> Self {
        let _ = display.init().map_err(|_| error!("Failed to init display"));
        let _ = display
            .flush()
            .map_err(|_| error!("Failed to flush display"));

        Self {
            app_channel_subscriber,
            ui_action_publisher,
            display,

            display_state: ApplicationState::Startup,
            last_display_update: Instant::MIN,

            settings_screen: SettingMenu::new(),
            test_mode_screen: TestModeScreen::new(),
            monitoring_screen: MonitoringScreen::new(),
            heap_status_screen: HeapStatusScreen::new(),
            calibration_screens: CalibrationScreens::new(),
        }
    }

    pub async fn set_display_state(&mut self, display_state: ApplicationState) {
        debug!("Display state: {:?}", display_state);
        self.display_state = display_state;
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

            ApplicationState::Wait => self.draw_wait_screen(),
            ApplicationState::ErrorScreenWithMessage(s) => {
                draw_message_screen(&mut self.display, s)
            }

            ApplicationState::TestScreen => self.test_mode_screen.draw(&mut self.display),
            ApplicationState::Settings => self.settings_screen.draw(&mut self.display),
            ApplicationState::Monitoring => self.monitoring_screen.draw(&mut self.display),
            ApplicationState::HeapStatus => self.heap_status_screen.draw(&mut self.display),
            ApplicationState::Calibration => self.calibration_screens.draw(&mut self.display),
        }

        let _ = self
            .display
            .flush()
            .map_err(|_| error!("Display flush failed"));
        self.last_display_update = Instant::now();
    }

    fn draw_wait_screen(&mut self) {
        draw_message_screen(&mut self.display, "Please wait...");
    }

    fn route_ui_input(&mut self, input: UiInput) {
        match self.display_state {
            ApplicationState::Startup => {}
            ApplicationState::Wait => {}
            ApplicationState::ErrorScreenWithMessage(_) => {}

            ApplicationState::Settings => self
                .settings_screen
                .ui_input_handler(input, &self.ui_action_publisher),
            ApplicationState::TestScreen => self
                .test_mode_screen
                .ui_input_handler(input, &self.ui_action_publisher),
            ApplicationState::Monitoring => self
                .monitoring_screen
                .ui_input_handler(input, &self.ui_action_publisher),
            ApplicationState::HeapStatus => self
                .heap_status_screen
                .ui_input_handler(input, &self.ui_action_publisher),
            ApplicationState::Calibration => self
                .calibration_screens
                .ui_input_handler(input, &self.ui_action_publisher),
        }
    }

    pub async fn run(&mut self) {
        self.update_screen().await;
        let mut update_ticker = Ticker::every(Duration::from_millis(200));

        loop {
            let wait_result = select(
                self.app_channel_subscriber.next_message(),
                update_ticker.next(),
            )
            .await;
            match wait_result {
                Either::First(w) => match w {
                    WaitResult::Lagged(count) => {
                        warn! {"Display lost {} messages from HMI channel", count}
                    }
                    WaitResult::Message(message) => match message {
                        ApplicationMessage::HmiInput(hmi_message) => match hmi_message {
                            HmiMessage::EncoderUpdate(direction) => {
                                trace!("Encoder update: {:?}", direction);
                                match direction {
                                    Direction::Clockwise => {
                                        self.route_ui_input(UiInput::EncoderClockwise)
                                    }
                                    Direction::CounterClockwise => {
                                        self.route_ui_input(UiInput::EncoderCounterClockwise)
                                    }
                                    Direction::None => {}
                                }
                            }
                            HmiMessage::PushButtonPressed(is_pressed) => {
                                debug!("Button pressed {:?} ", is_pressed);
                                match is_pressed {
                                    true => self.route_ui_input(UiInput::ButtonPress),
                                    false => self.route_ui_input(UiInput::ButtonRelease),
                                }
                            }
                        },
                        ApplicationMessage::ApplicationStateUpdate(new_state) => {
                            trace!("Set display state");
                            self.set_display_state(new_state).await;
                        }
                        ApplicationMessage::ApplicationDataUpdate(data_update) => {
                            trace!("App data update");
                            self.route_ui_input(UiInput::ApplicationData(data_update));
                        }
                        ApplicationMessage::WeighSystemRequest(..) => {}
                    },
                },
                Either::Second(_) => {}
            }
            self.update_screen().await;
        }
    }
}
