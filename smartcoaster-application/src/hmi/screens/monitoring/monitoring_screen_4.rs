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

use crate::drink_monitor::drink_monitoring::MonitoringStateSubstates;
use crate::hmi::screens::monitoring::{MonitoringData, MonitoringScreenContent};
use core::cmp::max;
use core::fmt::Write;
use embedded_graphics::draw_target::{DrawTarget, DrawTargetExt};
use embedded_graphics::geometry::{AnchorX, Dimensions, OriginDimensions, Point, Size};
use embedded_graphics::image::Image;
use embedded_graphics::mono_font::ascii::{FONT_6X10, FONT_8X13_BOLD};
use embedded_graphics::mono_font::MonoTextStyleBuilder;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{PrimitiveStyleBuilder, Rectangle};
use embedded_graphics::text::renderer::TextRenderer;
use embedded_graphics::text::{Alignment, Baseline, Text, TextStyleBuilder};
use embedded_graphics::Drawable;
use embedded_icon::NewIcon;
use heapless::String;

pub struct MonitoringScreen4 {}

impl MonitoringScreen4 {
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

impl<D> MonitoringScreenContent<D> for MonitoringScreen4
where
    D: DrawTarget<Color = BinaryColor>,
{
    fn draw_content(
        &self,
        display: &mut D,
        state: MonitoringStateSubstates,
        data: &MonitoringData,
    ) -> Result<(), D::Error> {
        let main_area_display = display;

        // Draw cup (if present) and base
        let left_icon_area_width = main_area_display.bounding_box().size.width / 3;
        let mut left_icon_display = main_area_display.cropped(
            &main_area_display
                .bounding_box()
                .resized_width(left_icon_area_width, AnchorX::Left),
        );

        let icon = embedded_icon::mdi::size32px::Cup::new(BinaryColor::On);
        if state == MonitoringStateSubstates::VesselPlaced {
            let mut icon_location = left_icon_display.bounding_box().center();
            icon_location.x -= (icon.size().width / 2) as i32;
            icon_location.y -= (icon.size().height / 2) as i32;
            Image::new(&icon, icon_location).draw(&mut left_icon_display)?;
        }

        let icon_base_height = 3;
        let icon_base_space_from_edge = 5;
        let padding_from_bottom_of_icon = 0;
        let icon_base_origin = Point::new(
            icon_base_space_from_edge,
            left_icon_display.size().height as i32 / 2
                + icon.size().height as i32 / 2
                + padding_from_bottom_of_icon,
        );
        let base_style = PrimitiveStyleBuilder::new()
            .fill_color(BinaryColor::On)
            .build();
        Rectangle::new(
            icon_base_origin,
            Size::new(
                left_icon_area_width - (icon_base_space_from_edge * 2) as u32,
                icon_base_height as u32,
            ),
        )
        .into_styled(base_style)
        .draw(&mut left_icon_display)?;

        // show how much is needed to drink to stay on track
        let amount_to_stay_on_target = Self::calculate_drink_to_stay_on_target(data);

        let mut string_buffer = String::<20>::new();

        // use right 2/3 of the screen
        let mut right_display_area =
            main_area_display.cropped(&main_area_display.bounding_box().resized_width(
                2 * main_area_display.bounding_box().size.width / 3,
                AnchorX::Right,
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
            .baseline(Baseline::Bottom)
            .build();

        let mut pos = right_display_area.bounding_box().center();
        pos.y -= (value_char_style.line_height() / 2) as i32
            + (label_char_style.line_height() / 2) as i32
            + 3;
        Text::with_text_style("Drink", pos, label_char_style, centre_text_style)
            .draw(&mut right_display_area)?;

        string_buffer.clear();
        write!(string_buffer, "{} ml", amount_to_stay_on_target).unwrap();
        pos = right_display_area.bounding_box().center();

        Text::with_text_style(
            string_buffer.as_str(),
            pos,
            value_char_style,
            centre_text_style,
        )
        .draw(&mut right_display_area)?;

        pos.y += (value_char_style.line_height() / 2) as i32
            + (label_char_style.line_height() / 2) as i32;
        Text::with_text_style(
            "to stay on\ntrack",
            pos,
            label_char_style,
            centre_text_style,
        )
        .draw(&mut right_display_area)?;

        Ok(())
    }
}
