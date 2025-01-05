use embedded_graphics::mono_font::ascii::FONT_7X13_BOLD;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_menu::interaction::programmed::Programmed;
use embedded_menu::selection_indicator::style::Line;
use embedded_menu::selection_indicator::StaticPosition;
use embedded_menu::{DisplayScrollbar, MenuStyle};

pub mod settings;

pub(crate) const fn menu_style<R>() -> MenuStyle<Line, Programmed, StaticPosition, R, BinaryColor> {
    MenuStyle::new(BinaryColor::On)
        .with_scrollbar_style(DisplayScrollbar::Auto)
        .with_input_adapter(Programmed)
        .with_title_font(&FONT_7X13_BOLD)
}
