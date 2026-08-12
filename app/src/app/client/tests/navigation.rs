use crate::app::client::ui_navigation;

#[test]
fn tab_sizing_matches_the_uppercase_title() {
    assert_eq!(ui_navigation::tab_width("General", false), 182.0);
    assert_eq!(ui_navigation::tab_width(&"x".repeat(40), false), 258.0);
    assert_eq!(
        ui_navigation::tab_width("aaaaaaaaaaß", false),
        12.0_f32.mul_add(8.1, 98.0)
    );
}
