//! the picker's choreography — the entrance, the room crossfade, and the
//! enter-game glide — now lives in the shared crate, generic over the animation
//! clock so the desktop (`Instant`) and the browser viewer (`f64` ms) run one
//! state machine. See [`superiority_ui::products::games`].

pub(in crate::app::client) use superiority_ui::products::games::{CardMotion, Motion, StageMotion};
