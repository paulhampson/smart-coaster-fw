
use embassy_futures::select::{select, Either};
use embassy_rp::gpio::{Input, Level};
use embassy_time::{Duration, Timer};

pub struct Debouncer<'a> {
    input: Input<'a>,
    debounce_time: Duration,
}

#[allow(dead_code)]
impl<'a> Debouncer<'a>
{

    pub fn new(input: Input<'a>, debounce: Duration) -> Self {
        Self { input, debounce_time: debounce }
    }

    pub async fn debounce(&mut self) -> Level {
        loop {
            let io_changed = select(self.input.wait_for_any_edge(), Timer::after(self.debounce_time)).await;

            match io_changed {
                Either::First(_) => {
                    // edge detected, signal not yet stable for time required
                    continue
                }
                Either::Second(_) => {
                    // timer completed, signal stable
                    break self.input.get_level();
                }
            }
        }
    }

    pub async fn wait_for_first_rising_edge(&mut self) {
        self.input.wait_for_rising_edge().await;
    }

    pub async fn wait_for_first_falling_edge(&mut self) {
        self.input.wait_for_falling_edge().await;
    }

    pub async fn wait_for_change(&mut self) -> Level {
        let starting_level = self.debounce().await;

        match starting_level {
            Level::High => {
                self.wait_for_change_to(Level::Low).await;
                Level::Low
            },
            Level::Low => {
                self.wait_for_change_to(Level::High).await;
                Level::High
            }
        }
    }

    pub async fn wait_for_change_to(&mut self, target_level: Level)
    {
        loop {
            match target_level {
                Level::High => self.input.wait_for_rising_edge().await,
                Level::Low => self.input.wait_for_falling_edge().await
            }

            if self.debounce().await == target_level {
                break;
            }
        }
    }

    pub async fn wait_until_stable(&mut self, target_level: Level) {
        match target_level {
            Level::High => {self.input.wait_for_high().await;},
            Level::Low => {self.input.wait_for_low().await;},
        }
        while self.debounce().await != target_level {}
    }
}