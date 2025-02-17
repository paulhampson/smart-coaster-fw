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

use defmt::warn;
use crate::rtc::signal::{RtcWatchReceiver, RTC_SET_TIME, RTC_TIME_UPDATE};
use ds323x::NaiveDateTime;

pub enum RtcAccessorError {
    NoWatchSlotsAvailable,
}

pub struct RtcAccessor {
    receiver: RtcWatchReceiver,
    recent_dt: NaiveDateTime,
}

impl RtcAccessor {
    pub fn new() -> Result<RtcAccessor, RtcAccessorError> {
        if let Some(receiver) = RTC_TIME_UPDATE.receiver() {
            Ok(Self {
                receiver,
                recent_dt: NaiveDateTime::default(),
            })
        } else {
            warn!("Unable to obtain RTC timer update receiver");
            Err(RtcAccessorError::NoWatchSlotsAvailable)
        }
    }

    pub async fn wait_for_next_second(&mut self) -> NaiveDateTime {
        self.recent_dt = self.receiver.changed().await;
        self.recent_dt
    }

    pub fn get_date_time(&mut self) -> NaiveDateTime {
        if let Some(dt) = self.receiver.try_get() {
            self.recent_dt = dt;
        }
        self.recent_dt
    }
}

pub fn set_date_time(dt: NaiveDateTime) {
    RTC_SET_TIME.signal(dt);
}