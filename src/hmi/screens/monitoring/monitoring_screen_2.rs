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
use crate::hmi::screens::settings_menu::monitoring_options::MonitoringTargetPeriodOptions;
use core::fmt::Write;
use embedded_graphics::draw_target::{DrawTarget, DrawTargetExt};
use embedded_graphics::geometry::{AnchorX, Dimensions, OriginDimensions, Point, Size};
use embedded_graphics::image::Image;
use embedded_graphics::mono_font::ascii::{FONT_5X7, FONT_8X13_BOLD};
use embedded_graphics::mono_font::MonoTextStyleBuilder;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{Line, PrimitiveStyle, PrimitiveStyleBuilder, Rectangle};
use embedded_graphics::text::renderer::TextRenderer;
use embedded_graphics::text::{Alignment, Baseline, Text, TextStyleBuilder};
use embedded_graphics::Drawable;
use embedded_icon::NewIcon;
use heapless::String;

pub struct MonitoringScreen2 {}

impl MonitoringScreen2 {}

impl<D> MonitoringScreenContent<D> for MonitoringScreen2
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

        // vertical separator between the rate and total values
        let left_right_adjustment = -1;
        let vertical_pad = 3;
        Line::new(
            Point::new(
                (main_area_display.bounding_box().size.width * 2 / 3) as i32
                    + left_right_adjustment,
                vertical_pad,
            ),
            Point::new(
                (main_area_display.bounding_box().size.width * 2 / 3) as i32
                    + left_right_adjustment,
                (main_area_display.bounding_box().size.height) as i32 - vertical_pad,
            ),
        )
        .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
        .draw(main_area_display)?;

        // setup some standard font sizes and styles
        let value_char_style = MonoTextStyleBuilder::new()
            .font(&FONT_8X13_BOLD)
            .text_color(BinaryColor::On)
            .build();
        let label_char_style = MonoTextStyleBuilder::new()
            .font(&FONT_5X7)
            .text_color(BinaryColor::On)
            .build();
        let centre_text_style = TextStyleBuilder::new()
            .alignment(Alignment::Center)
            .baseline(Baseline::Middle)
            .build();
        let additional_top_padding = 0;
        let var_to_label_padding = 0;
        let label_to_var_padding = 5;
        let mut string_buffer = String::<20>::new();

        // Middle area - show the data for current rate and target rate
        let mut middle_display_area =
            main_area_display.cropped(&main_area_display.bounding_box().resized_width(
                main_area_display.bounding_box().size.width / 3,
                AnchorX::Center,
            ));

        write!(string_buffer, "{:.0}", data.consumption_rate).unwrap();
        let mut pos = middle_display_area.bounding_box().center();
        pos.y = (value_char_style.line_height() / 2) as i32 + additional_top_padding;
        Text::with_text_style(
            string_buffer.as_str(),
            pos,
            value_char_style,
            centre_text_style,
        )
        .draw(&mut middle_display_area)?;

        string_buffer.clear();
        write!(string_buffer, "average\nml/hour").unwrap();
        pos.y += (value_char_style.line_height() / 2) as i32
            + var_to_label_padding
            + (label_char_style.line_height() / 2) as i32;
        Text::with_text_style(
            string_buffer.as_str(),
            pos,
            label_char_style,
            centre_text_style,
        )
        .draw(&mut middle_display_area)?;

        string_buffer.clear();
        write!(string_buffer, "{:.0}", data.target_rate).unwrap();
        pos.y += ((2 * label_char_style.line_height()) / 2) as i32
            + label_to_var_padding
            + (value_char_style.line_height() / 2) as i32;
        Text::with_text_style(
            string_buffer.as_str(),
            pos,
            value_char_style,
            centre_text_style,
        )
        .draw(&mut middle_display_area)?;

        string_buffer.clear();
        write!(string_buffer, "target\nml/hour").unwrap();
        pos.y += (value_char_style.line_height() / 2) as i32
            + var_to_label_padding
            + ((1 * label_char_style.line_height()) / 2) as i32;
        Text::with_text_style(
            string_buffer.as_str(),
            pos,
            label_char_style,
            centre_text_style,
        )
        .draw(&mut middle_display_area)?;

        // Right area - show data for total consumption and target total
        let mut right_display_area =
            main_area_display.cropped(&main_area_display.bounding_box().resized_width(
                main_area_display.bounding_box().size.width / 3,
                AnchorX::Right,
            ));

        string_buffer.clear();
        write!(string_buffer, "{:.0}", data.total_consumed).unwrap();
        let mut pos = right_display_area.bounding_box().center();
        pos.y = (value_char_style.line_height() / 2) as i32 + additional_top_padding;
        Text::with_text_style(
            string_buffer.as_str(),
            pos,
            value_char_style,
            centre_text_style,
        )
        .draw(&mut right_display_area)?;

        string_buffer.clear();
        write!(string_buffer, "today's\ntotal ml").unwrap();
        pos.y += (value_char_style.line_height() / 2) as i32
            + var_to_label_padding
            + (label_char_style.line_height() / 2) as i32;
        Text::with_text_style(
            string_buffer.as_str(),
            pos,
            label_char_style,
            centre_text_style,
        )
        .draw(&mut right_display_area)?;

        string_buffer.clear();
        if data.target_mode == MonitoringTargetPeriodOptions::Daily {
            write!(string_buffer, "{:.0}", data.target_consumption).unwrap();
        } else {
            write!(string_buffer, "--").unwrap();
        }
        pos.y += ((2 * label_char_style.line_height()) / 2) as i32
            + label_to_var_padding
            + (value_char_style.line_height() / 2) as i32;
        Text::with_text_style(
            string_buffer.as_str(),
            pos,
            value_char_style,
            centre_text_style,
        )
        .draw(&mut right_display_area)?;

        string_buffer.clear();
        write!(string_buffer, "target\ntotal ml").unwrap();
        pos.y += (value_char_style.line_height() / 2) as i32
            + var_to_label_padding
            + ((1 * label_char_style.line_height()) / 2) as i32;
        Text::with_text_style(
            string_buffer.as_str(),
            pos,
            label_char_style,
            centre_text_style,
        )
        .draw(&mut right_display_area)?;

        Ok(())
    }
}
