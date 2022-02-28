#![deny(missing_docs)]

//! A lending program for the Solana blockchain.

pub mod dex;
pub mod error;
pub mod entrypoint;
pub mod instruction;
pub mod invoker;
pub mod lp_oracle;
pub mod oracle;
pub mod math;
pub mod processor;
pub mod state;

// Export current sdk types for downstream users building with a different sdk version
use solana_program;
use solana_program::program_error::ProgramError;
use solana_program::program_pack::{Pack, IsInitialized};
use solana_program::{msg, rent::Rent, account_info::AccountInfo, entrypoint::ProgramResult};
use error::LendingError;

solana_program::declare_id!("Soda111Jv27so2PRBd6ofRptC6dKxosdN5ByFhCcR3V");

/// Data
pub trait Data: Sized {
    ///
    fn to_vec(self) -> Vec<u8>;
}

fn assert_rent_exempt(rent: &Rent, account_info: &AccountInfo) -> ProgramResult {
    if !rent.is_exempt(account_info.lamports(), account_info.data_len()) {
        msg!(&rent.minimum_balance(account_info.data_len()).to_string());
        Err(LendingError::NotRentExempt.into())
    } else {
        Ok(())
    }
}

fn assert_uninitialized<T: Pack + IsInitialized>(account_info: &AccountInfo) -> ProgramResult {
    let account: T = T::unpack_unchecked(&account_info.try_borrow_data()?)?;
    if account.is_initialized() {
        Err(LendingError::AlreadyInitialized.into())
    } else {
        Ok(())
    }
}

#[inline(always)]
fn handle_amount<F: FnOnce()>(amount: u64, notify: F) -> Result<Option<u64>, ProgramError> {
    if amount == 0 {
        notify();
        Err(LendingError::InvalidAmount.into())
    } else if amount == u64::MAX {
        Ok(None)
    } else {
        Ok(Some(amount))
    }
}