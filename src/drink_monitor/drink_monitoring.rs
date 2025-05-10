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
use crate::application::messaging::{ApplicationChannelSubscriber, ApplicationMessage};
use crate::drink_monitor::log_data::DrinkMonitorLogData;
use crate::drink_monitor::messaging::{DrinkMonitorChannelPublisher, DrinkMonitoringUpdate};
use crate::hmi::screens::settings_menu::monitoring_options::MonitoringTargetPeriodOptions;
use crate::rtc::accessor::RtcAccessor;
use crate::storage::historical::accessor::HistoricalLogAccessor;
use crate::storage::historical::messaging::{HistoricalLogChannel, HistoricalLogMessage};
use crate::storage::historical::{log_config, LogEncodeDecode};
use crate::storage::settings::accessor::FlashSettingsAccessor;
use crate::storage::settings::messaging::SettingsMessage;
use crate::storage::settings::monitor::FlashSettingsMonitor;
use crate::storage::settings::{SettingValue, SettingsAccessor, SettingsAccessorId};
use crate::weight::WeighingSystem;
use chrono::{NaiveDateTime, NaiveTime, TimeDelta, Timelike};
use core::cmp::PartialEq;
use core::ops::Sub;
use defmt::{debug, error, info, trace, warn, Debug2Format};
use embassy_futures::select::{select4, Either4};
use embassy_sync::pubsub::PubSubChannel;
use embassy_time::{Duration, Ticker, Timer};
use heapless::HistoryBuffer;
use micromath::F32Ext;

static LOG_READ_CHANNEL: HistoricalLogChannel = PubSubChannel::new();

