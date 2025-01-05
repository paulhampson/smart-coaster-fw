use crate::application::application_state::ApplicationState;
use crate::application::messaging::{
    ApplicationChannelSubscriber, ApplicationData, ApplicationMessage,
};
use crate::hmi::messaging::HmiMessage;
use crate::hmi::rotary_encoder::Direction;
use crate::hmi::screens;
use crate::hmi::screens::settings::TestEnum;
use core::fmt::Write;
use defmt::{debug, error, trace, warn};
use embassy_futures::select::{select, Either};
use embassy_sync::pubsub::WaitResult;
use embassy_time::{Duration, Instant, Ticker};
use embedded_graphics::geometry::Point;
use embedded_graphics::mono_font::ascii::FONT_6X10;
use embedded_graphics::mono_font::{MonoTextStyle, MonoTextStyleBuilder};
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::text::renderer::TextRenderer;
use embedded_graphics::text::{Alignment, Baseline, Text, TextStyleBuilder};
use embedded_graphics::Drawable;
use embedded_menu::interaction::{Action, Interaction, Navigation};
use embedded_menu::Menu;
use heapless::String;
use micromath::F32Ext;
use sh1106::mode::GraphicsMode;

pub struct DisplayManager<'a, DI>
where
    DI: sh1106::interface::DisplayInterface,
{
    app_channel_subscriber: ApplicationChannelSubscriber<'static>,
    display: GraphicsMode<DI>,
    text_style: MonoTextStyle<'a, BinaryColor>,

    display_state: ApplicationState,

    cw_count: u32,
    ccw_count: u32,
    btn_press_count: u32,
    last_weight: f32,
    last_update: Instant,
    consumption: f32,
    consumption_rate: f32,
    total_consumed: f32,
}

