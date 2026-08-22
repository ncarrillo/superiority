//! Per-product desktop UI and interaction policy.
//!
//! The shell and session lifecycle stay in `client`; a product owns concepts
//! that another product cannot represent without empty or invented fields.

pub(in crate::app::client) mod sc2;
pub(in crate::app::client) mod scr;
pub(in crate::app::client) mod wc3;
