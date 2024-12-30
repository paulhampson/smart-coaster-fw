use crate::application::application_state::ApplicationState;
use crate::application::messaging::{ApplicationChannelPublisher, ApplicationData, ApplicationMessage};
use crate::hmi::messaging::HmiMessage::PushButtonPressed;
use crate::hmi::messaging::HmiChannelSubscriber;
use crate::weight::WeighingSystem;
use embassy_futures::select::{select, Either};
use embassy_time::{Duration, Instant, Timer};

pub struct ApplicationManager<WS> {
    hmi_subscriber: HmiChannelSubscriber<'static>,
    app_publisher: ApplicationChannelPublisher<'static>,
    weighing_system: WS
}

impl<WS> ApplicationManager<WS>
where
    WS: WeighingSystem,
{
    const LONG_LONG_PRESS_TIME: Duration = Duration::from_secs(2);

    pub fn new(hmi_rx: HmiChannelSubscriber<'static>, app_publisher: ApplicationChannelPublisher<'static>, weighing_system: WS) -> Self {
        Self {
            hmi_subscriber: hmi_rx,
            app_publisher,
            weighing_system
        }
    }

    async fn clear_out_hmi_rx(&mut self) {
        while self.hmi_subscriber.try_next_message_pure().is_some() {}
    }

    async fn manage_error(&mut self, message: &'static str) -> ! {
        self.app_publisher.publish(ApplicationMessage::ApplicationStateUpdate(ApplicationState::ErrorScreenWithMessage(message))).await;
        loop {
            Timer::after(Duration::from_secs(1)).await;
        }
    }

    async fn update_application_state(&mut self, state: ApplicationState) {
        self.app_publisher.publish(ApplicationMessage::ApplicationStateUpdate(state)).await;
    }

    async fn wait_for_button(&mut self) {
        while self.hmi_subscriber.next_message_pure().await != PushButtonPressed(true) {}
    }

    pub async fn run(&mut self) {
        match self.weighing_calibration_sequence().await {
            Ok(_) => {
                self.clear_out_hmi_rx().await;

            }
            Err(_) => { self.manage_error("Scale calibration failed").await;}
        }

        self.test_screen(ApplicationState::WaitingForActivity).await;

        loop {
            Timer::after(Duration::from_secs(1)).await;
        }
    }

    async fn weighing_calibration_sequence(&mut self) -> Result<(), WS::Error> {
        self.update_application_state(ApplicationState::Tare).await;
        self.wait_for_button().await;
        self.update_application_state(ApplicationState::Wait).await;
        self.weighing_system.stabilize_measurements().await?;
        self.weighing_system.tare().await?;
        let calibration_mass = 500;
        self.update_application_state(ApplicationState::Calibration(calibration_mass)).await;
        self.wait_for_button().await;
        self.update_application_state(ApplicationState::Wait).await;
        self.weighing_system.calibrate(calibration_mass as f32).await?;
        self.update_application_state(ApplicationState::CalibrationDone).await;
        Timer::after(Duration::from_secs(2)).await;
        Ok(())
    }

    async fn test_screen(&mut self, exit_to: ApplicationState) {
        self.update_application_state(ApplicationState::TestScreen).await;
        let mut press_start = Instant::MIN;
        loop {
            let hmi_or_weight_message = select(self.hmi_subscriber.next_message_pure(), self.weighing_system.get_reading()).await;

            match hmi_or_weight_message {
                Either::First(hmi_message) => {
                    self.app_publisher.publish(ApplicationMessage::HmiInput(hmi_message)).await;
                    match hmi_message {
                        PushButtonPressed(true) => { press_start = Instant::now(); }
                        PushButtonPressed(false) => {
                            if press_start.elapsed() >= Self::LONG_LONG_PRESS_TIME {break;}
                        }
                        _ => {}
                    }
                }
                Either::Second(weight_reading) => {
                    match weight_reading {
                        Ok(w) => { self.app_publisher.publish(ApplicationMessage::ApplicationDataUpdate(ApplicationData::Weight(w))).await; } // just send on weight for now so it updates on screen
                        Err(_) => { self.manage_error("Scale reading failed").await }
                    }
                }
            }
        }
        self.update_application_state(exit_to).await;
    }
}
