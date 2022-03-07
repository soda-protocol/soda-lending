#![allow(missing_docs)]
mod orca;

pub use orca::*;

use solana_program::{program_error::ProgramError, entrypoint::ProgramResult};

pub trait Swapper<'a, 'b> {
    fn is_supported(&self) -> bool;
    fn get_user_source_token_balance(&self) -> Result<u64, ProgramError>;
    fn get_user_dest_token_balance(&self) -> Result<u64, ProgramError>;
    fn get_pool_source_token_balance(&self) -> Result<u64, ProgramError>;
    fn get_pool_dest_token_balance(&self) -> Result<u64, ProgramError>;
    fn swap(&self, amount_in: u64, minimum_amount_out: u64) -> ProgramResult;
}

pub type DexType = u8;

pub const ORCA_DEX: DexType = 0;