//! Subsonic API client module

pub mod auth;
pub mod client;
pub mod models;
#[cfg(test)]
mod tests;

pub use client::SubsonicClient;
