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
use crate::application::messaging::ApplicationData;
use crate::hmi::messaging::{UiActionChannelPublisher, UiRequestMessage};
use crate::hmi::screens::UiInput;
use crate::hmi::screens::UiInputHandler;
use crate::hmi::screens::{add_newlines_to_string, UiDrawer};
use core::fmt::Write;
use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::geometry::Point;
use embedded_graphics::mono_font::ascii::FONT_6X10;
use embedded_graphics::mono_font::MonoTextStyleBuilder;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::text::{Baseline, Text};
use embedded_graphics::Drawable;
use heapless::String;

pub struct HeapStatusScreen {
    heap_free: usize,
    heap_used: usize,
}

impl HeapStatusScreen {
    pub fn new() -> Self {
        Self {
            heap_free: 0,
            heap_used: 0,
        }
    }

    fn process_app_data(&mut self, data: ApplicationData) {
        match data {
            ApplicationData::HeapStatus { used, free } => {
                self.heap_used = used;
                self.heap_free = free;
            }
            _ => {}
        }
    }
}

impl UiInputHandler for HeapStatusScreen {
    async fn ui_input_handler(
        &mut self,
        input: UiInput,
        ui_channel_publisher: &UiActionChannelPublisher<'_>,
    ) {
        match input {
            UiInput::ButtonPress => ui_channel_publisher
                .publish_immediate(UiRequestMessage::ChangeState(ApplicationState::Settings)),
            UiInput::ApplicationData(data) => {
                self.process_app_data(data);
            }
            _ => {}
        }
    }
}

impl UiDrawer for HeapStatusScreen {
    fn draw<D>(&self, display: &mut D) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = BinaryColor>,
    {
        let text_style = MonoTextStyleBuilder::new()
            .font(&FONT_6X10)
            .text_color(BinaryColor::On)
            .build();
        let font_width = text_style.font.character_size.width;
        let mut count_string = String::<40>::new();

        count_string.clear();
        write!(&mut count_string, "Heap Used: {} bytes", self.heap_used).unwrap();
        Text::with_baseline(
            count_string.as_str(),
            Point::new(0, 0),
            text_style,
            Baseline::Top,
        )
        .draw(display)?;

        count_string.clear();
        write!(&mut count_string, "Heap Free: {} bytes", self.heap_free).unwrap();
        Text::with_baseline(
            count_string.as_str(),
            Point::new(0, 16),
            text_style,
            Baseline::Top,
        )
        .draw(display)?;

        count_string.clear();

        let max_line_length = display.bounding_box().size.width as usize / font_width as usize;
        let string_to_print =
            add_newlines_to_string::<40>("Press button to return to menu", max_line_length);
        write!(&mut count_string, "{}", string_to_print).unwrap();
        Text::with_baseline(
            count_string.as_str(),
            Point::new(0, 32),
            text_style,
            Baseline::Top,
        )
        .draw(display)?;
        Ok(())
    }
}
