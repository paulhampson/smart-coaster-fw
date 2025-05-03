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
use core::fmt::Write;
use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::geometry::Point;
use embedded_graphics::mono_font::ascii::FONT_6X10;
use embedded_graphics::mono_font::MonoTextStyleBuilder;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::text::renderer::TextRenderer;
use embedded_graphics::text::{Alignment, Baseline, Text, TextStyleBuilder};
use embedded_graphics::Drawable;
use heapless::String;
use micromath::F32Ext;

pub struct MonitoringScreenDebug {}

impl MonitoringScreenDebug {}

impl<D> MonitoringScreenContent<D> for MonitoringScreenDebug
where
    D: DrawTarget<Color = BinaryColor>,
{
    fn draw_content(
        &self,
        display: &mut D,
        state: MonitoringStateSubstates,
        data: &MonitoringData,
    ) -> Result<(), D::Error> {
        let mut string_buffer = String::<100>::new();
        let text_style = MonoTextStyleBuilder::new()
            .font(&FONT_6X10)
            .text_color(BinaryColor::On)
            .build();
        let centred_text_style = TextStyleBuilder::new()
            .alignment(Alignment::Center)
            .baseline(Baseline::Middle)
            .build();

        let central_x_pos = display.bounding_box().size.width as i32 / 2;
        let central_y_pos = display.bounding_box().size.height as i32 / 2;
        let target_y_pos = central_y_pos - (2f32 * text_style.line_height() as f32).round() as i32;
        let centre_point = Point::new(central_x_pos, target_y_pos);

        match state {
            MonitoringStateSubstates::VesselPlaced => {
                writeln!(string_buffer, "Vessel placed").unwrap();
            }
            MonitoringStateSubstates::VesselRemoved => {
                writeln!(string_buffer, "Vessel removed").unwrap();
            }
            _ => {}
        };
        writeln!(
            string_buffer,
            "Rate: {:.0} ml/hr",
            data.day_consumption_rate
        )
        .unwrap();
        writeln!(string_buffer, "Last drink: {:.0} ml", data.last_consumption).unwrap();
        write!(string_buffer, "Total: {:.0} ml", data.day_total_consumed).unwrap();
        Text::with_text_style(
            string_buffer.as_str(),
            centre_point,
            text_style,
            centred_text_style,
        )
        .draw(display)?;
        Ok(())
    }
}
