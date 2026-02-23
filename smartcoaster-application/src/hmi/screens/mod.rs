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

use crate::application::messaging::ApplicationData;
use crate::hmi::messaging::UiActionChannelPublisher;
use ds323x::NaiveDateTime;
use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::pixelcolor::BinaryColor;

pub mod monitoring;
pub mod settings_menu;
pub mod settings_screens;

#[derive(Debug)]
pub enum UiInput {
    EncoderClockwise,
    EncoderCounterClockwise,
    ButtonPress,
    ButtonRelease,
    ApplicationData(ApplicationData),
    DateTimeUpdate(NaiveDateTime),
}

pub trait UiInputHandler {
    async fn ui_input_handler(
        &mut self,
        input: UiInput,
        ui_action_publisher: &UiActionChannelPublisher<'static>,
    );
}

pub trait UiDrawer {
    fn draw<D>(&self, display: &mut D) -> Result<(), D::Error>
    where
        D: DrawTarget<Color=BinaryColor>;
}
