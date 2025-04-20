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

use crate::drink_monitor::drink_monitoring::MonitoringStateSubstates;
use crate::hmi::screens::settings_menu::monitoring_options::MonitoringTargetPeriodOptions;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::pubsub::{PubSubChannel, Publisher, Subscriber};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DrinkMonitoringUpdate {
    Consumption(f32),
    ConsumptionRate(f32),
    TotalConsumed(f32),
    TargetRate(f32),
    TargetConsumption(f32),
    TargetMode(MonitoringTargetPeriodOptions),
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
