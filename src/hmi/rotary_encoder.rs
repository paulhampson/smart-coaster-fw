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

use crate::hmi::debouncer::Debouncer;
use embassy_futures::join::join;
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

/// Rotary encoder interface
pub trait RotaryEncoder {
    /// Wait for a state change
    async fn state_change(&mut self) -> Direction;
}

pub struct DebouncedRotaryEncoder<'a> {
    debounced_dt: Debouncer<'a>,
    debounced_clk: Debouncer<'a>,
}

impl<'a> DebouncedRotaryEncoder<'a> {
    pub fn new(pin_dt: Input<'a>, pin_clk: Input<'a>, debounce_duration: Duration) -> Self {
        Self {
            debounced_dt: Debouncer::new(pin_dt, debounce_duration),
            debounced_clk: Debouncer::new(pin_clk, debounce_duration),
        }
    }
}

impl RotaryEncoder for DebouncedRotaryEncoder<'_> {
    async fn state_change(&mut self) -> Direction {
        let mut clk_level: Level;
        let mut dt_level: Level;

        // ensure we aren't in the middle of a transition
        loop {
            (clk_level, dt_level) =
                join(self.debounced_clk.debounce(), self.debounced_dt.debounce()).await;
            if clk_level == dt_level {
                break;
            }
        }

        match clk_level {
            Level::Low => {
                let io_changes = select(
                    self.debounced_dt.wait_for_first_rising_edge(),
                    self.debounced_clk.wait_for_first_rising_edge(),
                )
                .await;
                match io_changes {
                    Either::First(_) => {
                        // DT changed first
                        Direction::CounterClockwise
                    }
                    Either::Second(_) => {
                        // CLK changed first
                        Direction::Clockwise
                    }
                }
            }
            Level::High => {
                let io_changes = select(
                    self.debounced_dt.wait_for_first_falling_edge(),
                    self.debounced_clk.wait_for_first_falling_edge(),
                )
                .await;
                match io_changes {
                    Either::First(_) => {
                        // DT changed first
                        Direction::CounterClockwise
                    }
                    Either::Second(_) => {
                        // CLK changed first
                        Direction::Clockwise
                    }
                }
            }
        }
    }
}

#[allow(dead_code)]
pub struct RawRotaryEncoder<'a> {
    dt: Input<'a>,
    clk: Input<'a>,
}

#[allow(dead_code)]
impl<'a> RawRotaryEncoder<'a> {
    fn new(pin_dt: Input<'a>, pin_clk: Input<'a>) -> Self {
        Self {
            dt: pin_dt,
            clk: pin_clk,
        }
    }

    fn get_current_state(&mut self) -> u8 {
        let mut s = 0u8;
        let dt_level = self.dt.get_level();
        let clk_level = self.clk.get_level();
        if dt_level == Level::High {
            s |= 0b01;
        }
        if clk_level == Level::High {
            s |= 0b10;
        }
        s
    }
}

#[allow(dead_code)]
impl<'a> RotaryEncoder for RawRotaryEncoder<'a> {
    async fn state_change(&mut self) -> Direction {
        let mut s = self.get_current_state();
        s |= s << 2;

        let io_changes = select(self.dt.wait_for_any_edge(), self.clk.wait_for_any_edge()).await;

        match io_changes {
            Either::First(_) => {
                let dt_level = self.dt.get_level();
                if dt_level == Level::High {
                    s |= 0b0100;
                    self.clk.wait_for_high().await;
                } else {
                    s &= 0b1011;
                    self.clk.wait_for_low().await;
                }
            }
            Either::Second(_) => {
                let clk_level = self.clk.get_level();
                if clk_level == Level::High {
                    s |= 0b1000;
                    self.dt.wait_for_high().await;
                } else {
                    s &= 0b0111;
                    self.dt.wait_for_low().await;
                }
            }
        }

        s.into()
    }
}
