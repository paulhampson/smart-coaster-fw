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
