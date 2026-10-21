//! Helpers shared by unit tests and integration tests through the `test-util` feature. The seed fixture
//! is also compiled into debug builds, for `timetracker dev seed`.

#[cfg(any(test, feature = "test-util"))]
mod fixtures;
pub mod seed;

#[cfg(any(test, feature = "test-util"))]
pub use fixtures::*;
