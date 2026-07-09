//! Typed client for the Kaiten API (<https://developers.kaiten.ru>).

mod client;
mod error;

pub mod api;
pub mod models;

pub use client::KaitenClient;
pub use error::{KaitenError, Result};
pub use models::*;
pub use api::cards::{CardFilter, CreateCard, UpdateCard};
