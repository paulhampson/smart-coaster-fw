use crate::hmi::rotary_encoder::Direction;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::pubsub::{PubSubChannel, Publisher, Subscriber};
use crate::application::application_state::ProductState;

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum HmiEvents {
    ChangeProductState(ProductState),
    EncoderUpdate(Direction),
    PushButtonPressed(bool),
    WeightUpdate(f32),
}

const CHANNEL_DEPTH: usize = 10;
const CHANNEL_SUBS: usize = 4;
const CHANNEL_PUBS: usize = 2;

pub type HmiEventChannel = PubSubChannel<CriticalSectionRawMutex, HmiEvents, CHANNEL_DEPTH, CHANNEL_SUBS, CHANNEL_PUBS>;
pub type HmiEventChannelReceiver<'a> = Subscriber<'a, CriticalSectionRawMutex, HmiEvents, CHANNEL_DEPTH, CHANNEL_SUBS, CHANNEL_PUBS>;
pub type HmiEventChannelSender<'a> = Publisher<'a, CriticalSectionRawMutex, HmiEvents, CHANNEL_DEPTH, CHANNEL_SUBS, CHANNEL_PUBS>;
