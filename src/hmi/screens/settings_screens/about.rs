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
    DEFAULT_TEXT_STYLE,
};
use core::cmp::min;
use core::fmt::Write;
use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::geometry::AnchorY;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::{DrawTargetExt, Point};
use embedded_graphics::text::renderer::TextRenderer;
use embedded_layout::View;
use heapless::String;

pub struct AboutScreen {
    start_entry_index: usize,
    max_entries: usize,

    repository_url_scroll_pos: usize,
}

impl AboutScreen {
    pub fn new() -> Self {
        Self {
            start_entry_index: 0,
            max_entries: 4,

            repository_url_scroll_pos: 0,
        }
    }

    fn line_draw<D>(&self, display: &mut D, index: usize) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = BinaryColor>,
    {
        let mut message_string = String::<100>::new();
        match index {
            0 => {
                write!(
                    message_string,
                    "{} v{}",
                    built_info::PKG_NAME,
                    built_info::PKG_VERSION,
                )
                .expect("String too long");
                draw_message_screen_no_reformat(display, &message_string)
            }
            1 => {
                let dirty_indicator = if built_info::GIT_DIRTY.unwrap_or(true) {
                    "+"
                } else {
                    ""
                };
                write!(
                    message_string,
                    "{}{}",
                    built_info::GIT_COMMIT_HASH_SHORT.unwrap_or("unknown"),
                    dirty_indicator
                )
                .expect("String too long");
                draw_message_screen_no_reformat(display, &message_string)
            }
            2 => {
                write!(message_string, "License: {}", built_info::PKG_LICENSE,)
                    .expect("String too long");
                draw_message_screen_no_reformat(display, &message_string)
            }
            3 => {
                write!(message_string, "{}", built_info::PKG_REPOSITORY).expect("String too long");
                draw_message_screen_no_reformat(display, &message_string)
            }
            _ => draw_message_screen(display, "Unknown page index"),
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
            UiInput::EncoderClockwise => self.start_entry_index += 1,
            UiInput::EncoderCounterClockwise => self.start_entry_index -= 1,
            UiInput::ButtonPress => ui_action_publisher.publish_immediate(
                UiActionsMessage::StateChangeRequest(ApplicationState::Settings),
            ),
            _ => {}
        }
        if self.start_entry_index >= self.max_entries {
            self.start_entry_index = 0;
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
        let lines_available = display.bounding_box().size.height / DEFAULT_TEXT_STYLE.line_height();
        let end_line = min(
            self.start_entry_index + lines_available as usize,
            self.max_entries,
        );

        let display_size = display.bounding_box();
        let mut line_draw_area =
            display_size.resized_height(DEFAULT_TEXT_STYLE.line_height(), AnchorY::Top);

        for (line_count, line_index) in (self.start_entry_index..end_line).enumerate() {
            let mut line_display = display.cropped(&line_draw_area);
            self.line_draw(&mut line_display, line_index)?;
            line_draw_area =
                line_draw_area.translate(Point::new(0, DEFAULT_TEXT_STYLE.line_height() as i32));
        }
        Ok(())
    }
}
