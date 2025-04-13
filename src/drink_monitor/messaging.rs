use crate::drink_monitor::drink_monitoring::MonitoringStateSubstates;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::pubsub::{PubSubChannel, Publisher, Subscriber};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DrinkMonitoringUpdate {
    Consumption(f32),
    ConsumptionRate(f32),
    TotalConsumed(f32),
    UpdateMonitoringSubstate(MonitoringStateSubstates),
}

const CHANNEL_DEPTH: usize = 10;
const CHANNEL_SUBS: usize = 1;
const CHANNEL_PUBS: usize = 1;

pub type DrinkMonitorChannel = PubSubChannel<
    CriticalSectionRawMutex,
    DrinkMonitoringUpdate,
    CHANNEL_DEPTH,
    CHANNEL_SUBS,
    CHANNEL_PUBS,
>;
pub type DrinkMonitorChannelSubscriber<'a> = Subscriber<
    'a,
    CriticalSectionRawMutex,
    DrinkMonitoringUpdate,
    CHANNEL_DEPTH,
    CHANNEL_SUBS,
    CHANNEL_PUBS,
>;
pub type DrinkMonitorChannelPublisher<'a> = Publisher<
    'a,
    CriticalSectionRawMutex,
    DrinkMonitoringUpdate,
    CHANNEL_DEPTH,
    CHANNEL_SUBS,
    CHANNEL_PUBS,
>;
