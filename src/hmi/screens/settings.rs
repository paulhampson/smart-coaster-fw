use embedded_menu::SelectValue;

//
// pub fn create_menu<T: AsRef<str>, R>(
//     title: T,
// ) -> MenuBuilder<T, Programmed, NoItems, R, AnimatedPosition, AnimatedTriangle, BinaryColor> {
//     Menu::with_style(title, menu_style())
// }

#[derive(Copy, Clone, PartialEq, SelectValue)]
pub enum TestEnum {
    A,
    B,
    C,
}
