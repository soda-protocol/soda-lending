//! State types
#![allow(missing_docs)]
mod manager;
mod last_update;
mod reserve;
mod obligation;
mod rate_oracle;

use std::convert::TryFrom;

pub use manager::*;
pub use last_update::*;
pub use obligation::*;
pub use reserve::*;
pub use rate_oracle::*;

use crate::{
    error::LendingError,
    math::{Decimal, Rate, TryAdd, TryDiv, TryMul, TrySub},
};
use arrayref::{array_refs, mut_array_refs};
use solana_program::{
    msg,
    entrypoint::ProgramResult,
    program_error::ProgramError,
    program_option::COption,
    program_pack::Pack,
    pubkey::{Pubkey, PUBKEY_BYTES},
};

/// Current version of the program and all new accounts created
pub const PROGRAM_VERSION: u8 = 1;

/// Accounts are created with data zeroed out, so uninitialized state instances
/// will have the version set to 0.
pub const UNINITIALIZED_VERSION: u8 = 0;

///
pub const COPTION_LEN: usize = 4;
///
const COPTION_SOME_TAG: [u8; 4] = [1, 0, 0, 0];
///
const COPTION_NONE_TAG: [u8; 4] = [0, 0, 0, 0];

///
pub trait Param: Sized {
    ///
    fn is_valid(&self) -> ProgramResult;
}

///
pub trait Operator<P: Param + Copy> {
    ///
    fn operate_unchecked(&mut self, param: P) -> ProgramResult;
    ///
    fn operate(&mut self, param: P) -> ProgramResult {
        param.is_valid()?;
        self.operate_unchecked(param)
    }
}

///
pub trait Migrator<O>: Sized {
    ///
    fn migrate(old: O) -> Result<Self, ProgramError>;
}

// Helpers
fn pack_coption_pubkey(src: &COption<Pubkey>, dst: &mut [u8; COPTION_LEN + PUBKEY_BYTES]) {
    #[allow(clippy::ptr_offset_with_cast)]
    let (tag, body) = mut_array_refs![dst, COPTION_LEN, PUBKEY_BYTES];
    match src {
        COption::Some(key) => {
            *tag = COPTION_SOME_TAG;
            body.copy_from_slice(key.as_ref());
        }
        COption::None => {
            *tag = COPTION_NONE_TAG;
        }
    }
}

fn unpack_coption_pubkey(src: &[u8; COPTION_LEN + PUBKEY_BYTES]) -> Result<COption<Pubkey>, ProgramError> {
    #[allow(clippy::ptr_offset_with_cast)]
    let (tag, body) = array_refs![src, COPTION_LEN, PUBKEY_BYTES];
    match *tag {
        COPTION_NONE_TAG => Ok(COption::None),
        COPTION_SOME_TAG => Ok(COption::Some(Pubkey::new_from_array(*body))),
        _ => Err(LendingError::COptionUnpackError.into()),
    }
}
#[allow(dead_code)]
fn pack_coption_struct<T: Pack>(src: &COption<T>, dst: &mut [u8]) {
    let (tag, data) = dst.split_at_mut(COPTION_LEN);
    match src {
        COption::Some(t) => {
            tag.copy_from_slice(&COPTION_SOME_TAG[..]);
            t.pack_into_slice(data);
        }
        COption::None => tag.copy_from_slice(&COPTION_NONE_TAG[..]),
    }
}
#[allow(dead_code)]
fn unpack_coption_struct<T: Pack>(src: &[u8]) -> Result<COption<T>, ProgramError> {
    let (tag, data) = src.split_at(COPTION_LEN);
    match <&[u8; 4]>::try_from(tag).map_err(|_| ProgramError::InvalidAccountData)? {
        &COPTION_NONE_TAG => Ok(COption::None),
        &COPTION_SOME_TAG => Ok(COption::Some(T::unpack_from_slice(data)?)),
        _ => Err(LendingError::COptionUnpackError.into()),
    }
}

fn pack_decimal(decimal: Decimal, dst: &mut [u8; 16]) {
    *dst = decimal
        .to_scaled_val()
        .expect("Decimal cannot be packed")
        .to_le_bytes();
}

fn unpack_decimal(src: &[u8; 16]) -> Decimal {
    Decimal::from_scaled_val(u128::from_le_bytes(*src))
}

fn pack_bool(boolean: bool, dst: &mut [u8; 1]) {
    *dst = (boolean as u8).to_le_bytes()
}

fn unpack_bool(src: &[u8; 1]) -> Result<bool, ProgramError> {
    match u8::from_le_bytes(*src) {
        0 => Ok(false),
        1 => Ok(true),
        _ => {
            msg!("Boolean cannot be unpacked");
            Err(ProgramError::InvalidAccountData)
        }
    }
}

#[inline(always)]
fn calculate_decimals(decimal: u8) -> Result<u64, ProgramError> {
    10u64
        .checked_pow(decimal as u32)
        .ok_or(LendingError::MathOverflow.into())
}

///
#[derive(Clone, Copy, Debug)]
pub struct RepaySettle {
    ///
    pub amount: u64,
    ///
    pub amount_decimal: Decimal,
}

///
#[derive(Clone, Copy, Debug)]
pub struct LiquidationSettle {
    ///
    pub repay: u64,
    ///
    pub repay_decimal: Decimal,
    ///
    pub repay_with_fee: u64,
    ///
    pub fee: u64,
}

impl LiquidationSettle {
    ///
    pub fn new(
        collateral_value: Decimal,
        loan_price: Decimal,
        loan_amount: Decimal,
        loan_decimals: u64,
        fee_rate: Rate,
    ) -> Result<Self, ProgramError> {
        let equivalent_amount = collateral_value
            .try_mul(loan_decimals)?
            .try_div(loan_price)?;

        let repay = loan_amount.try_ceil_u64()?;
        let (repay_with_fee, fee) = if equivalent_amount > loan_amount {
            let fee = equivalent_amount
                .try_sub(loan_amount)?
                .try_mul(fee_rate)?
                .try_ceil_u64()?;

            let repay_with_fee = repay
                .checked_add(fee)
                .ok_or(LendingError::MathOverflow)?;

            (repay_with_fee, fee)
        } else {
            (repay, 0)
        };

        if repay_with_fee == 0 {
            return Err(LendingError::LiquidationRepayTooSmall.into());
        }

        Ok(Self {
            repay,
            repay_decimal: loan_amount,
            repay_with_fee,
            fee,
        })
    }
}

// #[cfg(test)]
// mod test {
//     use super::*;

//     #[test]
//     fn test_calculate_borrow_interest() {
//         assert_eq!(
//             calculate_borrow_interest(100_000_000_000, Rate::from_percent(10), 78840000).unwrap(),
//             10_000_000_000,
//         );
//     }

//     #[test]
//     fn test_calculate_compound_sum() {
//         assert_eq!(
//             calculate_compound_sum(100_000_000_000, Rate::from_percent(10), 78840000).unwrap(),
//             110_517_091_793,
//         );
//     }
// }