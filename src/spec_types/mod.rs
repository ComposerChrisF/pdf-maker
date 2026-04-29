// CLI argument spec types with FromStr impls for clap integration.
// Submodules group related specs; all public types are re-exported here.

mod parse;

pub mod drawing;
pub mod layout;
pub mod misc;

pub use drawing::{DrawImageSpec, DrawLineSpec, DrawRectSpec, WatermarkSpec};
pub use layout::{BookletSpec, DuplexFlip, GridOrder, NupSpec};
pub use misc::{BlankPageSpec, OverlaySpec, PadFileSpec, PadToSpec};
