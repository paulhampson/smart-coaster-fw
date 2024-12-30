use crate::hmi::rotary_encoder::Direction;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::pubsub::{PubSubChannel, Publisher, Subscriber};

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum HmiMessage {
    EncoderUpdate(Direction),
    PushButtonPressed(bool),
}

const CHANNEL_DEPTH: usize = 10;
const CHANNEL_SUBS: usize = 1;
const CHANNEL_PUBS: usize = 1;

pub type HmiChannel = PubSubChannel<CriticalSectionRawMutex, HmiMessage, CHANNEL_DEPTH, CHANNEL_SUBS, CHANNEL_PUBS>;
pub type HmiChannelSubscriber<'a> = Subscriber<'a, CriticalSectionRawMutex, HmiMessage, CHANNEL_DEPTH, CHANNEL_SUBS, CHANNEL_PUBS>;
pub type HmiChannelPublisher<'a> = Publisher<'a, CriticalSectionRawMutex, HmiMessage, CHANNEL_DEPTH, CHANNEL_SUBS, CHANNEL_PUBS>;