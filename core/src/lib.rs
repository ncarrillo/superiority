// derive output refers to this crate by its library name.
extern crate self as superiority_core;

pub mod error;
pub mod games;
pub mod platform;
pub mod product;
pub mod session;

pub use error::{Error, Result};
pub use product::Product;

// The paths this tree used before the modules moved under `platform`, `games`,
// and `session`. They are kept resolvable rather than rewritten because:
//
//   - `bsn-derive` writes `::superiority_core::bsn::` into every type it derives,
//   - the schema generator writes `use superiority_core::bsn::` into 35 generated
//     files, and
//   - four sibling crates import these modules by path.
//
// Keeping them is what makes the move a move instead of a rewrite of everything
// that ever referred to it.
pub use games::sc2::{bsn, chat, metadata, native};
pub use platform::{auth, bgs, wire};
pub use session::{observer, worker as connection};
