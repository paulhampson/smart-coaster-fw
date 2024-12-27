use defmt::Format;

#[derive(Debug, Format, Clone, Copy, PartialEq)]
pub enum ProductState {
    Startup,
    Home,
    Tare,
    Calibration(u32),
    CalibrationDone,
    Wait,
}