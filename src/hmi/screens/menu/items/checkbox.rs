use crate::hmi::screens::menu::items::{DrawableHighlighted, MenuItem, MenuItemData, SelectedData};
use crate::hmi::screens::menu::MenuStyle;
use core::fmt;
use core::fmt::{Debug, Display, Formatter};
use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::geometry::{Point, Size};
use embedded_graphics::pixelcolor::PixelColor;
use embedded_graphics::prelude::Primitive;
use embedded_graphics::primitives::{PrimitiveStyleBuilder, Rectangle};
use embedded_graphics::text::renderer::TextRenderer;
use embedded_graphics::text::{Alignment, Baseline, Text, TextStyleBuilder};
use embedded_graphics::Drawable;
use embedded_layout::View;

#[derive(PartialEq, Clone, Copy)]
pub struct CheckboxItem<'a, C>
where
    C: PixelColor,
{
    label: &'static str,
    position: Point,
    menu_style: MenuStyle<'a, C>,
    checkbox_state: bool,
}

impl<C> CheckboxItem<'_, C>
where
    C: PixelColor,
{
    pub const fn new<'a>(label: &'static str, menu_style: MenuStyle<'a, C>) -> CheckboxItem<'a, C> {
        let initial_state = false;
        CheckboxItem {
            label,
            position: Point::zero(),
            menu_style,
            checkbox_state: initial_state,
        }
    }
}

impl<C> MenuItem for CheckboxItem<'_, C>
where
    C: PixelColor,
{
    fn label(&self) -> &'static str {
        self.label
    }
}

impl<C: PixelColor> Debug for CheckboxItem<'_, C> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "[\"{}\":Checkbox]", self.label)
    }
}

impl<C: PixelColor> Display for CheckboxItem<'_, C> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.label)
    }
}

impl<C: PixelColor> View for CheckboxItem<'_, C> {
    fn translate_impl(&mut self, by: Point) {
        self.position += by;
    }

    fn bounds(&self) -> Rectangle {
        self.menu_style
            .item_character_style
            .measure_string(self.label, Point::zero(), Baseline::Bottom)
            .bounding_box
    }
}

impl<C: PixelColor> Drawable for CheckboxItem<'_, C> {
    type Color = C;
    type Output = ();

    fn draw<D>(&self, display: &mut D) -> Result<Self::Output, D::Error>
    where
        D: DrawTarget<Color = Self::Color>,
    {
        Text::with_baseline(
            self.label,
            self.position,
            self.menu_style.item_character_style,
            Baseline::Top,
        )
        .draw(display)?;

        Text::with_text_style(
            self.display_string(),
            Point::new(display.bounding_box().size().width as i32, 0),
            self.menu_style.item_character_style,
            TextStyleBuilder::new()
                .alignment(Alignment::Right)
                .baseline(Baseline::Top)
                .build(),
        )
        .draw(display)?;

        Ok(())
    }
}

impl<C: PixelColor> DrawableHighlighted for CheckboxItem<'_, C> {
    type Color = C;
    type Output = ();

    fn draw_highlighted<D>(&self, display: &mut D) -> Result<Self::Output, D::Error>
    where
        D: DrawTarget<Color = Self::Color>,
    {
        let highlight_box_style = PrimitiveStyleBuilder::new()
            .fill_color(self.menu_style.highlight_item_color)
            .build();

        Rectangle::new(
            self.position,
            Size::new(
                display.bounding_box().size().width,
                self.menu_style.highlight_text_style.line_height(),
            ),
        )
        .into_styled(highlight_box_style)
        .draw(display)?;

        Text::with_baseline(
            self.label,
            self.position,
            self.menu_style.highlight_text_style,
            Baseline::Top,
        )
        .draw(display)?;

        Text::with_text_style(
            self.display_string(),
            Point::new(display.bounding_box().size().width as i32, 0),
            self.menu_style.highlight_text_style,
            TextStyleBuilder::new()
                .alignment(Alignment::Right)
                .baseline(Baseline::Top)
                .build(),
        )
        .draw(display)?;

        Ok(())
    }
}

impl<C> MenuItemData for CheckboxItem<'_, C>
where
    C: PixelColor,
{
    fn selected(&mut self) -> SelectedData {
        self.checkbox_state = !self.checkbox_state;
        SelectedData::Checkbox(self.checkbox_state)
    }

    fn display_string(&self) -> &str {
        match self.checkbox_state {
            true => "[X]",
            false => "[ ]",
        }
    }
}
