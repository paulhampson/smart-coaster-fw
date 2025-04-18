use crate::application::messaging::ApplicationChannelSubscriber;
use crate::drink_monitor::messaging::{DrinkMonitorChannelPublisher, DrinkMonitoringUpdate};
use crate::hmi::screens::settings_menu::monitoring_options::MonitoringTargetPeriodOptions;
use crate::storage::settings::accessor::FlashSettingsAccessor;
use crate::storage::settings::messaging::SettingsMessage;
use crate::storage::settings::monitor::FlashSettingsMonitor;
use crate::storage::settings::{SettingValue, SettingsAccessor, SettingsAccessorId};
use crate::weight::WeighingSystem;
use defmt::{debug, error, trace, warn, Debug2Format};
use embassy_futures::select::{select4, Either4};
use embassy_time::{Duration, Instant, Ticker, Timer};
use heapless::HistoryBuffer;
use micromath::F32Ext;

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
        Self {
            drink_monitor_publisher,
            weighing_system,
            hourly_consumption_target: 0.0,
            daily_consumption_target: 0,
            target_mode: MonitoringTargetPeriodOptions::Hourly,
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
        self.drink_monitor_publisher.publish(d).await;
    }

    async fn get_weight_reading_managed_error(&mut self) -> f32 {
        match self.weighing_system.get_reading().await {
            Ok(w) => w,
            Err(_) => self.manage_error("Scale reading failed").await,
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

    async fn update_consumption_rate(
        &mut self,
        monitoring_start_time: Instant,
        total_consumption: f32,
    ) -> f32 {
        let elapsed_time_in_hours = f32::max(
            monitoring_start_time.elapsed().as_secs() as f32 / 3600.0,
            1.0,
        );
        let consumption_rate = total_consumption / elapsed_time_in_hours;
        self.send_monitoring_update(DrinkMonitoringUpdate::ConsumptionRate(consumption_rate))
            .await;
        debug!("Consumption rate = {} ml/hr", consumption_rate.round());
        consumption_rate
    }

    fn update_hourly_target(&mut self) {
        match self.target_mode {
            MonitoringTargetPeriodOptions::Daily => {
                self.hourly_consumption_target = self.daily_consumption_target as f32 / 24.0;
                debug!(
                    "Hourly target (calc'd from daily) is now {}",
                    self.hourly_consumption_target
                );
            }
            MonitoringTargetPeriodOptions::Hourly => {
                // no calculation required.
                debug!("Hourly target is {}", self.hourly_consumption_target);
            }
        }
    }

    /// Sets internal monitoring mode state and retrieves associated target values.
    async fn update_monitoring_mode(
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
            }
        }
        self.update_hourly_target();
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
        let mut total_consumption = 0.0;
        let monitoring_start_time = Instant::now();
        let mut consumption_update_ticker = Ticker::every(Duration::from_secs(60));
        let mut settings_monitor = FlashSettingsMonitor::new();

        // initialise from stored settings
        let mode_from_settings = settings
            .get_setting(SettingsAccessorId::MonitoringTargetType)
            .await;
        if let Some(SettingValue::SmallUInt(mode_id)) = mode_from_settings {
            let mode = mode_id.try_into().unwrap();
            debug!("Monitoring mode initialised to: {}", Debug2Format(&mode));
            self.update_monitoring_mode(mode, &settings).await;
        } else {
            panic!(
                "Unexpected monitoring mode initialisation data: {:?}",
                &mode_from_settings
            );
        }

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
                        let consumption = vessel_placed_weight - new_stable_weight;
                        if consumption > 0.0 {
                            total_consumption += consumption;
                        }
                        self.update_consumption_rate(monitoring_start_time, total_consumption)
                            .await;
                        vessel_placed_weight = new_stable_weight;
                        trace!("New placed weight {}", vessel_placed_weight);
                        debug!("Consumption = {} ml", consumption);
                        debug!("Total consumption = {} ml", total_consumption);
                        self.send_monitoring_update(DrinkMonitoringUpdate::Consumption(f32::max(
                            0.0,
                            consumption,
                        )))
                        .await;
                        self.send_monitoring_update(DrinkMonitoringUpdate::TotalConsumed(
                            f32::max(0.0, total_consumption),
                        ))
                        .await;
                    } else if stable_delta < -MINIMUM_DELTA_FOR_STATE_CHANGE {
                        self.update_monitoring_substate(MonitoringStateSubstates::VesselRemoved)
                            .await;
                        trace!("New removed weight {}", new_stable_weight);
                    }
                    last_stable_weight = new_stable_weight;
                }
                Either4::Second(_) => {
                    self.update_consumption_rate(monitoring_start_time, total_consumption)
                        .await;
                    self.update_hourly_target();
                }
                Either4::Third(_app_message) => {}
                Either4::Fourth(setting_message) => {
                    if let SettingsMessage::Change(changed_setting) = setting_message {
                        match changed_setting.setting_id {
                            SettingsAccessorId::MonitoringTargetHourly => {
                                if let SettingValue::UInt(new_hourly_target) = changed_setting.value
                                {
                                    self.hourly_consumption_target = new_hourly_target as f32;
                                    debug!("Hourly target is now {}", new_hourly_target);
                                } else {
                                    warn!(
                                        "Expected setting value for MonitoringTargetHourly: {}",
                                        Debug2Format(&changed_setting.value)
                                    );
                                }
                            }
                            SettingsAccessorId::MonitoringTargetDaily => {
                                if let SettingValue::UInt(new_daily_target) = changed_setting.value
                                {
                                    self.daily_consumption_target = new_daily_target;
                                } else {
                                    warn!(
                                        "Expected setting value for MonitoringTargetDaily: {}",
                                        Debug2Format(&changed_setting.value)
                                    );
                                }
                            }
                            SettingsAccessorId::MonitoringTargetType => {
                                if let SettingValue::SmallUInt(mode_id) = changed_setting.value {
                                    let new_mode = mode_id.try_into().unwrap();
                                    debug!(
                                        "Monitoring mode changed to: {}",
                                        Debug2Format(&new_mode)
                                    );
                                    self.update_monitoring_mode(new_mode, &settings).await;
                                } else {
                                    warn!(
                                        "Unexpected MonitoringTargetType setting value: {}",
                                        Debug2Format(&changed_setting.value)
                                    );
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }
}
