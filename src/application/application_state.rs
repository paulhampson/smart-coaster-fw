use defmt::Format;

#[derive(Debug, Format, Clone, Copy, PartialEq)]
pub enum ApplicationState {
    Startup,
    TestScreen,
    ErrorScreenWithMessage(&'static str),
    Settings,
    Monitoring,
    HeapStatus,
    Calibration,
}

#[derive(Clone)]
pub enum MonitoringStateSubstates {
    WaitingForActivity,
    VesselRemoved,
    VesselPlaced,
}

#[derive(Clone)]
pub enum CalibrationStateSubstates {
    Tare,
    Wait,
    Calibration(u32),
    CalibrationDone,
}
