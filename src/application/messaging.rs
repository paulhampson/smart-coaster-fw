use crate::application::application_state::ApplicationState;
use crate::weight::messaging::WeightRequest;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::pubsub::{PubSubChannel, Publisher, Subscriber};
use crate::hmi::messaging::HmiMessage;

#[derive(Clone)]
pub enum ApplicationMessage {
    WeighSystemRequest(WeightRequest),
    ApplicationStateUpdate(ApplicationState),
    ApplicationDataUpdate(ApplicationData),
    HmiInput(HmiMessage)
}

#[derive(Clone)]
pub enum ApplicationData {
    Weight(f32)
}

const CHANNEL_DEPTH: usize = 10;
const CHANNEL_SUBS: usize = 3;
const CHANNEL_PUBS: usize = 2; // One for the actual application manager and one for WeighingSystemOverChannel

pub type ApplicationChannel = PubSubChannel<CriticalSectionRawMutex, ApplicationMessage, CHANNEL_DEPTH, CHANNEL_SUBS, CHANNEL_PUBS>;
pub type ApplicationChannelSubscriber<'a> = Subscriber<'a, CriticalSectionRawMutex, ApplicationMessage, CHANNEL_DEPTH, CHANNEL_SUBS, CHANNEL_PUBS>;
pub type ApplicationChannelPublisher<'a> = Publisher<'a, CriticalSectionRawMutex, ApplicationMessage, CHANNEL_DEPTH, CHANNEL_SUBS, CHANNEL_PUBS>;