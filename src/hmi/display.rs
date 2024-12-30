use core::fmt::Write;
use defmt::{debug, error, trace, warn};
use embassy_sync::pubsub::WaitResult;
use embassy_time::Instant;
use embedded_graphics::Drawable;
use embedded_graphics::geometry::Point;
use embedded_graphics::mono_font::ascii::FONT_6X10;
use embedded_graphics::mono_font::{MonoTextStyle, MonoTextStyleBuilder};
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::text::{Baseline, Text, Alignment, TextStyleBuilder};
use embedded_graphics::text::renderer::TextRenderer;
use heapless::String;
use sh1106::mode::GraphicsMode;
use micromath::F32Ext;
use crate::application::application_state::ApplicationState;
use crate::application::messaging::{ApplicationChannelSubscriber, ApplicationData, ApplicationMessage};
use crate::hmi::messaging::HmiMessage;
use crate::hmi::rotary_encoder::Direction;

const FRAME_TIMING_MS:u64 = 1000 / 30;

struct DisplayManager<'a, DI: sh1106::interface::DisplayInterface> {
    display: &'a mut GraphicsMode<DI>,
    text_style: MonoTextStyle<'a, BinaryColor>,

    display_state: ApplicationState,

    cw_count: u32,
    ccw_count: u32,
    btn_press_count: u32,
    last_weight: f32,
    last_update: Instant,
}

impl<'a, DI: sh1106::interface::DisplayInterface> DisplayManager<'a, DI>
{
    const DEFAULT_FONTH_WIDTH:usize = 6;

    pub fn new(display: &'a mut GraphicsMode<DI>) -> Self {
        let _ = display.init().map_err(|_| error!("Failed to init display"));
        let _ = display.flush().map_err(|_| error!("Failed to flush display"));

        Self {
            display,
            text_style: MonoTextStyleBuilder::new()
                .font(&FONT_6X10)
                .text_color(BinaryColor::On)
                .build(),

            display_state: ApplicationState::Startup,

            cw_count: 0,
            ccw_count: 0,
            btn_press_count: 0,
            last_weight: 0.0,
            last_update: Instant::MIN,
        }
    }

    pub fn set_display_state(&mut self, display_state: ApplicationState) {
        debug!("Display state: {:?}", display_state);
        self.display_state = display_state;
        self.update_now();

    }

    pub fn input_up(&mut self) {
        self.cw_count += 1;
    }

    pub fn input_down(&mut self) {
        self.ccw_count += 1;
    }

    pub fn input_press(&mut self) {
        self.btn_press_count += 1;
    }

    pub fn input_weight(&mut self, weight: f32) {
        self.last_weight = weight;
    }

    pub fn update(&mut self) {
        if self.last_update.elapsed().as_millis() < FRAME_TIMING_MS {
            return;
        }
        self.update_now();
    }

    fn update_now(&mut self) {
        self.display.clear();
        match self.display_state {
            ApplicationState::Startup => self.draw_message_screen("Starting up..."),
            ApplicationState::TestScreen => self.draw_home_screen(),
            ApplicationState::Tare => self.draw_message_screen("Remove items from device and press button"),
            ApplicationState::Calibration(calibration_mass_grams) => {
                let mut message_string = String::<40>::new();
                write!(message_string, "Put {}g on device then press button.", calibration_mass_grams)
                    .expect("String too long");
                self.draw_message_screen(&message_string);}
            ApplicationState::CalibrationDone => {self.draw_message_screen("Calibration complete")}
            ApplicationState::Wait => {self.draw_wait_screen()}
            ApplicationState::ErrorScreenWithMessage(s) => {self.draw_message_screen(s)}
        }

        let _ = self.display.flush().map_err(|_| error!("Display flush failed"));
        self.last_update = Instant::now();
    }

    pub fn add_newlines_to_string<const S: usize>(input: &str, max_line_length: usize) -> String<S> {
        let mut result = String::<S>::new();
        let mut current_length = 0;

        for word in input.split_whitespace() {
            // If the word exceeds max_line_length, split it with a hyphen
            if word.len() > max_line_length {
                let mut start = 0;

                while start < word.len() {
                    // Split the word into chunks of max_line_length
                    let end = core::cmp::min(start + max_line_length, word.len());
                    let part = &word[start..end];

                    // If not the first chunk, insert a newline
                    if current_length > 0 {
                        result.push('\n').unwrap();
                        current_length = 0;
                    }

                    // Add the part to the result
                    if end < word.len() {
                        // Add part of the word with a hyphen
                        result.push_str(part).unwrap();
                        result.push('-').unwrap();
                        current_length = part.len() + 1;
                    } else {
                        // Last chunk, no hyphen
                        result.push_str(part).unwrap();
                        current_length += part.len();
                    }

                    start = end; // Move the start position for the next chunk
                }
                continue;
            }

            // If adding the word exceeds the max line length, insert a newline
            if current_length + word.len() > max_line_length {
                result.push('\n').unwrap();
                current_length = 0; // Reset line length
            }

            // Add the word to the result
            result.push_str(word).unwrap();
            result.push(' ').unwrap(); // Add a space after the word
            current_length += word.len() + 1; // Include space in the length
        }

        result
    }

