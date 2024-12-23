use crate::hmi::debouncer::Debouncer;
use embassy_futures::select::{select, Either};
use embassy_rp::gpio::{Input, Level};
use embassy_time::Duration;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Direction {
    Clockwise,
    CounterClockwise,
    None,
}

impl From<u8> for Direction {
    fn from(s: u8) -> Self {
        match s {
            0b0001 | 0b0111 | 0b1000 | 0b1110 => Direction::Clockwise,
            0b0010 | 0b0100 | 0b1011 | 0b1101 => Direction::CounterClockwise,
            _ => Direction::None,
        }
    }
}

pub struct RotaryEncoder<'a> {
    debounced_dt: Debouncer<'a>,
    debounced_clk: Debouncer<'a>
}

impl<'a> RotaryEncoder<'a>
{
    pub fn new(pin_dt: Input<'a>, pin_clk: Input<'a>) -> Self {
        Self {
            debounced_dt:  Debouncer::new(pin_dt, Duration::from_micros(500)),
            debounced_clk:  Debouncer::new(pin_clk, Duration::from_micros(500)),
        }
    }

    async fn get_current_state(&mut self) -> u8 {
        let mut s = 0u8;
        let dt_level = self.debounced_dt.get_debounced_level().await;
        let clk_level = self.debounced_clk.get_debounced_level().await;
        if dt_level == Level::High {
            s |= 0b01;
        }
        if clk_level == Level::High {
            s |= 0b10;
        }
        s
    }

    pub async fn state_change(&mut self) -> Direction {
        let mut s = self.get_current_state().await;
        s |= s << 2;

        let io_changes = select(self.debounced_dt.debounce(), self.debounced_clk.debounce()).await;

        match io_changes {
            Either::First(dt_level) => {
                if dt_level == Level::High {
                    s |= 0b0100;
                } else {
                    s &= 0b1011;
                }
            }
            Either::Second(clk_level) => {
                if clk_level == Level::High {
                    s |= 0b1000;
                } else {
                    s &= 0b0111;
                }
            }
        }

        s.into()
    }
}
