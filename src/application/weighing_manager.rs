use crate::application::messaging::{ApplicationChannelSubscriber, ApplicationMessage};
use crate::weight::messaging::{WeighingError, WeightChannelPublisher, WeightEvents, WeightRequest};
use crate::weight::WeighingSystem;
use embassy_futures::select::{select, Either};
use embassy_time::{Duration, Ticker};

/// Acts as a bridge between the pubsub channel and the real weight scale subsystem
pub struct WeighingManager<WS> {
    app_channel_subscriber: ApplicationChannelSubscriber<'static>,
    weight_channel_publisher: WeightChannelPublisher<'static>,
    weight_scale: WS
}

impl<WS> WeighingManager<WS>
where
    WS: WeighingSystem
{
    pub fn new(app_channel_subscriber: ApplicationChannelSubscriber<'static>, weight_channel_publisher: WeightChannelPublisher<'static>, weight_scale: WS) -> Self {
        Self {
            app_channel_subscriber,
            weight_channel_publisher,
            weight_scale
        }
    }

    pub async fn run(mut self) -> ! {
        let mut periodic_timer = Ticker::every(Duration::from_millis(250));
        loop {
            let request_or_timer = select(self.app_channel_subscriber.next_message_pure(), periodic_timer.next()).await;
            match request_or_timer {
                Either::First(message) => {
                    match message {
                        ApplicationMessage::WeighSystemRequest(weight_request) => { self.handle_request(weight_request).await; }
                        _ => {}
                    }
                },
                Either::Second(_) => {
                    self.do_measurement().await;
                }
            }
        }
    }

    async fn handle_request(&mut self, request: WeightRequest) {
        match request {
            WeightRequest::Stabilisation => {
                match self.weight_scale.stabilize_measurements().await {
                    Ok(..) => self.weight_channel_publisher.publish_immediate(WeightEvents::RequestCompleted(WeightRequest::Stabilisation)),
                    Err(..) => self.weight_channel_publisher.publish_immediate(WeightEvents::RequestFailed(WeighingError::StabilisationFailed))
                }
            }
            WeightRequest::Tare => {
                match self.weight_scale.tare().await {
                    Ok(..) => self.weight_channel_publisher.publish_immediate(WeightEvents::RequestCompleted(WeightRequest::Tare)),
                    Err(..) => self.weight_channel_publisher.publish_immediate(WeightEvents::RequestFailed(WeighingError::TareFailed))
                }
            }
            WeightRequest::CalibrationAtMass(mass) => {
                match self.weight_scale.calibrate(mass).await {
                    Ok(..) => self.weight_channel_publisher.publish_immediate(WeightEvents::RequestCompleted(WeightRequest::CalibrationAtMass(mass))),
                    Err(..) => self.weight_channel_publisher.publish_immediate(WeightEvents::RequestFailed(WeighingError::CalibrationFailed))
                }
            }
            WeightRequest::Weight => {
                self.do_measurement().await;
            }
        }
    }

    async fn do_measurement(&mut self) {
        match self.weight_scale.get_instantaneous_weight_grams().await {
            Ok(weight) => self.weight_channel_publisher.publish_immediate(WeightEvents::WeightUpdate(weight)),
            Err(..) => self.weight_channel_publisher.publish_immediate(WeightEvents::RequestFailed(WeighingError::MeasurementFailed))
        }
    }
}