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

use crate::application::application_state::ApplicationState;
use crate::hmi::messaging::{UiActionChannelPublisher, UiActionsMessage};
use crate::hmi::screens::{
    draw_message_screen, draw_message_screen_no_reformat, UiDrawer, UiInput, UiInputHandler,
};
use core::fmt::Write;
use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::pixelcolor::BinaryColor;
use heapless::String;

pub struct AboutScreen {
    page_index: usize,
    max_pages: usize,
}

impl AboutScreen {
    pub fn new() -> Self {
        Self {
            page_index: 0,
            max_pages: 1,
        }
    }
}

impl UiInputHandler for AboutScreen {
    async fn ui_input_handler(
        &mut self,
        input: UiInput,
        ui_action_publisher: &UiActionChannelPublisher<'_>,
    ) {
        match input {
            UiInput::EncoderClockwise => self.page_index += 1,
            UiInput::EncoderCounterClockwise => self.page_index -= 1,
            UiInput::ButtonPress => ui_action_publisher.publish_immediate(
                UiActionsMessage::StateChangeRequest(ApplicationState::Settings),
            ),
            _ => {}
        }
        if self.page_index >= self.max_pages {
            self.page_index = 0;
        }
    }
}

mod built_info {
    // The file has been placed there by the build script.
    include!(concat!(env!("OUT_DIR"), "/built.rs"));
}

impl UiDrawer for AboutScreen {
    fn draw<D>(&self, display: &mut D) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = BinaryColor>,
    {
        match self.page_index {
            0 => {
                let dirty_indicator = if built_info::GIT_DIRTY.unwrap_or(true) {
                    "+"
                } else {
                    ""
                };
                let mut message_string = String::<100>::new();
                write!(
                    message_string,
                    "{} v{}\n{}{}\nLicense: {}\n",
                    built_info::PKG_NAME,
                    built_info::PKG_VERSION,
                    built_info::GIT_COMMIT_HASH_SHORT.unwrap_or("unknown"),
                    dirty_indicator,
                    built_info::PKG_LICENSE,
                )
                .expect("String too long");
                draw_message_screen_no_reformat(display, &message_string)
            }
            _ => draw_message_screen(display, "Unknown page index"),
        }
    }
}
