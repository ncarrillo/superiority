//! Product-neutral UI primitives.

pub mod animation;
pub mod assets;
pub mod scrollbar;
pub mod text_input;

pub use scrollbar::{
    ScrollableHandle, ScrollbarAutoHide, ScrollbarAxes, ScrollbarColors, ScrollbarRevealPolicy,
    ScrollbarStyle, Scrollbars, ShowScrollbar, WithScrollbar,
};
