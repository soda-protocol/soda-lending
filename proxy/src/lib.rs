#![deny(missing_docs)]

//! Soda proxy program for the Solana blockchain.

pub mod error;
pub mod entrypoint;
pub mod instruction;
pub mod processor;

// Export current sdk types for downstream users building with a different sdk version
pub use solana_program;

#[cfg(feature = "devnet")]
solana_program::declare_id!("Soda3iMdcSeK5qX9HMTuSLUjiBwA7GQT4YKSYSWJWQf");
#[cfg(not(feature = "devnet"))]
solana_program::declare_id!("Soda2BBinmZtnWPM9UBBaHo7zcgcuxUfSD6ksxdbbGg");