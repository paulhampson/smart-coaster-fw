use crate::application::storage;

pub enum MonitoringTargetPeriodOptions {
    Daily,
    Hourly,
}

#[allow(dead_code)]
impl MonitoringTargetPeriodOptions {
    pub fn option_strings() -> &'static [&'static str] {
        &["Daily", "Hourly"]
    }

    pub fn monitoring_mode_to_storage_option_mapping(
        option_idx: usize,
    ) -> storage::settings::option_types::MonitoringTargetPeriodOptions {
        match option_idx {
            0 => storage::settings::option_types::MonitoringTargetPeriodOptions::Daily,
            1 => storage::settings::option_types::MonitoringTargetPeriodOptions::Hourly,
            _ => panic!("Invalid monitoring target period"),
        }
    }

    pub fn to_option_index(&self) -> usize {
        match self {
            MonitoringTargetPeriodOptions::Daily => 0,
            MonitoringTargetPeriodOptions::Hourly => 1,
        }
    }

    pub fn to_units(&self) -> &'static str {
        match self {
            MonitoringTargetPeriodOptions::Daily => "ml/day",
            MonitoringTargetPeriodOptions::Hourly => "ml/hour",
        }
    }
}
