use core::fmt::Write;
use defmt::{debug, error, Format};
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
use crate::{HmiEventChannelReceiver};
use crate::hmi::event_channels::HmiEvents;
use crate::hmi::rotary_encoder::Direction;

const FRAME_TIMING_MS:u64 = 1000 / 30;

#[derive(Debug, Format, Clone, Copy, PartialEq)]
pub enum DisplayState {
    Startup,
    Home,
    Tare,
    Calibration(u32),
    CalibrationDone,
    Wait,
}

struct DisplayManager<'a, DI: sh1106::interface::DisplayInterface> {
    display: &'a mut GraphicsMode<DI>,
    text_style: MonoTextStyle<'a, BinaryColor>,

    display_state: DisplayState,

    cw_count: u32,
    ccw_count: u32,
    btn_press_count: u32,
    last_weight: f32,
    last_update: Instant,
}

impl<'a, DI: sh1106::interface::DisplayInterface> DisplayManager<'a, DI>
{
    pub fn new(display: &'a mut GraphicsMode<DI>) -> Self {
        let _ = display.init().map_err(|_| error!("Failed to init display"));
        let _ = display.flush().map_err(|_| error!("Failed to flush display"));

        Self {
            display,
            text_style: MonoTextStyleBuilder::new()
                .font(&FONT_6X10)
                .text_color(BinaryColor::On)
                .build(),

            display_state: DisplayState::Startup,

            cw_count: 0,
            ccw_count: 0,
            btn_press_count: 0,
            last_weight: 0.0,
            last_update: Instant::MIN,
        }
    }

    pub fn set_display_state(&mut self, display_state: DisplayState) {
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
            DisplayState::Startup => self.draw_message_screen("Starting up..."),
            DisplayState::Home => self.draw_home_screen(),
            DisplayState::Tare => self.draw_message_screen("Remove items\nfrom device\nand press button"),
            DisplayState::Calibration(calibration_mass_grams) => {
                let mut message_string = String::<40>::new();
                write!(message_string, "Put {}g on device\nthen press button.", calibration_mass_grams)
                    .expect("String too long");
                self.draw_message_screen(&message_string);}
            DisplayState::CalibrationDone => {self.draw_message_screen("Calibration complete")}
            DisplayState::Wait => {self.draw_wait_screen()}
        }

        let _ = self.display.flush().map_err(|_| error!("Display flush failed"));
        self.last_update = Instant::now();
    }

    fn draw_message_screen(&mut self, message: &str) {
        let centred_text_style = TextStyleBuilder::new()
            .alignment(Alignment::Center)
            .baseline(Baseline::Middle)
            .build();

        let line_offset_pixels = (message.lines().count() - 1) as i32 * self.text_style.line_height() as i32;
        let x_pos = self.display.get_dimensions().0 as i32 / 2;
        let y_pos = self.display.get_dimensions().1 as i32 / 2 - line_offset_pixels;
        Text::with_text_style(message, Point::new(x_pos, y_pos), self.text_style, centred_text_style)
            .draw(self.display)
            .unwrap();
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

pub async fn display_update_handler<DI: sh1106::interface::DisplayInterface>(mut hmi_event_channel: HmiEventChannelReceiver<'_>, display: &mut GraphicsMode<DI>) {
    let mut display_manager = DisplayManager::new(display);

    display_manager.update();

    loop {
        let event = hmi_event_channel.next_message_pure().await;
        match event {
            HmiEvents::EncoderUpdate(direction) => {
                if direction == Direction::Clockwise {
                    display_manager.input_up();
                }
                if direction == Direction::CounterClockwise {
                    display_manager.input_down();
                }
            }
            HmiEvents::PushButtonPressed(is_pressed) => {
                if is_pressed {
                    display_manager.input_press();
                }
            }
            HmiEvents::WeightUpdate(weight) => {
                display_manager.input_weight(weight);
            }
            HmiEvents::ChangeDisplayState(new_state) => {
                display_manager.set_display_state(new_state);
            }
        }

        display_manager.update();
    }
}