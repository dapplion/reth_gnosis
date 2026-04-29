//! EF-test harness for `reth_gnosis`.
//!
//! Files copied from upstream reth's `testing/ef-tests/src` with a Gnosis hook in
//! `cases/blockchain_test.rs` that injects `eip1559collector` / `blockRewardsContract` /
//! `deposit_contract` from the Chiado genesis into the per-test chain spec.
//!
//! Gated behind the `testing` feature.

#![cfg(feature = "testing")]
#![allow(dead_code, missing_docs)]

mod assert;
mod case;
mod cases;
mod models;
mod result;
mod suite;

pub use case::Case;
pub use result::Error;
pub use suite::Suite;

mod tests;
