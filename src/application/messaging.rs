use crate::application::application_state::{
    ApplicationState, CalibrationStateSubstates, MonitoringStateSubstates,
};
use crate::hmi::messaging::HmiMessage;
use crate::weight::messaging::WeightRequest;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::pubsub::{PubSubChannel, Publisher, Subscriber};

#[derive(Clone)]
pub enum ApplicationMessage {
    WeighSystemRequest(WeightRequest),
    ApplicationStateUpdate(ApplicationState),
    ApplicationDataUpdate(ApplicationData),
    HmiInput(HmiMessage),
}

#[derive(Clone)]
pub enum ApplicationData {
    Weight(f32),
    Consumption(f32),
    ConsumptionRate(f32),
    TotalConsumed(f32),
    MonitoringSubstate(MonitoringStateSubstates),
    CalibrationSubstate(CalibrationStateSubstates),
    HeapStatus { used: usize, free: usize },
    LedBrightness(u8),
    DisplayBrightness(u8),
    DisplayTimeout(u8),
}

const CHANNEL_DEPTH: usize = 10;
const CHANNEL_SUBS: usize = 3;
const CHANNEL_PUBS: usize = 2; // One for the actual application manager and one for WeighingSystemOverChannel

pub type ApplicationChannel = PubSubChannel<
    CriticalSectionRawMutex,
    ApplicationMessage,
    CHANNEL_DEPTH,
    CHANNEL_SUBS,
    CHANNEL_PUBS,
>;
pub type ApplicationChannelSubscriber<'a> = Subscriber<
    'a,
    CriticalSectionRawMutex,
    ApplicationMessage,
    CHANNEL_DEPTH,
    CHANNEL_SUBS,
    CHANNEL_PUBS,
>;
pub type ApplicationChannelPublisher<'a> = Publisher<
    'a,
    CriticalSectionRawMutex,
    ApplicationMessage,
    CHANNEL_DEPTH,
    CHANNEL_SUBS,
    CHANNEL_PUBS,
>;
