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
use crate::hmi::messaging::{UiActionChannelPublisher, UiRequestMessage};
use crate::hmi::screens::{
    draw_message_screen, draw_message_screen_no_reformat, UiDrawer, UiInput, UiInputHandler,
    DEFAULT_FONT_WIDTH, DEFAULT_TEXT_STYLE,
};
use core::cmp::min;
use core::fmt::Write;
use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::geometry::AnchorY;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::{DrawTargetExt, Point};
use embedded_graphics::text::renderer::TextRenderer;
use embedded_graphics::text::Alignment;
use embedded_layout::View;
use heapless::String;

const REPOSITORY_PREFIX: &str = "Repository: ";
const REPOSITORY_SUFFIX: &str = "   ";

#[derive(Debug)]
struct ScrollingString {
    pub scroll_pos: u32,
    pub string: String<80>,
}

impl ScrollingString {
    pub fn new(string: String<80>) -> Self {
        Self { scroll_pos: 0, string }
    }

    pub fn increment_scroll_pos(&mut self, increment_by: u32) {
        self.scroll_pos += increment_by;
        if self.scroll_pos >= self.string.len() as u32 {
            self.scroll_pos = 0;
        }
    }

    /// Calculate the start and end indices of the substring to display based on the screen width, font width, scroll position and string length.
    /// Will not give a substring that is longer than the string length.
    /// If no scrolling is required, it will return the indices for the full string.
    fn get_scroll_text_substring(&self, screen_width: u32, font_width: u32) -> &str {
        let (start_pos, end_pos) =
            if (screen_width / font_width) < self.string.len() as u32 {
                (
                    self.scroll_pos as usize,
                    min((self.scroll_pos + (screen_width / font_width)) as usize, self.string.len())
                )
            } else {
                // small enough to fit on screen
                (0, self.string.len())
            };
        &self.string[start_pos..end_pos]
    }
}

pub struct AboutScreen {
    start_entry_index: usize,
    max_entries: usize,

    package_information: ScrollingString,
    repository_url: ScrollingString,
    version_tag: ScrollingString,
}

impl AboutScreen {
    pub fn new() -> Self {
        let mut repository_url_string = String::<80>::new();
        write!(
            repository_url_string,
            "{}{}{}",
            REPOSITORY_PREFIX,
            built_info::PKG_REPOSITORY,
            REPOSITORY_SUFFIX
        )
            .unwrap();

        let mut package_information_string = String::<80>::new();
        write!(
            package_information_string,
            "{} v{}",
            built_info::PKG_NAME,
            built_info::PKG_VERSION,
        )
            .expect("Package string too long");

        let mut tag_string = String::<80>::new();
        if built_info::GIT_VERSION.is_some() {
            if built_info::GIT_VERSION.unwrap()
                != built_info::GIT_COMMIT_HASH_SHORT.unwrap()
            {
                write!(tag_string, "Tag: {}", built_info::GIT_VERSION.unwrap())
                    .expect("tag string too long");
            } else {
                write!(tag_string, "No tag").expect("tag string too long");
            }
        }

        let s = Self {
            start_entry_index: 0,
            max_entries: 5,

            package_information: ScrollingString::new(package_information_string),
            repository_url: ScrollingString::new(repository_url_string),
            version_tag: ScrollingString::new(tag_string),
        };
        s
    }

    /// Manage updates on each re-draw. This function currently assumes that the top level
    /// display width is the same as the width the line display area is going to get. We need
    /// to do this here because the draw function is (rightly) not allowed to modify data.
    pub fn update_pre_draw_actions<D>(&mut self, _display: &D)
    where
        D: DrawTarget<Color=BinaryColor>,
    {
        self.package_information.increment_scroll_pos(1);
        self.repository_url.increment_scroll_pos(1);
        self.version_tag.increment_scroll_pos(1);
    }


    fn line_draw<D>(&self, display: &mut D, index: usize) -> Result<(), D::Error>
    where
        D: DrawTarget<Color=BinaryColor>,
    {
        let mut message_string = String::<100>::new();
        match index {
            0 => {
                let sub_string_to_display = self.package_information.get_scroll_text_substring(display.bounding_box().size.width,
                                                                                               DEFAULT_FONT_WIDTH as u32, );
                write!(message_string, "{}", sub_string_to_display).expect("String too long");
                draw_message_screen_no_reformat(display, &message_string, Alignment::Left)
            }
            1 => {
                let dirty_indicator = if built_info::GIT_DIRTY.unwrap_or(true) {
                    "+"
                } else {
                    ""
                };
                write!(
                    message_string,
                    "Commit: {}{}",
                    built_info::GIT_COMMIT_HASH_SHORT.unwrap_or("unknown"),
                    dirty_indicator
                )
                    .expect("String too long");
                draw_message_screen_no_reformat(display, &message_string, Alignment::Left)
            }
            2 => {
                let sub_string_to_display = self.version_tag.get_scroll_text_substring(display.bounding_box().size.width,
                                                                                       DEFAULT_FONT_WIDTH as u32);
                write!(message_string, "{}", sub_string_to_display).expect("String too long");
                draw_message_screen_no_reformat(display, &message_string, Alignment::Left)
            }
            3 => {
                write!(message_string, "License: {}", built_info::PKG_LICENSE, )
                    .expect("String too long");
                draw_message_screen_no_reformat(display, &message_string, Alignment::Left)
            }
            4 => {
                let sub_string_to_display = self.repository_url.get_scroll_text_substring(display.bounding_box().size.width,
                                                                                          DEFAULT_FONT_WIDTH as u32);
                write!(message_string, "{}", sub_string_to_display).expect("String too long");
                draw_message_screen_no_reformat(display, &message_string, Alignment::Left)
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
            UiInput::ButtonPress => ui_action_publisher
                .publish_immediate(UiRequestMessage::ChangeState(ApplicationState::Settings)),
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
        D: DrawTarget<Color=BinaryColor>,
    {
        let lines_available = display.bounding_box().size.height / DEFAULT_TEXT_STYLE.line_height();
        let end_line = min(
            self.start_entry_index + lines_available as usize,
            self.max_entries,
        );

        let display_size = display.bounding_box();
        let mut line_draw_area =
            display_size.resized_height(DEFAULT_TEXT_STYLE.line_height(), AnchorY::Top);

        for line_index in self.start_entry_index..end_line {
            let mut line_display = display.cropped(&line_draw_area);
            self.line_draw(&mut line_display, line_index)?;
            line_draw_area =
                line_draw_area.translate(Point::new(0, DEFAULT_TEXT_STYLE.line_height() as i32));
        }
        Ok(())
    }
}