#[derive(Clone, PartialEq, Copy, Debug)]
pub enum MonitoringStateSubstates {
    WaitingForActivity,
    VesselRemoved,
    VesselPlaced,
    Error(&'static str),
}

pub struct DrinkMonitoring<WS> {
    weighing_system: WS,
    drink_monitor_publisher: DrinkMonitorChannelPublisher<'static>,
    hourly_consumption_target: f32,
    daily_consumption_target: u32,
    target_mode: MonitoringTargetPeriodOptions,
    rtc_accessor: RtcAccessor,
    monitoring_start_time: NaiveDateTime,
    total_consumption: f32,
    daily_consumption_target_time: NaiveTime,
    monitoring_log: HistoricalLogAccessor,
}

impl<WS> DrinkMonitoring<WS>
where
    WS: WeighingSystem,
{
    const STABILISED_WEIGHT_MAX_DELTA: f32 = 5.0;

    pub fn new(
        drink_monitor_publisher: DrinkMonitorChannelPublisher<'static>,
        weighing_system: WS,
    ) -> Self {
        let mut rtc_accessor =
            RtcAccessor::new().unwrap_or_else(|_| panic!("Failed to get RTC accessor"));
        Self {
            drink_monitor_publisher,
            weighing_system,
            hourly_consumption_target: 0.0,
            daily_consumption_target: 0,
            target_mode: MonitoringTargetPeriodOptions::Hourly,
            monitoring_start_time: rtc_accessor.get_date_time(),
            rtc_accessor,
            total_consumption: 0.0,
            daily_consumption_target_time: Default::default(),
            monitoring_log: HistoricalLogAccessor::new(log_config::Logs::ConsumptionLog),
        }
    }

    async fn manage_error(&mut self, message: &'static str) -> ! {
        self.drink_monitor_publisher
            .publish(DrinkMonitoringUpdate::UpdateMonitoringSubstate(
                MonitoringStateSubstates::Error(message),
            ))
            .await;
        error!("{}", message);
        loop {
            // TODO - error recovery?
            Timer::after(Duration::from_secs(1)).await;
        }
    }

    async fn send_monitoring_update(&mut self, d: DrinkMonitoringUpdate) {
        trace!("Sending {}", Debug2Format(&d));
        trace!(
            "Space in queue - {}",
            self.drink_monitor_publisher.free_capacity()
        );
        self.drink_monitor_publisher.publish(d).await;
    }

    async fn get_weight_reading_managed_error(&mut self) -> f32 {
        match self.weighing_system.get_reading().await {
            Ok(w) => w,
            Err(_) => {
                self.manage_error("Scale reading failed. Restart device")
                    .await
            }
        }
    }

    async fn wait_for_weight_activity(&mut self) -> f32 {
        const MINIMUM_DELTA_FOR_ACTIVITY: f32 = 10.0;
        let mut last_weight = self.get_weight_reading_managed_error().await;
        loop {
            let current_weight = self.get_weight_reading_managed_error().await;
            let weight_delta = current_weight - last_weight;
            last_weight = current_weight;
            if weight_delta.abs() > MINIMUM_DELTA_FOR_ACTIVITY {
                return weight_delta;
            }
        }
    }

    async fn get_stabilised_weight(&mut self) -> f32 {
        const BUFFER_SIZE: usize = 4;
        let mut readings = HistoryBuffer::<_, BUFFER_SIZE>::new();
        loop {
            readings.write(self.get_weight_reading_managed_error().await);

            if readings.len() == BUFFER_SIZE {
                let min_reading: f32 = *readings
                    .as_slice()
                    .iter()
                    .reduce(|a, b| if a < b { a } else { b })
                    .unwrap();
                let max_reading: f32 = *readings
                    .as_slice()
                    .iter()
                    .reduce(|a, b| if a > b { a } else { b })
                    .unwrap();
                let reading_delta = max_reading - min_reading;
                if reading_delta < Self::STABILISED_WEIGHT_MAX_DELTA {
                    return readings.as_slice().iter().sum::<f32>() / readings.len() as f32;
                }
            }
        }
    }

    async fn update_monitoring_substate(&mut self, state: MonitoringStateSubstates) {
        self.drink_monitor_publisher
            .publish(DrinkMonitoringUpdate::UpdateMonitoringSubstate(state))
            .await;
    }

    async fn update_day_average_consumption_rate(&mut self) -> f32 {
        // daily reset logic
        let current_date_time = self.rtc_accessor.get_date_time();
        let current_date = NaiveDateTime::from(current_date_time.date());
        if self.monitoring_start_time != current_date {
            self.monitoring_start_time = current_date;
            self.total_consumption = 0.0;
        }

        // calculate the rate and notify the system
        let elapsed_time_in_hours = f32::max(
            current_date_time.num_seconds_from_midnight() as f32 / 3600.0,
            1.0, // this avoids getting meaningless consumption rate numbers when we are less than 1 hour from the start point
        );
        let consumption_rate = self.total_consumption / elapsed_time_in_hours;
        self.send_monitoring_update(DrinkMonitoringUpdate::DayAverageHourlyConsumptionRate(
            consumption_rate,
        ))
        .await;
        debug!("Consumption rate = {} ml/hr", consumption_rate.round());
        consumption_rate
    }

    async fn update_hourly_consumption_rate(&mut self) {
        trace!("Updating hourly consumption rate");

        let current_date_time = self.rtc_accessor.get_date_time();
        let time_stamp_one_hour_ago = current_date_time.sub(TimeDelta::hours(1));

        debug!(
            "Retrieving logs for hourly consumption rate calculation since {}",
            Debug2Format(&time_stamp_one_hour_ago)
        );
        let mut last_hour_consumption = 0.0;
        let mut total_entries = 0;
        trace!("Requesting log entries for hourly consumption rate calculation");
        self.monitoring_log
            .get_log_data_after_timestamp(time_stamp_one_hour_ago, &LOG_READ_CHANNEL)
            .await;

        let mut log_subscriber = LOG_READ_CHANNEL.subscriber().unwrap();
        'process_log_data: loop {
            trace!("Waiting for log data");
            let retrieved_log_message = log_subscriber.next_message_pure().await;
            trace!("Got log message");
            match retrieved_log_message {
                HistoricalLogMessage::Error() => {
                    error!("Log retrieval error when updating consumption rate. Skipping update");
                    break 'process_log_data;
                }
                HistoricalLogMessage::EndOfRead() => {
                    self.send_monitoring_update(DrinkMonitoringUpdate::LastHourConsumptionRate(
                        last_hour_consumption,
                    ))
                    .await;
                    debug!(
                        "Calculation complete over {} log entries - last hour consumption rate = {} ml/hr",
                        total_entries,
                        last_hour_consumption.round()
                    );
                    break 'process_log_data;
                }
                HistoricalLogMessage::Record(entry) => {
                    total_entries += 1;
                    let log_entry = DrinkMonitorLogData::from_bytes(&entry.data)
                        .map_err(|_| {
                            error!("Failed to decode data from log");
                        })
                        .unwrap();
                    last_hour_consumption += log_entry.get_last_consumption();
                    trace!(
                        "{} - consumption was {}",
                        Debug2Format(&entry.timestamp),
                        log_entry.get_last_consumption()
                    );
                }
            }
        }
    }

