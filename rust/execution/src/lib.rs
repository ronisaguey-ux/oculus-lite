//! Self-contained execution primitives for `oculus-lite`.
//!
//! This crate is a clean-room, dependency-free implementation of the parts of an
//! execution system that are worth showing in public: price/time priority
//! matching, strict order lifecycle tracking, venue emulation, risk limits,
//! trailing stops, reconciliation and a client abstraction. There is no
//! strategy logic and no venue connectivity here.
//!
//! Everything is deterministic: given the same orders and the same bars, the
//! same fills and the same event log are produced on every run.

pub mod client;
pub mod matching_core;
pub mod models;
pub mod order_emulator;
pub mod order_manager;
pub mod protection;
pub mod reconciliation;
pub mod trailing;
