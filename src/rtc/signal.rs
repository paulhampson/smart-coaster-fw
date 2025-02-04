use ds323x::NaiveDateTime;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;
use embassy_sync::watch::{Receiver, Watch};

const RTC_WATCH_RECEIVER_COUNT: usize = 5;
pub type RtcWatchReceiver = Receiver<'static, CriticalSectionRawMutex, NaiveDateTime, RTC_WATCH_RECEIVER_COUNT>;
pub static RTC_TIME_UPDATE: Watch<CriticalSectionRawMutex, NaiveDateTime, RTC_WATCH_RECEIVER_COUNT> = Watch::new();
pub static RTC_SET_TIME: Signal<CriticalSectionRawMutex, NaiveDateTime> = Signal::new();