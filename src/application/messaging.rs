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

use crate::application::application_state::{ApplicationState, CalibrationStateSubstates};
use crate::drink_monitor::messaging::DrinkMonitoringUpdate;
use crate::hmi::messaging::HmiMessage;
use crate::weight::messaging::WeightRequest;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::pubsub::{PubSubChannel, Publisher, Subscriber};

#[derive(Clone, PartialEq)]
pub enum ApplicationMessage {
    WeighSystemRequest(WeightRequest),
    ApplicationStateUpdate(ApplicationState),
    ApplicationDataUpdate(ApplicationData),
    HmiInput(HmiMessage),
}

#[derive(Clone, PartialEq)]
pub enum ApplicationData {
    Weight(f32),
    CalibrationSubstate(CalibrationStateSubstates),
    HeapStatus { used: usize, free: usize },
    LedBrightness(u8),
    DisplayBrightness(u8),
    DisplayTimeout(u8),
    MonitoringUpdate(DrinkMonitoringUpdate),
}

const CHANNEL_DEPTH: usize = 10;
const CHANNEL_SUBS: usize = 4;
const CHANNEL_PUBS: usize = 4; // (1) application manager, (2) WeighingSystemOverChannel, (3) DrinkMonitoring

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
