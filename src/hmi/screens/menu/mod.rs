use core::cmp::PartialEq;
use core::pin::Pin;
use items::{DrawableHighlighted, MenuItem, MenuItemData, SelectedData};

pub mod items;

use embedded_graphics::geometry::AnchorY;
use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::prelude::*;
use embedded_graphics::text::renderer::TextRenderer;
use embedded_graphics::text::{Baseline, Text};
use embedded_layout::View;
use items::backitem::BackItem;
use items::checkbox::CheckboxItem;
use items::multi_option::MultiOptionItem;
use items::section::SectionItem;
use items::submenu::SubmenuItem;
use items::MenuItems;
use trees::Tree;

pub struct Menu<'a, C>
where
    C: PixelColor,
{
    menu_tree_root: Tree<MenuItems<'a, C>>,
    menu_style: MenuStyle<'a, C>,
    menu_state: MenuState,
    active_submenu_node: Option<Tree<MenuItems<'a, C>>>,
}

impl<'a, C> Menu<'a, C>
where
    C: PixelColor,
{
    pub fn new(label: &'static str, menu_style: MenuStyle<'a, C>) -> Self {
        let tree_root = Tree::new(MenuItems::Submenu(SubmenuItem::new(label, menu_style)));
        Self {
            menu_tree_root: tree_root,
            menu_style,
            menu_state: MenuState::new(),
            active_submenu_node: None,
        }
    }

    /// Add menu item to the menu structure that will be drawn
    pub fn add_item(&mut self, item: MenuItems<'a, C>) {
        self.menu_tree_root.push_back(Tree::new(item));
        self.menu_state
            .update_item_count(self.menu_tree_root.iter().count());
    }

    /// Add checkbox as next item in the menu
    pub fn add_checkbox(&mut self, label: &'static str) {
        self.add_item(MenuItems::Checkbox(CheckboxItem::new(
            label,
            self.menu_style,
        )));
    }

    /// Add selector as next item in the menu
    pub fn add_selector(&mut self, label: &'static str, options: &'a [&'static str]) {
        self.add_item(MenuItems::Selector(MultiOptionItem::new(
            label,
            self.menu_style,
            options,
        )));
    }

    /// Add section (non-selectable item) as next item in the menu
    pub fn add_section(&mut self, label: &'static str) {
        self.add_item(MenuItems::Section(SectionItem::new(label, self.menu_style)));
    }

    /// Add a sub-menu to the menu structure that will be drawn
    pub fn add_submenu(&mut self, submenu: Menu<'a, C>) {
        self.menu_tree_root.push_back(submenu.into());
        self.menu_state
            .update_item_count(self.menu_tree_root.iter().count());
    }

    pub fn add_back(&mut self, label: &'static str) {
        self.add_item(MenuItems::Back(BackItem::new(label, self.menu_style)));
    }

    pub fn navigate_down(&mut self) {
        self.menu_state.move_down();
        if let Some(item) = self
            .get_active_submenu()
            .iter()
            .nth(self.menu_state.highlighted_item())
        {
            if let MenuItems::Section(_) = item.data() {
                self.menu_state.move_down();
            }
        }
    }

    pub fn navigate_up(&mut self) {
        self.menu_state.move_up();
        if let Some(item) = self
            .menu_tree_root
            .iter()
            .nth(self.menu_state.highlighted_item())
        {
            if let MenuItems::Section(_) = item.data() {
                self.menu_state.move_up();
            }
        }
    }

    fn get_mut_active_submenu(&mut self) -> &mut Tree<MenuItems<'a, C>> {
        let menu_tree: &mut Tree<MenuItems<'_, C>>;
        if let Some(active_tree) = self.active_submenu_node.as_mut() {
            menu_tree = active_tree;
        } else {
            menu_tree = &mut self.menu_tree_root;
        }
        menu_tree
    }

    fn get_active_submenu(&self) -> &Tree<MenuItems<'a, C>> {
        let menu_tree: &Tree<MenuItems<'_, C>>;
        if let Some(active_tree) = &self.active_submenu_node {
            menu_tree = active_tree;
        } else {
            menu_tree = &self.menu_tree_root;
        }
        menu_tree
    }

    fn navigate_to_menu(&mut self, target: Tree<MenuItems<'a, C>>) {
        self.active_submenu_node = Some(target);
        self.menu_state = MenuState::new();
        let active_tree = self.get_active_submenu();
        self.menu_state
            .update_item_count(active_tree.iter().count());
    }

    fn navigate_to_selected_submenu(&mut self) {
        let highlighted_item = self.menu_state.highlighted_item();
        if let Some(item) = self.get_active_submenu().iter().nth(highlighted_item) {
            self.active_submenu_node = Some(item.deep_clone());
            self.menu_state = MenuState::new();
            let active_tree = self.get_active_submenu();
            self.menu_state
                .update_item_count(active_tree.iter().count());
        }
    }

    fn navigate_to_root(&mut self) {
        self.active_submenu_node = None;
        self.menu_state = MenuState::new();
        let active_tree = self.get_active_submenu();
        self.menu_state
            .update_item_count(active_tree.iter().count());
    }

    pub fn select_item(&mut self) {
        let highlighted_item = self.menu_state.highlighted_item();

        let active_tree = self.get_mut_active_submenu();
        if let Some(item) = active_tree.iter_mut().nth(highlighted_item) {
            let selection_result;
            // Is there some better way to do this? Behaviour doesn't seem to match the tree crate
            // examples, but they use simple types. In any case we don't move the memory, and it
            // remains valid so doesn't violate the Pin invariants.
            unsafe {
                let item = Pin::into_inner_unchecked(item);
                selection_result = item.data_mut().selected();

                if selection_result == SelectedData::Submenu() {
                    self.navigate_to_selected_submenu();
                }
                if selection_result == SelectedData::Back() {
                    let active_menu = self.get_active_submenu();
                    if let Some(parent_menu) = active_menu.parent().as_mut() {
                        self.navigate_to_menu(parent_menu.deep_clone());
                    } else {
                        self.navigate_to_root();
                    }
                }
            }
        }
    }

    fn draw_menu<D>(
        &self,
        display: &mut D,
        menu_tree: &Tree<MenuItems<'_, C>>,
    ) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = C>,
    {
        let display_area = display.bounding_box();
        display.clear(self.menu_style.menu_background_color)?;
        let header = menu_tree.data();
        let header_height = self.menu_style.heading_character_style.line_height();
        Text::with_baseline(
            header.label(),
            Point::zero(),
            self.menu_style.heading_character_style,
            Baseline::Top,
        )
        .draw(display)?;

        let mut remaining_item_area = display_area
            .resized_height(display_area.size().height - header_height, AnchorY::Bottom);

        let mut skip_count = 0;

        if self.menu_state.highlighted_item() > 1 {
            skip_count = self.menu_state.highlighted_item() - 1;
        }
        if self.menu_state.highlighted_item() == self.menu_state.item_count()
            && self.menu_state.item_count() >= 2
        {
            skip_count = self.menu_state.highlighted_item() - 2;
        }

        let menu_iter = menu_tree.iter().skip(skip_count);

        for (id, menu_item) in menu_iter.enumerate() {
            let item_height = menu_item.data().size().height;
            if item_height > remaining_item_area.size().height {
                break;
            }

            let mut item_display = display.cropped(&remaining_item_area);
            if id + skip_count == self.menu_state.highlighted_item() {
                menu_item.data().draw_highlighted(&mut item_display)?;
            } else {
                menu_item.data().draw(&mut item_display)?;
            }

            remaining_item_area = remaining_item_area.resized_height(
                remaining_item_area.size().height - item_height,
                AnchorY::Bottom,
            );
        }

        Ok(())
    }
}

