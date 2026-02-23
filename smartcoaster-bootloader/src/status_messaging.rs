// Copyright (C) 2026 Paul Hampson
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

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::pubsub::{PubSubChannel, Publisher, Subscriber};

#[derive(Clone, Debug, PartialEq)]
pub struct TransferProgress {
    pub transferred_chunks: usize,
    pub total_chunks: usize,
}

#[derive(Clone, PartialEq, Debug)]
pub enum BootloaderStatusMessage {
    Starting,
    HoldButton,
    WaitingForFirmware,
    TransferringFirmware(TransferProgress),
    FirmwareInstalling,
    StartingApplication,
}

const CHANNEL_DEPTH: usize = 1;
const CHANNEL_SUBS: usize = 1;
const CHANNEL_PUBS: usize = 2;

pub type BootloaderStatusChannel = PubSubChannel<
    CriticalSectionRawMutex,
    BootloaderStatusMessage,
    CHANNEL_DEPTH,
    CHANNEL_SUBS,
    CHANNEL_PUBS,
>;
pub type BootloaderStatusChannelSubscriber<'a> = Subscriber<
    'a,
    CriticalSectionRawMutex,
    BootloaderStatusMessage,
    CHANNEL_DEPTH,
    CHANNEL_SUBS,
    CHANNEL_PUBS,
>;
pub type BootloaderStatusChannelPublisher<'a> = Publisher<
    'a,
    CriticalSectionRawMutex,
    BootloaderStatusMessage,
    CHANNEL_DEPTH,
    CHANNEL_SUBS,
    CHANNEL_PUBS,
>;
