use crate::app::client::chrome::resize_button_frame;

#[test]
fn button_cap_insets_preserve_exact_control_bounds() {
    let source = image::RgbaImage::new(288, 76);
    for (width, height) in [(132, 36), (132, 44), (142, 42), (185, 50), (260, 42)] {
        let resized = resize_button_frame(&source, width, height);
        assert_eq!(resized.dimensions(), (width, height));
    }
}
