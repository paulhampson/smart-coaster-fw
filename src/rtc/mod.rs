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

pub mod accessor;
mod signal;

use crate::rtc::signal::{RTC_SET_TIME, RTC_TIME_UPDATE};
use defmt::{debug, error, trace, Debug2Format};
use ds323x::ic;
use ds323x::interface::I2cInterface;
use ds323x::{DateTimeAccess, Ds323x, NaiveDateTime};
use embassy_rp::i2c::{Async, I2c};
use embassy_rp::peripherals::I2C1;
use embassy_time::{Duration, Ticker};

pub type SystemRtc = Ds323x<I2cInterface<I2c<'static, I2C1, Async>>, ic::DS3231>;

pub struct RtcControl {
    rtc: SystemRtc,
    latest_dt: NaiveDateTime,
}

impl RtcControl {
    pub fn new(mut rtc: SystemRtc) -> Self {
        rtc.use_int_sqw_output_as_interrupt()
            .unwrap_or_else(|e| error!("unable to set RTC interrupt signal: {}", Debug2Format(&e)));
        let latest_dt = rtc.datetime().unwrap_or_default();
        Self { rtc, latest_dt }
    }

    pub async fn run(&mut self) {
        let sender = RTC_TIME_UPDATE.sender();
        let mut one_second_ticker = Ticker::every(Duration::from_secs(1));
        loop {
            one_second_ticker.next().await;
            if let Some(new_time) = RTC_SET_TIME.try_take() {
                self.rtc
                    .set_datetime(&new_time)
                    .unwrap_or_else(|e| error!("unable to set RTC time: {}", Debug2Format(&e)));
                debug!("Time set to {}", Debug2Format(&new_time));
            }
            if let Ok(dt) = self.rtc.datetime() {
                trace!("New RTC time: {:?}", Debug2Format(&dt));
                self.latest_dt = dt;
                sender.send(dt);
            }
        }
    }
}
