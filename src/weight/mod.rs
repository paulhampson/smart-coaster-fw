pub(crate) mod weight;
pub(crate) mod interface;
pub(crate) mod event_channels;


pub trait WeighingSystem {

    type Error;

    async fn stabilize_measurements(&mut self) -> Result<(), Self::Error>;

    async fn tare(&mut self) -> Result<(), Self::Error>;

    async fn calibrate(&mut self, calibration_mass:f32) -> Result<(), Self::Error>;

    async fn get_instantaneous_weight_grams(&mut self) -> Result<f32, Self::Error>;

    async fn get_reading(&mut self) -> Result<f32, Self::Error>;
}