    fn draw_message_screen(&mut self, message: &str) {
        let max_line_length = self.display.get_dimensions().0 as usize / Self::DEFAULT_FONTH_WIDTH;
        let formatted_message = Self::add_newlines_to_string::<100>(message, max_line_length);
        let centred_text_style = TextStyleBuilder::new()
            .alignment(Alignment::Center)
            .baseline(Baseline::Middle)
            .build();

        let line_offset_pixels = (formatted_message.lines().count() - 1) as i32 * self.text_style.line_height() as i32;
        let x_pos = self.display.get_dimensions().0 as i32 / 2;
        let y_pos = self.display.get_dimensions().1 as i32 / 2 - line_offset_pixels;
        Text::with_text_style(&*formatted_message, Point::new(x_pos, y_pos), self.text_style, centred_text_style)
            .draw(self.display)
            .unwrap();
        trace!("Draw message screen done");
    }

    fn draw_home_screen(&mut self) {
        let mut count_string = String::<32>::new();
        self.display.clear();

        count_string.clear();
        write!(&mut count_string, "CW Count = {}", self.cw_count).unwrap();
        Text::with_baseline(count_string.as_str(), Point::new(0, 0), self.text_style, Baseline::Top)
            .draw(self.display)
            .unwrap();

        count_string.clear();
        write!(&mut count_string, "CCW Count = {}", self.ccw_count).unwrap();
        Text::with_baseline(count_string.as_str(), Point::new(0, 16), self.text_style, Baseline::Top)
            .draw(self.display)
            .unwrap();

        count_string.clear();
        write!(&mut count_string, "Press Count = {}", self.btn_press_count).unwrap();
        Text::with_baseline(count_string.as_str(), Point::new(0, 32), self.text_style, Baseline::Top)
            .draw(self.display)
            .unwrap();

        count_string.clear();
        write!(&mut count_string, "Weight = {:.0}g", self.last_weight.round()).unwrap();
        Text::with_baseline(count_string.as_str(), Point::new(0, 48), self.text_style, Baseline::Top)
            .draw(self.display)
            .unwrap();
    }

    fn draw_wait_screen(&mut self) {
        self.draw_message_screen("Please wait...");
    }
}

pub async fn display_update_handler<'a, DI: sh1106::interface::DisplayInterface>(mut app_channel_subscriber: ApplicationChannelSubscriber<'a>, display: &mut GraphicsMode<DI>) {
    let mut display_manager = DisplayManager::new(display);

    display_manager.update();

    loop {
        let wait_result = app_channel_subscriber.next_message().await;
        match wait_result {
            WaitResult::Lagged(count) => { warn!{"Display lost {} messages from HMI channel", count} }
            WaitResult::Message(message) => {
                match message {
                    ApplicationMessage::HmiInput(hmi_message) => match hmi_message {
                        HmiMessage::EncoderUpdate(direction) => {
                            trace!("Encoder update");
                            if direction == Direction::Clockwise {
                                display_manager.input_up();
                            }
                            if direction == Direction::CounterClockwise {
                                display_manager.input_down();
                            }
                        }

                        HmiMessage::PushButtonPressed(is_pressed) => {
                            trace!("Button pressed {:?} ", is_pressed);
                            if is_pressed {
                                display_manager.input_press();
                            }
                        }
                    }
                    ApplicationMessage::ApplicationStateUpdate(new_state) => {
                        trace!("Set display state");
                        display_manager.set_display_state(new_state);
                    }
                    ApplicationMessage::ApplicationDataUpdate(data_update) => {
                        trace!("App data update");
                        match data_update {
                            ApplicationData::Weight(w) => { display_manager.input_weight(w); }
                        }
                    }
                    ApplicationMessage::WeighSystemRequest(..) => {}
                }

                display_manager.update();
            }
        }
    }
}