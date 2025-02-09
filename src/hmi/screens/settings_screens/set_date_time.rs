use crate::application::application_state::ApplicationState;
use crate::hmi::messaging::{UiActionChannelPublisher, UiActionsMessage};
use crate::hmi::screens::{UiDrawer, UiInput, UiInputHandler};
use crate::rtc;
use chrono::{Datelike, Month, Timelike};
use core::cmp::PartialEq;
use core::fmt::Write;
use ds323x::NaiveDateTime;
use embedded_graphics::geometry::Point;
use embedded_graphics::mono_font::ascii::FONT_6X10;
use embedded_graphics::mono_font::MonoTextStyleBuilder;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::text::{Baseline, Text};
use embedded_graphics::Drawable;
use heapless::String;
use sh1106::interface::DisplayInterface;
use sh1106::mode::GraphicsMode;
use strum::{EnumIter, IntoEnumIterator};

#[derive(EnumIter, Debug, PartialEq)]
enum DateTimeSettingElement {
    Hour,
    Minute,
    Seconds,
    Year,
    Month,
    Day,
    Save,
}

impl DateTimeSettingElement {
    pub fn increment(&self, dt: NaiveDateTime) -> NaiveDateTime {
        match self {
            DateTimeSettingElement::Hour => dt.with_hour(dt.hour() + 1).unwrap_or(dt),
            DateTimeSettingElement::Minute => dt.with_minute(dt.minute() + 1).unwrap_or(dt),
            DateTimeSettingElement::Seconds => dt.with_second(dt.second() + 1).unwrap_or(dt),
            DateTimeSettingElement::Day => dt.with_day(dt.day() + 1).unwrap_or(dt),
            DateTimeSettingElement::Month => dt.with_month(dt.month() + 1).unwrap_or(dt),
            DateTimeSettingElement::Year => {
                if let Some(new_dt) = dt.with_year(dt.year() + 1) {
                    new_dt
                } else {
                    // with_year can return None when the date doesn't exist (e.g. 29 Feb), so set
                    // it to first of the month which always exists.
                    let new_dt = dt.with_day(1).unwrap().with_year(dt.year() + 1).unwrap();
                    new_dt
                }
            }
            _ => dt,
        }
    }

    pub fn decrement(&self, dt: NaiveDateTime) -> NaiveDateTime {
        match self {
            DateTimeSettingElement::Hour => dt.with_hour(dt.hour() - 1).unwrap_or(dt),
            DateTimeSettingElement::Minute => dt.with_minute(dt.minute() - 1).unwrap_or(dt),
            DateTimeSettingElement::Seconds => dt.with_second(dt.second() - 1).unwrap_or(dt),
            DateTimeSettingElement::Day => dt.with_day(dt.day() - 1).unwrap_or(dt),
            DateTimeSettingElement::Month => dt.with_month(dt.month() - 1).unwrap_or(dt),
            DateTimeSettingElement::Year => {
                if let Some(new_dt) = dt.with_year(dt.year() - 1) {
                    new_dt
                } else {
                    // with_year can return None when the date doesn't exist (e.g. 29 Feb), so set
                    // it to first of the month which always exists.
                    let new_dt = dt.with_day(1).unwrap().with_year(dt.year() - 1).unwrap();
                    new_dt
                }
            }
            _ => dt,
        }
    }
}

pub struct SetDateTimeScreen {
    local_datetime: NaiveDateTime,
    active_element: DateTimeSettingElement,
    element_idx: usize,
}

impl SetDateTimeScreen {
    pub fn new() -> Self {
        Self {
            local_datetime: NaiveDateTime::default(),
            active_element: DateTimeSettingElement::iter().nth(0).unwrap(),
            element_idx: 0,
        }
    }

    pub fn reset(&mut self, dt: NaiveDateTime) {
        self.local_datetime = dt;
        self.active_element = DateTimeSettingElement::iter().nth(0).unwrap();
        self.element_idx = 0;
    }
}

