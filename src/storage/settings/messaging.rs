use crate::storage::settings::{SettingValue, SettingsAccessorId};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::pubsub::{PubSubChannel, Publisher, Subscriber};

#[derive(Clone)]
pub enum SettingsMessage {
    Change(SettingData),
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub struct SettingData {
    pub setting_id: SettingsAccessorId,
    pub value: SettingValue,
}

const CHANNEL_DEPTH: usize = 10;
const CHANNEL_SUBS: usize = 2;
const CHANNEL_PUBS: usize = 1;

pub static SETTINGS_CHANNEL: SettingsChannel = PubSubChannel::new();

pub type SettingsChannel = PubSubChannel<
    CriticalSectionRawMutex,
    SettingsMessage,
    CHANNEL_DEPTH,
    CHANNEL_SUBS,
    CHANNEL_PUBS,
>;
pub type SettingsChannelSubscriber<'a> = Subscriber<
    'a,
    CriticalSectionRawMutex,
    SettingsMessage,
    CHANNEL_DEPTH,
    CHANNEL_SUBS,
    CHANNEL_PUBS,
>;
pub type SettingsChannelPublisher<'a> = Publisher<
    'a,
    CriticalSectionRawMutex,
    SettingsMessage,
    CHANNEL_DEPTH,
    CHANNEL_SUBS,
    CHANNEL_PUBS,
>;
