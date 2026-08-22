use gpui::{Div, div, prelude::*, rgb};

use crate::products::sc2::theme::{BACKGROUND, TEXT};

#[must_use]
pub(super) fn root() -> Div {
    div()
        .size_full()
        .flex()
        .flex_col()
        .overflow_hidden()
        .bg(rgb(BACKGROUND))
        .text_color(rgb(TEXT))
}
