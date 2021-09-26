//! State types
#![allow(missing_docs)]
mod manager;
mod last_update;
mod market_reserve;
mod user_obligation;
mod rate_model;
#[cfg(feature = "unique-credit")]
mod unique_credit;

use std::convert::TryFrom;

pub use manager::*;
pub use last_update::*;
pub use user_obligation::*;
pub use market_reserve::*;
pub use rate_model::*;
#[cfg(feature = "unique-credit")]
pub use unique_credit::*;

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
    fn assert_valid(&self) -> ProgramResult;
}

///
pub trait Operator<P: Param> {
    ///
    fn operate_unchecked(&mut self, param: P) -> ProgramResult;
    ///
    fn operate(&mut self, param: P) -> ProgramResult {
        param.assert_valid()?;
        self.operate_unchecked(param)
    }
}

///
pub trait Upgrader<O>: Sized {
    ///
    fn upgrade(old: O) -> Result<Self, ProgramError>;
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
            tag.copy_from_slice(&COPTION_SOME_TAG);
            t.pack_into_slice(data);
        }
        COption::None => tag.copy_from_slice(&COPTION_NONE_TAG),
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

#[inline(always)]
fn amount_mul_rate(amount: u64, rate: Rate) -> Result<u64, ProgramError> {
    Decimal::from(amount)
        .try_mul(rate)?
        .try_floor_u64()
}

#[inline(always)]
pub fn calculate_amount(amount: u64, max: u64) -> u64 {
    if amount == u64::MAX {
        max
    } else {
        amount
    }
}

#[inline(always)]
fn calculate_amount_and_decimal(amount: u64, max: Decimal) -> Result<(u64, Decimal), ProgramError> {
    if amount == u64::MAX {
        Ok((max.try_ceil_u64()?, max))
    } else {
        Ok((amount, Decimal::from(amount)))
    }
}

///
#[derive(Clone, Debug)]
pub struct RepaySettle {
    ///
    pub amount: u64,
    ///
    pub amount_decimal: Decimal,
}

pub struct ReservesRefVec<'a, 'b>(Vec<&'b(&'a Pubkey, MarketReserve)>);

impl<'a, 'b> ReservesRefVec<'a, 'b> {
    pub fn find_and_remove<E>(&mut self, reserve: &Pubkey, e: E) -> Result<&'b MarketReserve, E> {
        let index = self.0
            .iter()
            .position(|(key, _)| key == &reserve)
            .ok_or(e)?;

        let (_, market_reserve) = self.0.remove(index);

        Ok(market_reserve)
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