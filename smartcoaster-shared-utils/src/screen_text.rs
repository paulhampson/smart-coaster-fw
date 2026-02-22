use embedded_graphics::{
    mono_font::{ascii::FONT_6X10, MonoTextStyle, MonoTextStyleBuilder},
    pixelcolor::BinaryColor,
    prelude::*,
    text::{Text, Alignment, Baseline, TextStyleBuilder, renderer::TextRenderer},
};
use heapless::String;

pub const DEFAULT_FONT_WIDTH: usize = 6;
pub const DEFAULT_TEXT_STYLE: MonoTextStyle<BinaryColor> = MonoTextStyleBuilder::new()
    .font(&FONT_6X10)
    .text_color(BinaryColor::On)
    .build();

pub fn draw_message_screen<D: DrawTarget<Color=BinaryColor>>(
    display: &mut D,
    message: &str,
) -> Result<(), D::Error> {
    let max_line_length = display.bounding_box().size.width as usize / DEFAULT_FONT_WIDTH;
    let formatted_message = add_newlines_to_string::<100>(message, max_line_length);
    draw_message_screen_no_reformat(display, &formatted_message, Alignment::Center)
}

pub fn draw_message_screen_no_reformat<D: DrawTarget<Color=BinaryColor>>(
    display: &mut D,
    message: &str,
    alignment: Alignment,
) -> Result<(), D::Error> {
    let centred_text_style = TextStyleBuilder::new()
        .alignment(alignment)
        .baseline(Baseline::Middle)
        .build();

    let line_count = message.lines().count().max(1) as i32;
    let line_height = DEFAULT_TEXT_STYLE.line_height() as i32;

    // With Baseline::Middle, `y` is the *middle of the first line*.
    // To center a block of N lines, shift up by half of the distance
    // between the first and last line centers: (N - 1) * line_height / 2.
    let block_center_offset = ((line_count - 1) * line_height) / 2;

    let bb = display.bounding_box();
    let screen_center_x = bb.size.width as i32 / 2;
    let screen_center_y = bb.size.height as i32 / 2;

    let x_pos: i32;
    match alignment {
        Alignment::Center => x_pos = screen_center_x,
        Alignment::Left => x_pos = 0,
        Alignment::Right => x_pos = bb.size.width as i32,
    }

    let y_pos = screen_center_y - block_center_offset;

    Text::with_text_style(
        message,
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

    let line_count = input.lines().count();
    for (i, line) in input.lines().enumerate() {
        for word in line.split_whitespace() {
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

        // don't add a newline after the last line or if we just added a new line
        if i < line_count - 1 && current_length > 0 {
            result.push('\n').unwrap();
            current_length = 0;
        }
    }


    result
}