
pub mod hx711async;

pub trait AsyncStrainGaugeInterface {
    type Error;

    async fn initialize(&mut self) -> Result<(), Self::Error>;

    async fn get_next_reading(&mut self) -> Result<u32, Self::Error>;

    async fn power_down(&mut self) -> Result<(), Self::Error>;

    async fn power_up(&mut self) -> Result<(), Self::Error>;
}