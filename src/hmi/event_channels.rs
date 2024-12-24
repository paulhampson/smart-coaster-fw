use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::{Channel, Receiver, Sender};
use crate::hmi::rotary_encoder::Direction;

#[derive(Debug)]
pub enum HmiEvents {
    EncoderUpdate(Direction),
    PushButtonPressed(bool),
    WeightUpdate(u32),
}

pub type HmiEventChannel = Channel<CriticalSectionRawMutex, HmiEvents, 5>;
pub type HmiEventChannelReceiver = Receiver<'static, CriticalSectionRawMutex, HmiEvents, 5>;
pub type HmiEventChannelSender = Sender<'static, CriticalSectionRawMutex, HmiEvents, 5>;
