use backitem::BackItem;
use checkbox::CheckboxItem;
use core::fmt::{Display, Formatter};
use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::geometry::Point;
use embedded_graphics::prelude::PixelColor;
use embedded_graphics::primitives::Rectangle;
use embedded_graphics::Drawable;
use embedded_layout::View;
use multi_option::MultiOptionItem;
use section::SectionItem;
use submenu::SubmenuItem;

pub mod backitem;
pub mod checkbox;
pub mod multi_option;
pub mod section;
pub mod submenu;

#[derive(Clone, Copy, PartialEq)]
pub enum SelectedData {
    Checkbox(bool),
    Submenu(),
    Back(),
    MultiOption(usize),
    Section(),
}

pub trait MenuItem: View + Drawable + DrawableHighlighted + Display + MenuItemData {
    fn label(&self) -> &'static str;
}

pub trait DrawableHighlighted {
    type Color: PixelColor;
    type Output;

    fn draw_highlighted<D>(&self, target: &mut D) -> Result<Self::Output, D::Error>
    where
        D: DrawTarget<Color = Self::Color>;
}

pub trait MenuItemData {
    fn selected(&mut self) -> SelectedData;

    fn display_string(&self) -> &str;
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MenuItems<'a, C>
where
    C: PixelColor,
{
    Checkbox(CheckboxItem<'a, C>),
    Submenu(SubmenuItem<'a, C>),
    Selector(MultiOptionItem<'a, C>),
    Section(SectionItem<'a, C>),
    Back(BackItem<'a, C>),
}

impl<C> View for MenuItems<'_, C>
where
    C: PixelColor,
{
    fn translate_impl(&mut self, by: Point) {
        match self {
            MenuItems::Checkbox(item) => item.translate_impl(by),
            MenuItems::Submenu(item) => item.translate_impl(by),
            MenuItems::Selector(item) => item.translate_impl(by),
            MenuItems::Section(item) => item.translate_impl(by),
            MenuItems::Back(item) => item.translate_impl(by),
        }
    }

    fn bounds(&self) -> Rectangle {
        match self {
            MenuItems::Checkbox(item) => item.bounds(),
            MenuItems::Submenu(item) => item.bounds(),
            MenuItems::Selector(item) => item.bounds(),
            MenuItems::Section(item) => item.bounds(),
            MenuItems::Back(item) => item.bounds(),
        }
    }
}

impl<C> Display for MenuItems<'_, C>
where
    C: PixelColor,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self {
            MenuItems::Checkbox(item) => Display::fmt(&item, f),
            MenuItems::Submenu(item) => Display::fmt(&item, f),
            MenuItems::Selector(item) => Display::fmt(&item, f),
            MenuItems::Section(item) => Display::fmt(&item, f),
            MenuItems::Back(item) => Display::fmt(&item, f),
        }
    }
}

impl<C> MenuItemData for MenuItems<'_, C>
where
    C: PixelColor,
{
    fn selected(&mut self) -> SelectedData {
        match self {
            MenuItems::Checkbox(item) => item.selected(),
            MenuItems::Submenu(item) => item.selected(),
            MenuItems::Selector(item) => item.selected(),
            MenuItems::Section(item) => item.selected(),
            MenuItems::Back(item) => item.selected(),
        }
    }

    fn display_string(&self) -> &str {
        match self {
            MenuItems::Checkbox(item) => item.display_string(),
            MenuItems::Submenu(item) => item.display_string(),
            MenuItems::Selector(item) => item.display_string(),
            MenuItems::Section(item) => item.display_string(),
            MenuItems::Back(item) => item.display_string(),
        }
    }
}

impl<C> MenuItem for MenuItems<'_, C>
where
    C: PixelColor,
{
    fn label(&self) -> &'static str {
        match self {
            MenuItems::Checkbox(item) => item.label(),
            MenuItems::Submenu(item) => item.label(),
            MenuItems::Selector(item) => item.label(),
            MenuItems::Section(item) => item.label(),
            MenuItems::Back(item) => item.label(),
        }
    }
}

impl<C> Drawable for MenuItems<'_, C>
where
    C: PixelColor,
{
    type Color = C;
    type Output = ();

    fn draw<D>(&self, display: &mut D) -> Result<Self::Output, D::Error>
    where
        D: DrawTarget<Color = Self::Color>,
    {
        match self {
            MenuItems::Checkbox(item) => item.draw(display),
            MenuItems::Submenu(item) => item.draw(display),
            MenuItems::Selector(item) => item.draw(display),
            MenuItems::Section(item) => item.draw(display),
            MenuItems::Back(item) => item.draw(display),
        }
    }
}

impl<C: PixelColor> DrawableHighlighted for MenuItems<'_, C> {
    type Color = C;
    type Output = ();

    fn draw_highlighted<D>(&self, display: &mut D) -> Result<Self::Output, D::Error>
    where
        D: DrawTarget<Color = Self::Color>,
    {
        match self {
            MenuItems::Checkbox(item) => item.draw_highlighted(display),
            MenuItems::Submenu(item) => item.draw_highlighted(display),
            MenuItems::Selector(item) => item.draw_highlighted(display),
            MenuItems::Section(item) => item.draw_highlighted(display),
            MenuItems::Back(item) => item.draw_highlighted(display),
        }
    }
}
