//! `cairn-auth`: signs Cairn's sales team in through Entra ID and hands
//! the desktop app a Postgres login that expires on its own.
//!
//! Entra owns who you are. Postgres owns how long the login lasts. This
//! service sits between them and remembers nothing that outlives a
//! restart.

pub mod config;
pub mod entra;
pub mod server;
pub mod session;
pub mod token;
