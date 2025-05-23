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

use crate::application::messaging::ApplicationData;
use crate::hmi::messaging::UiActionChannelPublisher;
use crate::hmi::screens::{UiDrawer, UiInput, UiInputHandler};
use core::fmt::Write;
use ds323x::NaiveDateTime;
use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::geometry::Point;
use embedded_graphics::mono_font::ascii::FONT_6X10;
use embedded_graphics::mono_font::MonoTextStyleBuilder;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::text::renderer::TextRenderer;
use embedded_graphics::text::{Baseline, Text};
use embedded_graphics::Drawable;
use heapless::String;
use micromath::F32Ext;

pub struct TestModeScreen {
    cw_count: u32,
    ccw_count: u32,
    btn_press_count: u32,
    btn_release_count: u32,
    weight: f32,
    datetime: NaiveDateTime,
}

impl TestModeScreen {
    pub fn new() -> Self {
        Self {
            cw_count: 0,
            ccw_count: 0,
            btn_press_count: 0,
            btn_release_count: 0,
            weight: 0.0,
            datetime: NaiveDateTime::default(),
        }
    }

    fn process_app_data(&mut self, data: ApplicationData) {
        match data {
            ApplicationData::Weight(new_weight) => self.weight = new_weight,
            _ => {}
        }
    }
}

impl UiInputHandler for TestModeScreen {
    async fn ui_input_handler(
        &mut self,
        input: UiInput,
        _ui_channel_publisher: &UiActionChannelPublisher<'_>,
    ) {
        match input {
            UiInput::EncoderClockwise => {
                self.cw_count += 1;
            }
            UiInput::EncoderCounterClockwise => {
                self.ccw_count += 1;
            }
            UiInput::ButtonPress => {
                self.btn_press_count += 1;
            }
            UiInput::ButtonRelease => {
                self.btn_release_count += 1;
            }
            UiInput::ApplicationData(data) => {
                self.process_app_data(data);
            }
            UiInput::DateTimeUpdate(dt) => {
                self.datetime = dt;
            }
        }
    }
}

impl UiDrawer for TestModeScreen {
    fn draw<D>(&self, display: &mut D) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = BinaryColor>,
    {
        let text_style = MonoTextStyleBuilder::new()
            .font(&FONT_6X10)
            .text_color(BinaryColor::On)
            .build();
        let mut count_string = String::<32>::new();

        count_string.clear();
        write!(&mut count_string, "CW Count = {}", self.cw_count).unwrap();
        Text::with_baseline(
            count_string.as_str(),
            Point::new(0, 0),
            text_style,
            Baseline::Top,
        )
        .draw(display)?;

        count_string.clear();
        write!(&mut count_string, "CCW Count = {}", self.ccw_count).unwrap();
        Text::with_baseline(
            count_string.as_str(),
            Point::new(0, text_style.line_height() as i32),
            text_style,
            Baseline::Top,
        )
        .draw(display)?;

        count_string.clear();
        write!(&mut count_string, "Press Count = {}", self.btn_press_count).unwrap();
        Text::with_baseline(
            count_string.as_str(),
            Point::new(0, 2 * text_style.line_height() as i32),
            text_style,
            Baseline::Top,
        )
        .draw(display)?;

        count_string.clear();
        write!(&mut count_string, "Weight = {:.0}g", self.weight.round()).unwrap();
        Text::with_baseline(
            count_string.as_str(),
            Point::new(0, 3 * text_style.line_height() as i32),
            text_style,
            Baseline::Top,
        )
        .draw(display)?;

        count_string.clear();
        write!(&mut count_string, "{}", self.datetime).unwrap();
        Text::with_baseline(
            count_string.as_str(),
            Point::new(0, 4 * text_style.line_height() as i32),
            text_style,
            Baseline::Top,
        )
        .draw(display)?;
        Ok(())
    }
}
