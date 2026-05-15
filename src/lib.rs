/// Chord Shifter shared library.
///
/// Exposes the `song` and `auth` modules so they can be used by both the
/// WASM frontend binary (`src/main.rs`) and the Axum backend binary
/// (`src/bin/server.rs`) without duplicating source files.
pub mod auth;
pub mod song;
