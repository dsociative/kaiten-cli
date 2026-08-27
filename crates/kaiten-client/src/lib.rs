//! Typed client for the Kaiten API (<https://developers.kaiten.ru>).
//!
//! Response models and [`KaitenError`] are `#[non_exhaustive]`: read their
//! public fields freely, but build them only by deserializing — new fields
//! and variants appear as the API grows without a breaking release. Request
//! types are constructed through initializers ([`CreateCard::new`],
//! [`UpdateCard::default`], [`CardFilter::default`]) and then set field by
//! field; new request fields are optional.

mod client;
mod error;

pub mod api;
pub mod models;

pub use api::cards::{CardFilter, CreateCard, UpdateCard};
pub use client::KaitenClient;
pub use error::{KaitenError, Result};
pub use models::*;
