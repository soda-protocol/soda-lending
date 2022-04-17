#![allow(missing_docs)]
mod orca;
mod raydium;

pub use orca::*;
pub use raydium::*;

use solana_program::{program_error::ProgramError, entrypoint::ProgramResult};

pub type DexType = u8;

pub const ORCA: DexType = 0;
pub const ORCA_TWICE: DexType = 1;
pub const RAYDIUM: DexType = 2;

#[macro_export]
macro_rules! check_pubkey {
    ($ini:expr, $pool:expr, $($x:expr), *) => {
        {
            let mut res = $ini;
            $(res = res || ($pool == $x);)*
            res
        }
    };
}

pub trait Swapper<'a, 'b> {
    fn is_supported(&self) -> bool;
    fn get_user_source_token_balance(&self) -> Result<u64, ProgramError>;
    fn get_user_dest_token_balance(&self) -> Result<u64, ProgramError>;
    fn get_pool_source_token_balance(&self) -> Result<u64, ProgramError>;
    fn get_pool_dest_token_balance(&self) -> Result<u64, ProgramError>;
    fn swap_base_in(&self, amount_in: u64, minimum_amount_out: u64) -> ProgramResult;
    fn swap_base_out(&self, max_amount_in: u64, amount_out: u64) -> ProgramResult;
}
