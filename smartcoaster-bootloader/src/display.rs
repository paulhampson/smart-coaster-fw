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

use defmt::{error, info, warn};
use sh1106::mode::GraphicsMode;

use smartcoaster_shared_utils::screen_text::draw_message_screen;
use heapless::String;
use crate::status_messaging::{BootloaderStatusChannelSubscriber, BootloaderStatusMessage};
use core::fmt::Write;

const DISPLAY_BRIGHTNESS: u8 = 255;


pub struct DisplayManager<DI>
where
    DI: sh1106::interface::DisplayInterface,
{
    display: GraphicsMode<DI>,
    status_subscriber: BootloaderStatusChannelSubscriber<'static>,
}

impl<DI> DisplayManager<DI>
where
    DI: sh1106::interface::DisplayInterface,
{
    pub fn new(mut display: GraphicsMode<DI>,
               status_subscriber: BootloaderStatusChannelSubscriber<'static>,
    ) -> Self {
        let _ = display.init().map_err(|_| error!("Failed to init display"));
        display.clear();
        let _ = display
            .flush()
            .map_err(|_| error!("Failed to flush display"));

        display
            .set_contrast(DISPLAY_BRIGHTNESS)
            .unwrap_or_else(|_| warn!("Failed to set display brightness"));

        Self {
            display,
            status_subscriber,
        }
    }
    pub async fn run(&mut self) {
        info!("Display manager running");

        loop {
            let new_state = self.status_subscriber.next_message_pure().await;

            self.display.clear();
            match new_state {
                BootloaderStatusMessage::Starting => {
                    draw_message_screen(&mut self.display, "Bootloader starting").unwrap();
                }
                BootloaderStatusMessage::HoldButton => {
                    info!("Button held display");
                    draw_message_screen(&mut self.display, "Continue to hold button to enter update mode.").unwrap();
                }
                BootloaderStatusMessage::WaitingForFirmware => {
                    draw_message_screen(&mut self.display, "Waiting for new firmware transfer.\nFirmware loader can be started.").unwrap();
                }
                BootloaderStatusMessage::TransferringFirmware(progress) => {
                    let mut progress_string = String::<128>::new();
                    write!(progress_string, "Transferring firmware.\nCompleted {} of {} chunks", progress.transferred_chunks, progress.total_chunks).unwrap();
                    draw_message_screen(&mut self.display, &progress_string).unwrap();
                }
                BootloaderStatusMessage::FirmwareInstalling => {
                    draw_message_screen(&mut self.display, "Installing new firmware.\nPlease wait.").unwrap();
                }
                BootloaderStatusMessage::StartingApplication => {
                    draw_message_screen(&mut self.display, "Starting up...").unwrap(); // first line same as application
                }
            }
            let _ = self.display.flush().map_err(|_| error!("Failed to flush display"));
        }
    }
}