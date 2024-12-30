use embassy_futures::select::{select, Either};
use crate::application::application_state::ProductState;
use crate::hmi::event_channels::HmiEvents::PushButtonPressed;
use crate::hmi::event_channels::{HmiEventChannelReceiver, HmiEventChannelSender, HmiEvents};
use crate::weight::WeighingSystem;
use embassy_time::{Duration, Timer};

pub struct ApplicationManager<WS> {
    hmi_receiver: HmiEventChannelReceiver<'static>,
    hmi_sender: HmiEventChannelSender<'static>,
    weighing_system: WS
}

impl<WS> ApplicationManager<WS>
where
    WS: WeighingSystem,
{
    pub fn new(hmi_rx: HmiEventChannelReceiver<'static>, hmi_tx: HmiEventChannelSender<'static>, weighing_system: WS) -> Self {
        Self {
            hmi_receiver: hmi_rx,
            hmi_sender: hmi_tx,
            weighing_system
        }
    }

    async fn clear_out_hmi_rx(&mut self) {
        while self.hmi_receiver.try_next_message_pure().is_some() {}
    }

    pub async fn run(&mut self) {
        match self.weighing_calibration_sequence().await {
            Ok(_) => {
                self.clear_out_hmi_rx().await;
                self.hmi_sender.publish(HmiEvents::ChangeProductState(ProductState::TestScreen)).await;
            }
            Err(_) => { self.hmi_sender.publish(HmiEvents::ChangeProductState(ProductState::ErrorScreenWithMessage("Scale calibration failed"))).await;}
        }

        loop {
            let hmi_or_weight_message = select(self.hmi_receiver.next_message_pure(), self.weighing_system.get_reading()).await;

            match hmi_or_weight_message {
                Either::First(_) => {}
                Either::Second(weight_reading) => {
                    match weight_reading {
                        Ok(w) => { self.hmi_sender.publish(HmiEvents::WeightUpdate(w)).await; } // just send on weight for now so it updates on screen
                        Err(_) => { self.hmi_sender.publish(HmiEvents::ChangeProductState(ProductState::ErrorScreenWithMessage("Scale reading failed"))).await; }
                    }
                }
            }
        }
    }

    async fn weighing_calibration_sequence(&mut self) -> Result<(), WS::Error> {
        self.hmi_sender.publish(HmiEvents::ChangeProductState(ProductState::Tare)).await;
        while self.hmi_receiver.next_message_pure().await != PushButtonPressed(true) {}
        self.hmi_sender.publish(HmiEvents::ChangeProductState(ProductState::Wait)).await;
        self.weighing_system.stabilize_measurements().await?;
        self.weighing_system.tare().await?;
        // TODO Handle tare/stabilisation failure
        let calibration_mass = 250;
        self.hmi_sender.publish(HmiEvents::ChangeProductState(ProductState::Calibration(calibration_mass))).await;
        while self.hmi_receiver.next_message_pure().await != PushButtonPressed(true) {}
        self.hmi_sender.publish(HmiEvents::ChangeProductState(ProductState::Wait)).await;
        self.weighing_system.calibrate(calibration_mass as f32).await?;
        // TODO Handle calibration failure
        self.hmi_sender.publish(HmiEvents::ChangeProductState(ProductState::CalibrationDone)).await;
        Timer::after(Duration::from_secs(2)).await;
        Ok(())
    }
}
