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

use ds323x::NaiveDateTime;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;
use embassy_sync::watch::{Receiver, Watch};

const RTC_WATCH_RECEIVER_COUNT: usize = 5;
pub type RtcWatchReceiver = Receiver<'static, CriticalSectionRawMutex, NaiveDateTime, RTC_WATCH_RECEIVER_COUNT>;
pub static RTC_TIME_UPDATE: Watch<CriticalSectionRawMutex, NaiveDateTime, RTC_WATCH_RECEIVER_COUNT> = Watch::new();
pub static RTC_SET_TIME: Signal<CriticalSectionRawMutex, NaiveDateTime> = Signal::new();