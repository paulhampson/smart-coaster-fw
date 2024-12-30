use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::pubsub::{PubSubChannel, Publisher, Subscriber};
use crate::weight::WeighingSystem;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WeighingError {
    StabilisationFailed,
    TareFailed,
    CalibrationFailed,
    MeasurementFailed,
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum WeightEvents {
    RequestStabilisation,
    RequestTare,
    RequestCalibrationAtMass(f32),
    RequestCompleted,
    RequestWeight,
    WeightUpdate(f32),
    RequestFailed(WeighingError)
}

const CHANNEL_DEPTH: usize = 10;
const CHANNEL_SUBS: usize = 2;
const CHANNEL_PUBS: usize = 2;

pub type WeightEventChannel = PubSubChannel<CriticalSectionRawMutex, WeightEvents, CHANNEL_DEPTH, CHANNEL_SUBS, CHANNEL_PUBS>;
pub type WeightEventChannelReceiver<'a> = Subscriber<'a, CriticalSectionRawMutex, WeightEvents, CHANNEL_DEPTH, CHANNEL_SUBS, CHANNEL_PUBS>;
pub type WeightEventChannelSender<'a> = Publisher<'a, CriticalSectionRawMutex, WeightEvents, CHANNEL_DEPTH, CHANNEL_SUBS, CHANNEL_PUBS>;

pub struct WeighingSystemOverChannel {
    weight_event_rx: WeightEventChannelReceiver<'static>,
    weight_event_tx: WeightEventChannelSender<'static>
}

impl WeighingSystemOverChannel {
    pub(crate) fn new(weight_event_rx: WeightEventChannelReceiver<'static>, weight_event_tx: WeightEventChannelSender<'static>) -> Self {
        Self {
            weight_event_tx,
            weight_event_rx
        }
    }

    async fn get_result(&mut self) -> Result<(), WeighingError> {
        loop {
            match self.weight_event_rx.next_message_pure().await {
                WeightEvents::RequestCompleted => {return Ok(());}
                WeightEvents::RequestFailed(e) => {return Err(e);}
                _ => {}
            }
        }
    }

    async fn get_weight(&mut self) -> Result<f32, WeighingError> {
        loop {
            match self.weight_event_rx.next_message_pure().await {
                WeightEvents::WeightUpdate(w) => {return Ok(w);}
                WeightEvents::RequestFailed(e) => {return Err(e);}
                _ => {}
            }
        }
    }
}

impl WeighingSystem for WeighingSystemOverChannel {
    type Error = WeighingError;

    async fn stabilize_measurements(&mut self) -> Result<(), Self::Error> {
        self.weight_event_tx.publish_immediate(WeightEvents::RequestStabilisation);
        self.get_result().await
    }

    async fn tare(&mut self) -> Result<(), Self::Error> {
        self.weight_event_tx.publish_immediate(WeightEvents::RequestTare);
        self.get_result().await
    }

    async fn calibrate(&mut self, calibration_mass: f32) -> Result<(), Self::Error> {
        self.weight_event_tx.publish_immediate(WeightEvents::RequestCalibrationAtMass(calibration_mass));
        self.get_result().await
    }

    async fn get_instantaneous_weight_grams(&mut self) -> Result<f32, Self::Error> {
        self.weight_event_tx.publish_immediate(WeightEvents::RequestWeight);
        self.get_weight().await
    }

    async fn get_reading(&mut self) -> Result<f32, Self::Error> {
        self.get_weight().await
    }
}