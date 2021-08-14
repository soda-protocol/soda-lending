#![deny(missing_docs)]

//! A lending program for the Solana blockchain.

pub mod entrypoint;
pub mod error;
pub mod instruction;
pub mod math;
pub mod processor;
pub mod state;
pub mod pyth;

// Export current sdk types for downstream users building with a different sdk version
pub use solana_program;

solana_program::declare_id!("SodaLending66666666666666666666666666666666");
