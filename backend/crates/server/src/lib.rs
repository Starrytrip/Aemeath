//! HTTP routing, application bootstrap and blog features.
mod app;
pub use app::router;

pub mod bootstrap;
pub mod documents;
pub mod handler;
pub mod media_migration;
pub mod routes;
pub mod search;
