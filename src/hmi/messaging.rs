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

use crate::application::application_state::ApplicationState;
use crate::hmi::rotary_encoder::Direction;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::pubsub::{PubSubChannel, Publisher, Subscriber};

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum HmiMessage {
    EncoderUpdate(Direction),
    PushButtonPressed(bool),
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum UiRequestMessage {
    ChangeState(ApplicationState),
    ChangeLedBrightness(u8),
    ChangeDisplayBrightness(u8),
    ChangeDisplayTimeout(u8),
    ClearHistoricalConsumptionLog(),
}

const CHANNEL_DEPTH: usize = 10;
const CHANNEL_SUBS: usize = 2;
const CHANNEL_PUBS: usize = 1;

pub type HmiChannel =
    PubSubChannel<CriticalSectionRawMutex, HmiMessage, CHANNEL_DEPTH, CHANNEL_SUBS, CHANNEL_PUBS>;
pub type HmiChannelSubscriber<'a> =
    Subscriber<'a, CriticalSectionRawMutex, HmiMessage, CHANNEL_DEPTH, CHANNEL_SUBS, CHANNEL_PUBS>;
pub type HmiChannelPublisher<'a> =
    Publisher<'a, CriticalSectionRawMutex, HmiMessage, CHANNEL_DEPTH, CHANNEL_SUBS, CHANNEL_PUBS>;

pub type UiActionChannel = PubSubChannel<
    CriticalSectionRawMutex,
    UiRequestMessage,
    CHANNEL_DEPTH,
    CHANNEL_SUBS,
    CHANNEL_PUBS,
>;
pub type UiActionChannelSubscriber<'a> = Subscriber<
    'a,
    CriticalSectionRawMutex,
    UiRequestMessage,
    CHANNEL_DEPTH,
    CHANNEL_SUBS,
    CHANNEL_PUBS,
>;
pub type UiActionChannelPublisher<'a> = Publisher<
    'a,
    CriticalSectionRawMutex,
    UiRequestMessage,
    CHANNEL_DEPTH,
    CHANNEL_SUBS,
    CHANNEL_PUBS,
>;
