use crate::storage::historical::accessor::RetrievedLogEntry;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::pubsub::{PubSubChannel, Publisher, Subscriber};

#[derive(Clone)]
pub enum HistoricalLogMessage {
    Error(),
    EndOfRead(),
    Record(RetrievedLogEntry),
}

const CHANNEL_DEPTH: usize = 1;
const CHANNEL_SUBS: usize = 1;
const CHANNEL_PUBS: usize = 1;

pub type HistoricalLogChannel = PubSubChannel<
    CriticalSectionRawMutex,
    HistoricalLogMessage,
    CHANNEL_DEPTH,
    CHANNEL_SUBS,
    CHANNEL_PUBS,
>;
pub type HistoricalLogChannelSubscriber<'a> = Subscriber<
    'a,
    CriticalSectionRawMutex,
    HistoricalLogMessage,
    CHANNEL_DEPTH,
    CHANNEL_SUBS,
    CHANNEL_PUBS,
>;
pub type HistoricalLogChannelPublisher<'a> = Publisher<
    'a,
    CriticalSectionRawMutex,
    HistoricalLogMessage,
    CHANNEL_DEPTH,
    CHANNEL_SUBS,
    CHANNEL_PUBS,
>;
