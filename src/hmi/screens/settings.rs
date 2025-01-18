use defmt::error;
use embedded_graphics::mono_font::ascii::{FONT_6X10, FONT_7X13_BOLD};
use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::Drawable;
use sh1106::prelude::GraphicsMode;
use simple_embedded_graphics_menu::items::SelectedData;
use simple_embedded_graphics_menu::{Menu, MenuStyle};

#[derive(Copy, Clone, Debug)]
pub enum SettingMenuIdentifier {
    Root,
    EnterTestScreen,
    None,
}

pub struct SettingMenu {
    menu: Menu<'static, BinaryColor, SettingMenuIdentifier>,
}

impl SettingMenu {
    pub fn new() -> Self {
        Self {
            menu: Self::build_menu(),
        }
    }

    fn build_menu() -> Menu<'static, BinaryColor, SettingMenuIdentifier> {
        let heading_style = MonoTextStyle::new(&FONT_7X13_BOLD, BinaryColor::On);
        let item_style = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);
        let highlighted_item_style = MonoTextStyle::new(&FONT_6X10, BinaryColor::Off);

        let menu_style = MenuStyle::new(
            BinaryColor::Off,
            heading_style,
            item_style,
            BinaryColor::On,
            BinaryColor::On,
            highlighted_item_style,
            BinaryColor::Off,
        );

        let mut menu = Menu::new("Settings", SettingMenuIdentifier::Root, menu_style);
        menu.add_action("Device Test Mode", SettingMenuIdentifier::EnterTestScreen);
        menu.add_exit("Exit", SettingMenuIdentifier::None);
        menu
    }

    pub fn input_up(&mut self) {
        self.menu.navigate_up();
    }

    pub fn input_down(&mut self) {
        self.menu.navigate_down();
    }

    pub fn input_select(&mut self) -> Option<SelectedData<SettingMenuIdentifier>> {
        self.menu.select_item()
    }

    pub(crate) fn draw<DI>(&self, display: &mut GraphicsMode<DI>)
    where
        DI: sh1106::interface::DisplayInterface,
    {
        self.menu
            .draw(display)
            .unwrap_or_else(|_| error!("Setting menu draw failed"));
    }
}
