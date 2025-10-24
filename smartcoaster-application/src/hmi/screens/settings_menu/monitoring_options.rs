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

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MonitoringTargetPeriodOptions {
    Daily,
    Hourly,
}

#[allow(dead_code)]
impl MonitoringTargetPeriodOptions {
    pub fn option_strings() -> &'static [&'static str] {
        &["Daily", "Hourly"]
    }

    pub fn units(&self) -> &'static str {
        match self {
            MonitoringTargetPeriodOptions::Daily => "ml/day",
            MonitoringTargetPeriodOptions::Hourly => "ml/hour",
        }
    }

    pub fn title(&self) -> &'static str {
        match self {
            MonitoringTargetPeriodOptions::Daily => "Daily Target",
            MonitoringTargetPeriodOptions::Hourly => "Hourly Target",
        }
    }
}

impl TryFrom<usize> for MonitoringTargetPeriodOptions {
    type Error = ();
    fn try_from(value: usize) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Daily),
            1 => Ok(Self::Hourly),
            _ => panic!("Invalid monitoring target period"),
        }
    }
}

impl TryFrom<u8> for MonitoringTargetPeriodOptions {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Daily),
            1 => Ok(Self::Hourly),
            _ => panic!("Invalid monitoring target period"),
        }
    }
}

impl From<MonitoringTargetPeriodOptions> for u8 {
    fn from(value: MonitoringTargetPeriodOptions) -> Self {
        match value {
            MonitoringTargetPeriodOptions::Daily => 0,
            MonitoringTargetPeriodOptions::Hourly => 1,
        }
    }
}
