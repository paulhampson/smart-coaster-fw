use crate::application::messaging::ApplicationData;
use crate::hmi::messaging::UiActionChannelPublisher;
use ds323x::NaiveDateTime;
use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::geometry::Point;
use embedded_graphics::mono_font::ascii::FONT_6X10;
use embedded_graphics::mono_font::{MonoTextStyle, MonoTextStyleBuilder};
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::text::renderer::TextRenderer;
use embedded_graphics::text::{Alignment, Baseline, Text, TextStyleBuilder};
use embedded_graphics::Drawable;
use heapless::String;

pub mod monitoring;
pub mod settings_menu;
pub mod settings_screens;

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
        ui_action_publisher: &UiActionChannelPublisher,
    );
}

pub trait UiDrawer {
    fn draw<D>(&self, display: &mut D) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = BinaryColor>;
}

const DEFAULT_FONT_WIDTH: usize = 6;
const DEFAULT_TEXT_STYLE: MonoTextStyle<BinaryColor> = MonoTextStyleBuilder::new()
    .font(&FONT_6X10)
    .text_color(BinaryColor::On)
    .build();

pub fn draw_message_screen<D: DrawTarget<Color = BinaryColor>>(
    display: &mut D,
    message: &str,
) -> Result<(), D::Error> {
    let max_line_length = display.bounding_box().size.width as usize / DEFAULT_FONT_WIDTH;
    let formatted_message = add_newlines_to_string::<100>(message, max_line_length);
    let centred_text_style = TextStyleBuilder::new()
        .alignment(Alignment::Center)
        .baseline(Baseline::Middle)
        .build();

    let line_offset_pixels =
        (formatted_message.lines().count() - 1) as i32 * DEFAULT_TEXT_STYLE.line_height() as i32;
    let x_pos = display.bounding_box().size.width as i32 / 2;
    let y_pos = display.bounding_box().size.height as i32 / 2 - line_offset_pixels;
    Text::with_text_style(
        &formatted_message,
        Point::new(x_pos, y_pos),
        DEFAULT_TEXT_STYLE,
        centred_text_style,
    )
    .draw(display)?;
    Ok(())
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
