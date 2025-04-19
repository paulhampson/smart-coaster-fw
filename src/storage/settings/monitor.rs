use crate::storage::settings::messaging::{
    SettingsChannelSubscriber, SettingsMessage, SETTINGS_CHANNEL,
};

pub struct FlashSettingsMonitor {
    settings_subscriber: SettingsChannelSubscriber<'static>,
}

impl FlashSettingsMonitor {
    pub fn new() -> Self {
        Self {
            settings_subscriber: SETTINGS_CHANNEL.subscriber().unwrap(),
        }
    }

    pub async fn listen_for_changes_ignore_lag(&mut self) -> SettingsMessage {
        self.settings_subscriber.next_message_pure().await
    }
}
