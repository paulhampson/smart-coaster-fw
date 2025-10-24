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

use chrono::{Datelike, NaiveDateTime, Timelike};
use core::fmt::Write;
use embedded_graphics::mono_font::ascii::{
    FONT_10X20, FONT_4X6, FONT_5X7, FONT_6X10, FONT_8X13, FONT_9X15,
};
use embedded_graphics::mono_font::{MonoFont, MonoTextStyleBuilder};
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::{DrawTarget, Point, Size};
use embedded_graphics::primitives::Rectangle;
use embedded_graphics::text::{Alignment, Baseline, Text, TextStyleBuilder};
use embedded_graphics::Drawable;
use embedded_layout::View;
use heapless::String;

pub struct TopStatusBar {
    datetime: NaiveDateTime,
    bounds: Rectangle,
}

impl TopStatusBar {
    pub fn new(datetime: NaiveDateTime, position: Point, size: Size) -> Self {
        Self {
            bounds: Rectangle::new(position, size),
            datetime,
        }
    }

    fn decide_font(&self) -> &MonoFont {
        match self.bounds.size.height {
            0..=6 => &FONT_4X6,
            7..=9 => &FONT_5X7,
            10..=12 => &FONT_6X10,
            13..=14 => &FONT_8X13,
            15..=19 => &FONT_9X15,
            _ => &FONT_10X20,
        }
    }
}

impl View for TopStatusBar {
    #[inline]
    fn translate_impl(&mut self, by: Point) {
        // make sure you don't accidentally call `translate`!
        self.bounds.translate_mut(by);
    }

    #[inline]
    fn bounds(&self) -> Rectangle {
        self.bounds
    }
}

impl Drawable for TopStatusBar {
    type Color = BinaryColor;
    type Output = ();

    fn draw<D: DrawTarget<Color = BinaryColor>>(&self, display: &mut D) -> Result<(), D::Error> {
        let text_char_style = MonoTextStyleBuilder::new()
            .font(self.decide_font())
            .text_color(BinaryColor::On)
            .build();

        let left_text_style = TextStyleBuilder::new()
            .alignment(Alignment::Left)
            .baseline(Baseline::Top)
            .build();
        let right_text_style = TextStyleBuilder::new()
            .alignment(Alignment::Right)
            .baseline(Baseline::Top)
            .build();

        let mut string_buffer = String::<20>::new();
        write!(
            string_buffer,
            "{}-{:02}-{:02}",
            self.datetime.year(),
            self.datetime.month(),
            self.datetime.day()
        )
        .unwrap();
        Text::with_text_style(
            string_buffer.as_str(),
            Point::zero(),
            text_char_style,
            left_text_style,
        )
        .draw(display)?;

        string_buffer.clear();
        write!(
            string_buffer,
            "{:02}:{:02}",
            self.datetime.hour(),
            self.datetime.minute()
        )
        .unwrap();
        Text::with_text_style(
            string_buffer.as_str(),
            Point::new(self.bounds.size.width as i32, 0),
            text_char_style,
            right_text_style,
        )
        .draw(display)?;

        Ok(())
    }
}