    async fn update_targets(&mut self) {
        match self.target_mode {
            MonitoringTargetPeriodOptions::Daily => {
                let current_time = self.rtc_accessor.get_date_time().time();
                let time_left = self.daily_consumption_target_time - current_time;
                let mut hours_left = time_left.num_minutes() as f32 / 60.0;
                if hours_left <= 1.0 {
                    hours_left = 1.0;
                }

                self.hourly_consumption_target =
                    (self.daily_consumption_target as f32 - self.total_consumption) / hours_left;
                if self.hourly_consumption_target < 0.0 {
                    self.hourly_consumption_target = 0.0;
                }
                debug!(
                    "Hourly target (calculated from daily) is now {} (daily_target = {}, consumed = {}, hours left = {})",
                    self.hourly_consumption_target,
                    self.total_consumption,
                    self.daily_consumption_target,
                    hours_left
                );
                self.send_monitoring_update(DrinkMonitoringUpdate::TargetConsumption(
                    self.daily_consumption_target as f32,
                ))
                .await;
            }
            MonitoringTargetPeriodOptions::Hourly => {
                // no calculation required.
                debug!("Hourly target is {}", self.hourly_consumption_target);
            }
        }

        self.send_monitoring_update(DrinkMonitoringUpdate::TargetRate(
            self.hourly_consumption_target,
        ))
        .await;
    }

    async fn update(&mut self, new_consumption: f32) {
        self.update_targets().await;
        self.update_day_average_consumption_rate().await;
        self.update_hourly_consumption_rate().await;

        let snapshot = DrinkMonitorLogData::new(
            self.hourly_consumption_target,
            self.daily_consumption_target,
            self.target_mode,
            self.total_consumption,
            self.daily_consumption_target_time,
            new_consumption,
        );
        self.monitoring_log.log_data(snapshot).await;
    }

    /// Sets internal monitoring mode state and retrieves associated target values.
    async fn change_monitoring_mode(
        &mut self,
        mode: MonitoringTargetPeriodOptions,
        settings: &FlashSettingsAccessor,
    ) {
        self.target_mode = mode;
        self.drink_monitor_publisher
            .publish(DrinkMonitoringUpdate::TargetMode(mode))
            .await;

        match self.target_mode {
            MonitoringTargetPeriodOptions::Hourly => {
                let hourly_setting = settings
                    .get_setting(SettingsAccessorId::MonitoringTargetHourly)
                    .await;
                if let Some(SettingValue::UInt(hourly_target)) = hourly_setting {
                    self.hourly_consumption_target = hourly_target as f32;
                } else {
                    warn!(
                        "Unable to get expected data for hourly target: {}",
                        Debug2Format(&hourly_setting)
                    );
                }
            }
            MonitoringTargetPeriodOptions::Daily => {
                let daily_setting = settings
                    .get_setting(SettingsAccessorId::MonitoringTargetDaily)
                    .await;
                if let Some(SettingValue::UInt(daily_target)) = daily_setting {
                    self.daily_consumption_target = daily_target;
                } else {
                    warn!(
                        "Unable to get expected data for daily target: {}",
                        Debug2Format(&daily_setting)
                    );
                }

                let target_time_setting = settings
                    .get_setting(SettingsAccessorId::MonitoringDailyTargetTime)
                    .await;
                if let Some(SettingValue::Time(target_time)) = target_time_setting {
                    self.daily_consumption_target_time = target_time;
                } else {
                    warn!(
                        "Unable to get expected data for daily target time: {}",
                        Debug2Format(&target_time_setting)
                    );
                }
            }
        }
        self.update(0.0).await;
    }

