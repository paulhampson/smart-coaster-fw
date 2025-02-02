pub mod accessor;
mod signal;

use defmt::{error, trace, Debug2Format};
use ds323x::{DateTimeAccess, Ds323x, NaiveDateTime};
use ds323x::ic;
use ds323x::interface::I2cInterface;
use embassy_rp::i2c::{Async, I2c};
use embassy_rp::peripherals::I2C1;
use embassy_time::{Duration, Ticker};
use crate::rtc::signal::RTC_TIME_UPDATE;

pub type SystemRtc = Ds323x<I2cInterface<I2c<'static, I2C1, Async>>, ic::DS3231>;

pub struct RtcControl{
    rtc: SystemRtc,
    latest_dt: NaiveDateTime
}

impl RtcControl {
    pub fn new(mut rtc: SystemRtc) -> Self {
        rtc.use_int_sqw_output_as_interrupt().unwrap_or_else(|e| error!("unable to set RTC interrupt signal: {}", Debug2Format(&e)));
        let latest_dt = rtc.datetime().unwrap_or(NaiveDateTime::default());
        Self { rtc, latest_dt }
    }

    pub async fn run(&mut self) {
        let sender = RTC_TIME_UPDATE.sender();
        let mut one_second_ticker = Ticker::every(Duration::from_secs(1));
        loop {
            one_second_ticker.next().await;
            if let Ok(dt) = self.rtc.datetime() {
                trace!("New RTC time: {:?}", Debug2Format(&dt));
                self.latest_dt = dt;
                sender.send(dt);
            }
        }
    }

    pub fn get_latest_datetime(&self) -> NaiveDateTime {
        self.latest_dt
    }

    pub fn set_datetime(&mut self, _dt: NaiveDateTime) {
        todo!()
    }
}