impl<C> Drawable for Menu<'_, C>
where
    C: PixelColor,
{
    type Color = C;
    type Output = ();

    fn draw<D>(&self, display: &mut D) -> Result<Self::Output, D::Error>
    where
        D: DrawTarget<Color = Self::Color>,
    {
        let active_tree = self.get_active_submenu();
        self.draw_menu(display, active_tree)?;

        Ok(())
    }
}

impl<'a, C> From<Menu<'a, C>> for Tree<MenuItems<'a, C>>
where
    C: PixelColor,
{
    fn from(menu: Menu<'a, C>) -> Tree<MenuItems<'a, C>> {
        menu.menu_tree_root
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub struct MenuStyle<'a, C> {
    pub(crate) menu_background_color: C,
    pub(crate) heading_character_style: MonoTextStyle<'a, C>,
    pub(crate) item_character_style: MonoTextStyle<'a, C>,
    pub(crate) indicator_fill_color: C,
    pub(crate) highlight_item_color: C,
    pub(crate) highlight_text_style: MonoTextStyle<'a, C>,
    pub(crate) highlight_indicator_fill_color: C,
}

impl<'a, C> MenuStyle<'a, C>
where
    C: PixelColor,
{
    pub fn new(
        menu_background_color: C,
        heading_character_style: MonoTextStyle<'a, C>,
        item_character_style: MonoTextStyle<'a, C>,
        indicator_fill_color: C,
        highlight_item_color: C,
        highlight_text_style: MonoTextStyle<'a, C>,
        highlight_indicator_fill_color: C,
    ) -> Self {
        Self {
            menu_background_color,
            heading_character_style,
            item_character_style,
            indicator_fill_color,
            highlight_item_color,
            highlight_text_style,
            highlight_indicator_fill_color,
        }
    }
}

struct MenuState {
    highlighted_item: usize,
    item_count: usize,
}

impl MenuState {
    pub fn new() -> Self {
        Self {
            highlighted_item: 0,
            item_count: 0,
        }
    }
    pub fn update_item_count(&mut self, item_count: usize) {
        self.item_count = item_count;
    }
    pub fn move_down(&mut self) {
        self.highlighted_item += 1;
        if self.highlighted_item >= self.item_count {
            self.highlighted_item = 0;
        }
    }

    pub fn move_up(&mut self) {
        if self.highlighted_item == 0 {
            self.highlighted_item = self.item_count - 1;
        } else {
            self.highlighted_item -= 1;
        }
    }

    pub fn highlighted_item(&self) -> usize {
        self.highlighted_item
    }

    pub fn item_count(&self) -> usize {
        self.item_count
    }
}