    async fn initialise_total_consumption(&mut self) {
        let current_date = self.rtc_accessor.get_date_time().date();
        let day_start = NaiveDateTime::from(current_date);

        debug!("Retrieving logs for today's consumption");
        let mut total_consumption = 0.0;
        let mut total_entries = 0;
        trace!("Requesting log entries for hourly consumption rate calculation");
        self.monitoring_log
            .get_log_data_after_timestamp(day_start, &LOG_READ_CHANNEL)
            .await;

        let mut log_subscriber = LOG_READ_CHANNEL.subscriber().unwrap();
        'process_log_data: loop {
            trace!("Waiting for log data");
            let retrieved_log_message = log_subscriber.next_message_pure().await;
            trace!("Got log message");
            match retrieved_log_message {
                HistoricalLogMessage::Error() => {
                    self.total_consumption = total_consumption;
                    error!("Log retrieval error when calculating historical consumption. Assuming {} ml", total_consumption.round());
                    break 'process_log_data;
                }
                HistoricalLogMessage::EndOfRead() => {
                    self.total_consumption = total_consumption;
                    info!(
                        "Looking at {} log entries - consumption since day start = {} ml",
                        total_entries,
                        self.total_consumption.round()
                    );
                    break 'process_log_data;
                }
                HistoricalLogMessage::Record(entry) => {
                    total_entries += 1;
                    let log_entry = DrinkMonitorLogData::from_bytes(&entry.data)
                        .map_err(|_| {
                            error!("Failed to decode data from log");
                        })
                        .unwrap();
                    total_consumption += log_entry.get_last_consumption();
                    trace!(
                        "{} - consumption was {}",
                        Debug2Format(&entry.timestamp),
                        log_entry.get_last_consumption()
                    );
                }
            }
        }
    }

    /// Monitor the weight scale for large deltas. Compare the positives and negatives to estimate
    /// how much fluid has been added and consumed. Report the consumption rate.
    pub async fn run(
        &mut self,
        mut application_channel_subscriber: ApplicationChannelSubscriber<'_>,
        settings: FlashSettingsAccessor,
    ) {
        const MINIMUM_DELTA_FOR_STATE_CHANGE: f32 = 10.0;
        let mut last_stable_weight = self.get_stabilised_weight().await;
        let mut vessel_placed_weight = last_stable_weight;
        let mut consumption_update_ticker = Ticker::every(Duration::from_secs(60));
        let mut settings_monitor = FlashSettingsMonitor::new();

        // initialise from stored settings
        let mode_from_settings = settings
            .get_setting(SettingsAccessorId::MonitoringTargetType)
            .await;
        if let Some(SettingValue::SmallUInt(mode_id)) = mode_from_settings {
            let mode = mode_id.try_into().unwrap();
            info!("Monitoring mode initialised to: {}", Debug2Format(&mode));
            self.change_monitoring_mode(mode, &settings).await;
        } else {
            panic!(
                "Unexpected monitoring mode initialisation data: {:?}",
                &mode_from_settings
            );
        }

        self.initialise_total_consumption().await;
        self.send_monitoring_update(DrinkMonitoringUpdate::TotalConsumed(self.total_consumption))
            .await;
        self.update(0.0).await;

        loop {
            let weight_update_or_consumption_tick_or_app_data = select4(
                self.wait_for_weight_activity(),
                consumption_update_ticker.next(),
                application_channel_subscriber.next_message_pure(),
                settings_monitor.listen_for_changes_ignore_lag(),
            )
            .await;
            match weight_update_or_consumption_tick_or_app_data {
                Either4::First(_) => {
                    let new_stable_weight = self.get_stabilised_weight().await;
                    let stable_delta = new_stable_weight - last_stable_weight;

                    if stable_delta > MINIMUM_DELTA_FOR_STATE_CHANGE {
                        self.update_monitoring_substate(MonitoringStateSubstates::VesselPlaced)
                            .await;
                        let consumption = f32::max(0.0, vessel_placed_weight - new_stable_weight);
                        self.total_consumption += consumption;

                        self.update(consumption).await;

                        self.send_monitoring_update(DrinkMonitoringUpdate::Consumption(f32::max(
                            0.0,
                            consumption,
                        )))
                        .await;
                        self.send_monitoring_update(DrinkMonitoringUpdate::TotalConsumed(
                            self.total_consumption,
                        ))
                        .await;

                        vessel_placed_weight = new_stable_weight;
                        trace!("New placed weight {}", vessel_placed_weight);
                        debug!("Consumption = {} ml", consumption);
                        debug!("Total consumption = {} ml", self.total_consumption);
                    } else if stable_delta < -MINIMUM_DELTA_FOR_STATE_CHANGE {
                        self.update_monitoring_substate(MonitoringStateSubstates::VesselRemoved)
                            .await;
                        trace!("New removed weight {}", new_stable_weight);
                    }
                    last_stable_weight = new_stable_weight;
                }
                Either4::Second(_) => {
                    // check for tick over to next day and reset consumption
                    self.update(0.0).await;
                }
                Either4::Third(app_message) => {
                    // This ensures that anything else in the system (e.g., LEDs, display) gets
                    // updated data on the switch to monitoring mode
                    if app_message
                        == ApplicationMessage::ApplicationStateUpdate(ApplicationState::Monitoring)
                    {
                        self.update(0.0).await;
                    }
                    if app_message == ApplicationMessage::ClearHistoricalConsumptionLog {
                        self.monitoring_log.clear_log().await;
                        self.total_consumption = 0.0;
                        self.update(0.0).await;
                    }
                }
                Either4::Fourth(setting_message) => {
                    let SettingsMessage::Change(changed_setting) = setting_message;
                    trace!(
                        "Handling changed setting: {:?}",
                        Debug2Format(&changed_setting)
                    );
                    let mut do_update = false;
                    match changed_setting.setting_id {
                        SettingsAccessorId::MonitoringTargetHourly => {
                            if let SettingValue::UInt(new_hourly_target) = changed_setting.value {
                                self.hourly_consumption_target = new_hourly_target as f32;
                                debug!("Hourly target is now {}", new_hourly_target);
                                do_update = true;
                            } else {
                                warn!(
                                    "Expected setting value for MonitoringTargetHourly: {}",
                                    Debug2Format(&changed_setting.value)
                                );
                            }
                        }
                        SettingsAccessorId::MonitoringTargetDaily => {
                            if let SettingValue::UInt(new_daily_target) = changed_setting.value {
                                self.daily_consumption_target = new_daily_target;
                                do_update = true;
                            } else {
                                warn!(
                                    "Expected setting value for MonitoringTargetDaily: {}",
                                    Debug2Format(&changed_setting.value)
                                );
                            }
                        }
                        SettingsAccessorId::MonitoringDailyTargetTime => {
                            if let SettingValue::Time(target_time) = changed_setting.value {
                                self.daily_consumption_target_time = target_time;
                                debug!("Target time is now {}", Debug2Format(&target_time));
                                do_update = true;
                            } else {
                                warn!(
                                    "Unable to get expected data for daily target time: {}",
                                    Debug2Format(&changed_setting.value)
                                );
                            }
                        }
                        SettingsAccessorId::MonitoringTargetType => {
                            if let SettingValue::SmallUInt(mode_id) = changed_setting.value {
                                let new_mode = mode_id.try_into().unwrap();
                                debug!("Monitoring mode changed to: {}", Debug2Format(&new_mode));
                                self.change_monitoring_mode(new_mode, &settings).await;
                            } else {
                                warn!(
                                    "Unexpected MonitoringTargetType setting value: {}",
                                    Debug2Format(&changed_setting.value)
                                );
                            }
                        }
                        _ => {}
                    }
                    if do_update {
                        debug!("Updating after settings change");
                        self.update(0.0).await;
                    }
                }
            }
        }
    }
}
