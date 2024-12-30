use crate::weight::event_channels::{WeighingError, WeightEventChannelReceiver, WeightEventChannelSender, WeightEvents};
use crate::weight::WeighingSystem;
use embassy_futures::select::{select, Either};
use embassy_time::{Duration, Ticker};

/// Acts as a bridge between the pubsub channel and the real weight scale subsystem
pub struct WeighingManager<WS> {
    weight_event_rx: WeightEventChannelReceiver<'static>,
    weight_event_tx: WeightEventChannelSender<'static>,
    weight_scale: WS
}

impl<WS> WeighingManager<WS>
where
    WS: WeighingSystem
{
    pub fn new(weight_event_rx: WeightEventChannelReceiver<'static>, weight_event_tx: WeightEventChannelSender<'static>, weight_scale: WS) -> Self {
        Self {
            weight_event_rx,
            weight_event_tx,
            weight_scale
        }
    }

    pub async fn run(mut self) -> ! {
        let mut periodic_timer = Ticker::every(Duration::from_millis(250));
        loop {
            let request_or_timer = select(self.weight_event_rx.next_message_pure(), periodic_timer.next()).await;
            match request_or_timer {
                Either::First(message) => {
                    self.handle_request(message).await;
                },
                Either::Second(_) => {
                    self.do_measurement().await;
                }
            }
        }
    }

    async fn handle_request(&mut self, request: WeightEvents) {
        match request {
            WeightEvents::RequestStabilisation => {
                match self.weight_scale.stabilize_measurements().await {
                    Ok(..) => self.weight_event_tx.publish_immediate(WeightEvents::RequestCompleted),
                    Err(..) => self.weight_event_tx.publish_immediate(WeightEvents::RequestFailed(WeighingError::StabilisationFailed))
                }
            }
            WeightEvents::RequestTare => {
                match self.weight_scale.tare().await {
                    Ok(..) => self.weight_event_tx.publish_immediate(WeightEvents::RequestCompleted),
                    Err(..) => self.weight_event_tx.publish_immediate(WeightEvents::RequestFailed(WeighingError::TareFailed))
                }
            }
            WeightEvents::RequestCalibrationAtMass(mass) => {
                match self.weight_scale.calibrate(mass).await {
                    Ok(..) => self.weight_event_tx.publish_immediate(WeightEvents::RequestCompleted),
                    Err(..) => self.weight_event_tx.publish_immediate(WeightEvents::RequestFailed(WeighingError::CalibrationFailed))
                }
            }
            WeightEvents::RequestWeight => {
                self.do_measurement().await;
            }
            _ => { },
        }
    }

    async fn do_measurement(&mut self) {
        match self.weight_scale.get_instantaneous_weight_grams().await {
            Ok(weight) => self.weight_event_tx.publish_immediate(WeightEvents::WeightUpdate(weight)),
            Err(..) => self.weight_event_tx.publish_immediate(WeightEvents::RequestFailed(WeighingError::MeasurementFailed))
        }
    }
}