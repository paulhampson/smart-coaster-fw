// Copyright (C) 2026 Paul Hampson
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

use crate::drink_monitor::drink_monitoring::MonitoringStateSubstates;
use crate::hmi::screens::monitoring::{MonitoringData, MonitoringScreenContent};
use core::cmp::max;
use core::fmt::Write;
use embedded_graphics::draw_target::{DrawTarget, DrawTargetExt};
use embedded_graphics::geometry::{AnchorY, Dimensions, OriginDimensions, Point, Size};
use embedded_graphics::mono_font::ascii::{FONT_6X10, FONT_8X13_BOLD};
use embedded_graphics::mono_font::MonoTextStyleBuilder;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{PrimitiveStyleBuilder, Rectangle, StrokeAlignment};
use embedded_graphics::text::renderer::TextRenderer;
use embedded_graphics::text::{Alignment, Baseline, Text, TextStyleBuilder};
use embedded_graphics::Drawable;
use heapless::String;

pub struct MonitoringScreen5 {}

impl MonitoringScreen5 {
    fn calculate_drink_to_stay_on_target(monitoring_data: &MonitoringData) -> i32 {
        if monitoring_data.last_hour {
            return monitoring_data.target_rate as i32;
        }
        max(
            0,
            (monitoring_data.target_rate - monitoring_data.last_hour_consumption_rate) as i32,
        )
    }
}

impl<D> MonitoringScreenContent<D> for MonitoringScreen5
where
    D: DrawTarget<Color=BinaryColor>,
{
    fn draw_content(
        &self,
        display: &mut D,
        _state: MonitoringStateSubstates,
        data: &MonitoringData,
    ) -> Result<(), D::Error> {
        let main_area_display = display;

        // show how much is needed to drink to stay on track
        let amount_to_stay_on_target = Self::calculate_drink_to_stay_on_target(data);

        let mut string_buffer = String::<20>::new();

        // use the top two thirds of the screen
        let mut upper_display_area =
            main_area_display.cropped(&main_area_display.bounding_box().resized_height(
                2 * main_area_display.bounding_box().size.height / 3,
                AnchorY::Top,
            ));

        // setup some standard font sizes and styles
        let value_char_style = MonoTextStyleBuilder::new()
            .font(&FONT_8X13_BOLD)
            .text_color(BinaryColor::On)
            .build();
        let label_char_style = MonoTextStyleBuilder::new()
            .font(&FONT_6X10)
            .text_color(BinaryColor::On)
            .build();

        let centre_text_style = TextStyleBuilder::new()
            .alignment(Alignment::Center)
            .baseline(Baseline::Top)
            .build();

        let mut pos = upper_display_area.bounding_box().center();
        pos.y -= (value_char_style.line_height() / 2) as i32
            + (label_char_style.line_height() / 2) as i32;
        Text::with_text_style("Drink", pos, label_char_style, centre_text_style)
            .draw(&mut upper_display_area)?;

        string_buffer.clear();
        write!(string_buffer, "{} ml", amount_to_stay_on_target).unwrap();
        pos = upper_display_area.bounding_box().center();

        Text::with_text_style(
            string_buffer.as_str(),
            pos,
            value_char_style,
            centre_text_style,
        )
            .draw(&mut upper_display_area)?;

        pos.y += (value_char_style.line_height() / 2) as i32
            + (label_char_style.line_height() / 2) as i32;
        Text::with_text_style(
            "to stay on track",
            pos,
            label_char_style,
            centre_text_style,
        )
            .draw(&mut upper_display_area)?;

        // use the lower 1/3 of the screen
        let mut lower_display_area =
            main_area_display.cropped(&main_area_display.bounding_box().resized_height(
                main_area_display.bounding_box().size.height / 3,
                AnchorY::Bottom,
            ));

        let progress_outline_height = 5;
        let progress_outline_space_from_edge = 5;
        let progress_outline_origin = Point::new(
            progress_outline_space_from_edge,
            lower_display_area.size().height as i32 / 2
                - progress_outline_height / 2,
        );
        let max_progress_width = lower_display_area.size().width - (2 * progress_outline_space_from_edge) as u32;

        let outline_style = PrimitiveStyleBuilder::new()
            .fill_color(BinaryColor::Off)
            .stroke_width(1)
            .stroke_color(BinaryColor::On)
            .stroke_alignment(StrokeAlignment::Outside)
            .build();
        Rectangle::new(
            progress_outline_origin,
            Size::new(
                max_progress_width,
                progress_outline_height as u32,
            ),
        )
            .into_styled(outline_style)
            .draw(&mut lower_display_area)?;

        let padding_to_outline = 1;
        let progress_fill_origin = Point::new(progress_outline_origin.x + padding_to_outline,
                                              progress_outline_origin.y + padding_to_outline);

        let bar_style = PrimitiveStyleBuilder::new()
            .fill_color(BinaryColor::On)
            .build();

        let max_fill_width = max_progress_width - (2 * padding_to_outline as u32);
        let fill_width: u32 = if data.day_target_consumption > 0.0
        {
            data.day_total_consumed as u32 * max_fill_width / data.day_target_consumption as u32
        } else {
            0
        }.min(max_fill_width);

        Rectangle::new(
            progress_fill_origin,
            Size::new(
                fill_width,
                (progress_outline_height - (2 * padding_to_outline)) as u32,
            ),
        )
            .into_styled(bar_style)
            .draw(&mut lower_display_area)?;

        Ok(())
    }
}
