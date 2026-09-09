// CLI argument spec types with FromStr impls for clap integration.
// Submodules group related specs; all public types are re-exported here.

mod parse;

pub mod drawing;
pub mod layout;
pub mod misc;

pub use drawing::{DrawImageSpec, DrawLineSpec, DrawRectSpec, WatermarkSpec};
pub use layout::{
    BookletSpec, DuplexFlip, GridOrder, NupSpec, Orientation, TileAlign, TileMarks, TileOrder,
    TileSpec,
};

// The spec-key lists are re-exported for the `--help` drift guards in
// `main.rs::help_tests`, which assert the help text against the parser's own key
// constants rather than a copy of them (bug-0005). Nothing outside the tests
// needs them — `layout.rs` uses the constants directly — so gate the re-export
// and keep non-test builds warning-free.
#[cfg(test)]
pub use layout::{BOOKLET_KEYS, NUP_KEYS, TILE_KEYS};
pub use misc::{BlankPageSpec, OverlaySpec, PadFileSpec, PadToSpec};