impl UiInputHandler for SetDateTimeScreen {
    async fn ui_input_handler(
        &mut self,
        input: UiInput,
        ui_channel_publisher: &UiActionChannelPublisher<'_>,
    ) {
        match input {
            UiInput::EncoderClockwise => {
                self.local_datetime = self.active_element.increment(self.local_datetime);
            }
            UiInput::EncoderCounterClockwise => {
                self.local_datetime = self.active_element.decrement(self.local_datetime);
            }
            UiInput::ButtonPress => {
                if self.active_element == DateTimeSettingElement::Save {
                    rtc::accessor::set_date_time(self.local_datetime);
                    ui_channel_publisher.publish_immediate(UiActionsMessage::StateChangeRequest(
                        ApplicationState::Settings,
                    ))
                } else {
                    self.element_idx += 1;
                    self.element_idx %= DateTimeSettingElement::iter().len();
                    self.active_element = DateTimeSettingElement::iter()
                        .nth(self.element_idx)
                        .unwrap();
                }
            }
            UiInput::DateTimeUpdate(dt) => {
                self.reset(dt);
            }
            _ => {}
        }
    }
}

impl UiDrawer for SetDateTimeScreen {
    fn draw<DI>(&self, display: &mut GraphicsMode<DI>)
    where
        DI: DisplayInterface,
    {
        let active_element_style = MonoTextStyleBuilder::new()
            .font(&FONT_6X10)
            .text_color(BinaryColor::Off)
            .background_color(BinaryColor::On)
            .build();
        let text_style = MonoTextStyleBuilder::new()
            .font(&FONT_6X10)
            .text_color(BinaryColor::On)
            .build();

        let mut string_buffer = String::<32>::new();
        let mut next_point = Point::new(0, 0);

        let style_to_use = if self.active_element == DateTimeSettingElement::Hour {
            active_element_style
        } else {
            text_style
        };
        string_buffer.clear();
        write!(&mut string_buffer, "{:01}:", self.local_datetime.hour()).unwrap();
        next_point = Text::with_baseline(
            string_buffer.as_str(),
            next_point,
            style_to_use,
            Baseline::Top,
        )
        .draw(display)
        .unwrap();

        let style_to_use = if self.active_element == DateTimeSettingElement::Minute {
            active_element_style
        } else {
            text_style
        };
        string_buffer.clear();
        write!(&mut string_buffer, "{:01}:", self.local_datetime.minute()).unwrap();
        next_point = Text::with_baseline(
            string_buffer.as_str(),
            next_point,
            style_to_use,
            Baseline::Top,
        )
        .draw(display)
        .unwrap();

        let style_to_use = if self.active_element == DateTimeSettingElement::Seconds {
            active_element_style
        } else {
            text_style
        };
        string_buffer.clear();
        write!(&mut string_buffer, "{:01}\n", self.local_datetime.second()).unwrap();
        next_point = Text::with_baseline(
            string_buffer.as_str(),
            next_point,
            style_to_use,
            Baseline::Top,
        )
        .draw(display)
        .unwrap();

        next_point.x = 0;
        let style_to_use = if self.active_element == DateTimeSettingElement::Year {
            active_element_style
        } else {
            text_style
        };
        string_buffer.clear();
        write!(&mut string_buffer, "{:04}-", self.local_datetime.year()).unwrap();
        next_point = Text::with_baseline(
            string_buffer.as_str(),
            next_point,
            style_to_use,
            Baseline::Top,
        )
        .draw(display)
        .unwrap();

        let style_to_use = if self.active_element == DateTimeSettingElement::Month {
            active_element_style
        } else {
            text_style
        };
        string_buffer.clear();
        write!(
            &mut string_buffer,
            "{:^9}-",
            Month::try_from(self.local_datetime.month() as u8)
                .unwrap()
                .name()
        )
        .unwrap();
        next_point = Text::with_baseline(
            string_buffer.as_str(),
            next_point,
            style_to_use,
            Baseline::Top,
        )
        .draw(display)
        .unwrap();

        let style_to_use = if self.active_element == DateTimeSettingElement::Day {
            active_element_style
        } else {
            text_style
        };
        string_buffer.clear();
        write!(&mut string_buffer, "{:02}\n\n", self.local_datetime.day()).unwrap();
        next_point = Text::with_baseline(
            string_buffer.as_str(),
            next_point,
            style_to_use,
            Baseline::Top,
        )
        .draw(display)
        .unwrap();

        next_point.x = 0;
        let style_to_use = if self.active_element == DateTimeSettingElement::Save {
            active_element_style
        } else {
            text_style
        };
        string_buffer.clear();
        write!(&mut string_buffer, "[Set Time]").unwrap();
        Text::with_baseline(
            string_buffer.as_str(),
            next_point,
            style_to_use,
            Baseline::Top,
        )
        .draw(display)
        .unwrap();
    }
}
