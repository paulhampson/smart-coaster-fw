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
use crate::hmi::messaging::HmiChannelPublisher;
use crate::hmi::messaging::HmiMessage;
use crate::hmi::rotary_encoder::RotaryEncoder;
use defmt::{trace, Debug2Format};
use embassy_futures::select::{select, Either};
use embassy_rp::gpio::Level;

const PRESSED_LEVEL: Level = Level::Low;

pub async fn hmi_input_handler(
    hmi_event_channel: HmiChannelPublisher<'_>,
    mut debounced_btn: Debouncer<'_>,
    rotary_encoder: &mut impl RotaryEncoder,
) {
    let mut next_btn_level = PRESSED_LEVEL; // assumes we start unpressed
    loop {
        trace!("Waiting for input");
        let hmi_io_event = select(
            rotary_encoder.state_change(),
            debounced_btn.wait_for_change_to(next_btn_level),
        )
        .await;

        match hmi_io_event {
            Either::First(encoder_moved) => {
                trace!("Encoder moved: {}", Debug2Format(&encoder_moved));
                hmi_event_channel.publish_immediate(HmiMessage::EncoderUpdate(encoder_moved))
            }
            Either::Second(_) => {
                trace!("Button pressed: {}", Debug2Format(&next_btn_level));
                hmi_event_channel.publish_immediate(HmiMessage::PushButtonPressed(
                    next_btn_level == PRESSED_LEVEL,
                ));
                match next_btn_level {
                    Level::High => next_btn_level = Level::Low,
                    Level::Low => next_btn_level = Level::High,
                }
            }
        }
    }
}