impl<DI> DisplayManager<'_, DI>
where
    DI: sh1106::interface::DisplayInterface,
{
    const FRAME_TIMING_MS: u32 = 1000 / 30;
    const DEFAULT_FONTH_WIDTH: usize = 6;

    pub fn new(
        mut display: GraphicsMode<DI>,
        app_channel_subscriber: ApplicationChannelSubscriber<'static>,
    ) -> Self {
        let _ = display.init().map_err(|_| error!("Failed to init display"));
        let _ = display
            .flush()
            .map_err(|_| error!("Failed to flush display"));

        Self {
            app_channel_subscriber,
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
            consumption: 0.0,
            consumption_rate: 0.0,
            total_consumed: 0.0,
        }
    }

    pub async fn set_display_state(&mut self, display_state: ApplicationState) {
        debug!("Display state: {:?}", display_state);
        self.display_state = display_state;
        self.update_now().await;
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

    pub async fn update_screen(&mut self) {
        if self.last_update.elapsed().as_millis() < Self::FRAME_TIMING_MS as u64 {
            return;
        }
        self.update_now().await;
    }

    async fn update_now(&mut self) {
        self.display.clear();
        match self.display_state {
            ApplicationState::Startup => self.draw_message_screen("Starting up..."),
            ApplicationState::WaitingForActivity => {
                self.draw_message_screen("Waiting for activity")
            }
            ApplicationState::TestScreen => self.draw_test_screen(),
            ApplicationState::Tare => {
                self.draw_message_screen("Remove items from device and press button")
            }
            ApplicationState::Calibration(calibration_mass_grams) => {
                let mut message_string = String::<40>::new();
                write!(
                    message_string,
                    "Put {}g on device then press button.",
                    calibration_mass_grams
                )
                .expect("String too long");
                self.draw_message_screen(&message_string);
            }
            ApplicationState::CalibrationDone => self.draw_message_screen("Calibration complete"),
            ApplicationState::Wait => self.draw_wait_screen(),
            ApplicationState::ErrorScreenWithMessage(s) => self.draw_message_screen(s),
            ApplicationState::VesselPlaced => self.draw_monitoring_screen(),
            ApplicationState::VesselRemoved => self.draw_monitoring_screen(),
            ApplicationState::Settings => self.draw_setting_screen().await,
        }

        let _ = self
            .display
            .flush()
            .map_err(|_| error!("Display flush failed"));
        self.last_update = Instant::now();
    }

    pub fn add_newlines_to_string<const S: usize>(
        input: &str,
        max_line_length: usize,
    ) -> String<S> {
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

        let line_offset_pixels =
            (formatted_message.lines().count() - 1) as i32 * self.text_style.line_height() as i32;
        let x_pos = self.display.get_dimensions().0 as i32 / 2;
        let y_pos = self.display.get_dimensions().1 as i32 / 2 - line_offset_pixels;
        Text::with_text_style(
            &*formatted_message,
            Point::new(x_pos, y_pos),
            self.text_style,
            centred_text_style,
        )
        .draw(&mut self.display)
        .unwrap();
        trace!("Draw message screen done");
    }

    fn draw_test_screen(&mut self) {
        let mut count_string = String::<32>::new();

        count_string.clear();
        write!(&mut count_string, "CW Count = {}", self.cw_count).unwrap();
        Text::with_baseline(
            count_string.as_str(),
            Point::new(0, 0),
            self.text_style,
            Baseline::Top,
        )
        .draw(&mut self.display)
        .unwrap();

        count_string.clear();
        write!(&mut count_string, "CCW Count = {}", self.ccw_count).unwrap();
        Text::with_baseline(
            count_string.as_str(),
            Point::new(0, 16),
            self.text_style,
            Baseline::Top,
        )
        .draw(&mut self.display)
        .unwrap();

        count_string.clear();
        write!(&mut count_string, "Press Count = {}", self.btn_press_count).unwrap();
        Text::with_baseline(
            count_string.as_str(),
            Point::new(0, 32),
            self.text_style,
            Baseline::Top,
        )
        .draw(&mut self.display)
        .unwrap();

        count_string.clear();
        write!(
            &mut count_string,
            "Weight = {:.0}g",
            self.last_weight.round()
        )
        .unwrap();
        Text::with_baseline(
            count_string.as_str(),
            Point::new(0, 48),
            self.text_style,
            Baseline::Top,
        )
        .draw(&mut self.display)
        .unwrap();
    }

    fn draw_wait_screen(&mut self) {
        self.draw_message_screen("Please wait...");
    }

    fn draw_monitoring_screen(&mut self) {
        let mut string_buffer = String::<100>::new();
        let centred_text_style = TextStyleBuilder::new()
            .alignment(Alignment::Center)
            .baseline(Baseline::Middle)
            .build();

        let central_x_pos = self.display.get_dimensions().0 as i32 / 2;
        let central_y_pos = self.display.get_dimensions().1 as i32 / 2;
        let target_y_pos =
            central_y_pos - (2f32 * self.text_style.line_height() as f32).round() as i32;
        let centre_point = Point::new(central_x_pos, target_y_pos);

        match self.display_state {
            ApplicationState::VesselPlaced => {
                write!(string_buffer, "Vessel placed\n").unwrap();
            }
            ApplicationState::VesselRemoved => {
                write!(string_buffer, "Vessel removed\n").unwrap();
            }
            _ => {}
        };
        write!(string_buffer, "Rate: {:.0} ml/hr\n", self.consumption_rate).unwrap();
        write!(string_buffer, "Last drink: {:.0} ml\n", self.consumption).unwrap();
        write!(string_buffer, "Total: {:.0} ml", self.total_consumed).unwrap();
        Text::with_text_style(
            string_buffer.as_str(),
            centre_point,
            self.text_style,
            centred_text_style,
        )
        .draw(&mut self.display)
        .unwrap();
    }

    async fn draw_setting_screen(&mut self) {
        let mut menu = Menu::with_style("Menu", screens::menu_style())
            .add_item("Foo", ">", |_| 1)
            .add_item("Check this 1", false, |b| 20 + b as i32)
            .add_section_title("===== Section =====")
            .add_item("Check this 2", false, |b| 30 + b as i32)
            .add_item("Check this 3", TestEnum::A, |b| 40 + b as i32)
            .add_item("Check this 4", TestEnum::A, |b| 40 + b as i32)
            .add_item("Check this 5", TestEnum::A, |b| 40 + b as i32)
            .add_item("Check this 6", TestEnum::A, |b| 40 + b as i32)
            .build();

        self.display.clear();
        menu.update(&self.display);
        menu.draw(&mut self.display)
            .unwrap_or_else(|_| error!("Setting menu draw failed"));
        let _ = self
            .display
            .flush()
            .map_err(|_| error!("Display flush failed"));

        let mut update_ticker = Ticker::every(Duration::from_millis(10000));

        loop {
            let wait_result = select(
                self.app_channel_subscriber.next_message(),
                update_ticker.next(),
            )
            .await;
            match wait_result {
                Either::First(w) => match w {
                    WaitResult::Lagged(c) => {
                        warn! {"Missed {} messages", c};
                    }
                    WaitResult::Message(message) => match message {
                        ApplicationMessage::HmiInput(hmi_message) => match hmi_message {
                            HmiMessage::EncoderUpdate(direction) => match direction {
                                Direction::Clockwise => {
                                    menu.interact(Interaction::Navigation(Navigation::Next));
                                }
                                Direction::CounterClockwise => {
                                    menu.interact(Interaction::Navigation(Navigation::Previous));
                                }
                                Direction::None => {}
                            },
                            HmiMessage::PushButtonPressed(pressed) => {
                                if pressed {
                                    let selected = menu.selected_value();
                                    debug!("Selected {}", selected);
                                    menu.interact(Interaction::Action(Action::Select));
                                    if selected == 1 {
                                        break;
                                    }
                                }
                            }
                        },
                        _ => {}
                    },
                },
                Either::Second(_) => {}
            }

            self.display.clear();
            menu.update(&self.display);
            menu.draw(&mut self.display)
                .unwrap_or_else(|_| error!("Setting menu draw failed"));
            let _ = self
                .display
                .flush()
                .map_err(|_| error!("Display flush failed"));
        }
    }

    pub async fn run(&mut self) {
        self.update_screen().await;
        let mut update_ticker = Ticker::every(Duration::from_millis(200));

        loop {
            let wait_result = select(
                self.app_channel_subscriber.next_message(),
                update_ticker.next(),
            )
            .await;
            match wait_result {
                Either::First(w) => match w {
                    WaitResult::Lagged(count) => {
                        warn! {"Display lost {} messages from HMI channel", count}
                    }
                    WaitResult::Message(message) => match message {
                        ApplicationMessage::HmiInput(hmi_message) => match hmi_message {
                            HmiMessage::EncoderUpdate(direction) => {
                                trace!("Encoder update");
                                if direction == Direction::Clockwise {
                                    self.input_up();
                                }
                                if direction == Direction::CounterClockwise {
                                    self.input_down();
                                }
                            }

                            HmiMessage::PushButtonPressed(is_pressed) => {
                                debug!("Button pressed {:?} ", is_pressed);
                                if is_pressed {
                                    self.input_press();
                                }
                            }
                        },
                        ApplicationMessage::ApplicationStateUpdate(new_state) => {
                            trace!("Set display state");
                            self.set_display_state(new_state).await;
                        }
                        ApplicationMessage::ApplicationDataUpdate(data_update) => {
                            trace!("App data update");
                            match data_update {
                                ApplicationData::Weight(w) => {
                                    self.input_weight(w);
                                }
                                ApplicationData::ConsumptionRate(r) => {
                                    self.consumption_rate = r;
                                }
                                ApplicationData::Consumption(r) => {
                                    self.consumption = r;
                                }
                                ApplicationData::TotalConsumed(r) => {
                                    self.total_consumed = r;
                                }
                            }
                        }
                        ApplicationMessage::WeighSystemRequest(..) => {}
                    },
                },
                Either::Second(_) => {}
            }
            self.update_screen().await;
        }
    }
}
