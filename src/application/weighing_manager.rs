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

use crate::application::messaging::{ApplicationChannelSubscriber, ApplicationMessage};
use crate::weight::messaging::{
    WeighingError, WeightChannelPublisher, WeightEvents, WeightRequest,
};
use crate::weight::WeighingSystem;
use defmt::warn;
use embassy_futures::select::{select, Either};
use embassy_sync::pubsub::WaitResult;
use embassy_time::{Duration, Ticker};

/// Acts as a bridge between the pubsub channel and the real weight scale subsystem
pub struct WeighingManager<WS> {
    app_channel_subscriber: ApplicationChannelSubscriber<'static>,
    weight_channel_publisher: WeightChannelPublisher<'static>,
    weight_scale: WS,
}

impl<WS> WeighingManager<WS>
where
    WS: WeighingSystem,
{
    pub fn new(
        app_channel_subscriber: ApplicationChannelSubscriber<'static>,
        weight_channel_publisher: WeightChannelPublisher<'static>,
        weight_scale: WS,
    ) -> Self {
        Self {
            app_channel_subscriber,
            weight_channel_publisher,
            weight_scale,
        }
    }

    pub async fn run(&mut self) -> ! {
        let mut periodic_timer = Ticker::every(Duration::from_millis(250));
        loop {
            let request_or_timer = select(
                self.app_channel_subscriber.next_message(),
                periodic_timer.next(),
            )
            .await;
            match request_or_timer {
                Either::First(message) => match message {
                    WaitResult::Message(message) => {
                        if let ApplicationMessage::WeighSystemRequest(weight_request) = message {
                            self.handle_request(weight_request).await;
                        }
                    }
                    WaitResult::Lagged(missed) => {
                        warn!("Missed {} messages", missed);
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
                    Ok(..) => self.weight_channel_publisher.publish_immediate(
                        WeightEvents::RequestCompleted(WeightRequest::Stabilisation),
                    ),
                    Err(..) => self.weight_channel_publisher.publish_immediate(
                        WeightEvents::RequestFailed(WeighingError::StabilisationFailed),
                    ),
                }
            }
            WeightRequest::Tare => match self.weight_scale.tare().await {
                Ok(..) => self
                    .weight_channel_publisher
                    .publish_immediate(WeightEvents::RequestCompleted(WeightRequest::Tare)),
                Err(..) => self
                    .weight_channel_publisher
                    .publish_immediate(WeightEvents::RequestFailed(WeighingError::TareFailed)),
            },
            WeightRequest::CalibrationAtMass(mass) => {
                match self.weight_scale.calibrate(mass).await {
                    Ok(..) => self.weight_channel_publisher.publish_immediate(
                        WeightEvents::RequestCompleted(WeightRequest::CalibrationAtMass(mass)),
                    ),
                    Err(..) => self.weight_channel_publisher.publish_immediate(
                        WeightEvents::RequestFailed(WeighingError::CalibrationFailed),
                    ),
                }
            }
            WeightRequest::Weight => {
                self.do_measurement().await;
            }
        }
    }

    async fn do_measurement(&mut self) {
        match self.weight_scale.get_instantaneous_weight_grams().await {
            Ok(weight) => self
                .weight_channel_publisher
                .publish_immediate(WeightEvents::WeightUpdate(weight)),
            Err(..) => {
                self.weight_channel_publisher
                    .publish_immediate(WeightEvents::RequestFailed(
                        WeighingError::MeasurementFailed,
                    ))
            }
        }
    }
}
