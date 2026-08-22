//! StarCraft II desktop policy and adapters.
//!
//! Feature modules move below this boundary as their shared session callers are
//! made product-explicit. The reusable renderers already live in
//! `superiority_ui::products::sc2`.

use super::super::*;

pub(in crate::app::client) mod channel;
pub(in crate::app::client) mod chat;
pub(in crate::app) mod chrome;
pub(in crate::app::client) mod composer;
pub(in crate::app::client) mod join;
pub(in crate::app::client) mod navigation;
pub(in crate::app::client) mod roster;
pub(in crate::app::client) mod social;
