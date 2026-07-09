//! Typed client for the Kaiten API (<https://developers.kaiten.ru>).

mod client;
mod error;

pub use client::KaitenClient;
pub use error::{KaitenError, Result};
