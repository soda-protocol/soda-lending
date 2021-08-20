//! State types

mod manager;
mod last_update;
mod reserve;
mod obligation;
mod asset;
mod oracle;

use std::convert::TryFrom;

pub use manager::*;
pub use last_update::*;
pub use obligation::*;
pub use reserve::*;
pub use asset::*;
pub use oracle::*;

use crate::{
    error::LendingError,
    math::{Decimal, Rate, TryAdd, TryDiv, TryMul},
};
use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use solana_program::{
    clock::Slot,
    msg,
    program_error::ProgramError,
    program_option::COption,
    program_pack::{Pack, Sealed},
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
const COPTION_NONE_TAG: [u8; 4] = [0, 0, 0, 0];

// Helpers
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

fn unpack_coption_struct<T: Pack>(src: &[u8]) -> Result<COption<T>, ProgramError> {
    let (tag, data) = src.split_at(COPTION_LEN);
    match <&[u8; 4]>::try_from(tag).map_err(|_| ProgramError::InvalidAccountData)? {
        &COPTION_NONE_TAG => Ok(COption::None),
        &COPTION_SOME_TAG => Ok(COption::Some(T::unpack_from_slice(data)?)),
        _ => {
            msg!("COption<Pubkey> cannot be unpacked");
            Err(ProgramError::InvalidAccountData)
        }
    }
}

// fn pack_coption_liquidity(
//     src: &COption<LiquidityInfo>,
//     dst: &mut [u8; COPTION_LEN + MARKET_RESERVE_LIQUIDITY_INFO_LEN],
// ) {
//     #[allow(clippy::ptr_offset_with_cast)]
//     let (
//         tag,
//         rate_oracle,
//         available,
//         borrowed,
//         interest,
//         fee,
//         interest_fee_rate,
//         max_borrow_utilization_rate,
//     ) = mut_array_refs![
//         dst,
//         COPTION_LEN,
//         PUBKEY_BYTES,
//         8,
//         8,
//         16,
//         8,
//         8,
//         1
//     ];

//     match src {
//         COption::Some(t) => {
//             *tag = [1, 0, 0, 0];
//             rate_oracle.copy_from_slice(t.rate_oracle.as_ref());
//             *available = t.liquidity.available.to_le_bytes();
//             *borrowed = t.liquidity.borrowed.to_le_bytes();
//             *interest = t.liquidity.interest.to_le_bytes();
//             *fee = t.liquidity.fee.to_le_bytes();
//             *interest_fee_rate = t.config.interest_fee_rate.to_le_bytes();
//             *max_borrow_utilization_rate = t.config.max_borrow_utilization_rate.to_le_bytes();
//         }
//         COption::None => {
//             *tag = [0, 0, 0, 0];
//         }
//     }
// }

// fn unpack_coption_liquidity(
//     src: &[u8; COPTION_LEN + MARKET_RESERVE_LIQUIDITY_INFO_LEN],
// ) -> Result<COption<LiquidityInfo>, ProgramError> {
//     #[allow(clippy::ptr_offset_with_cast)]
//     let (
//         tag,
//         rate_oracle,
//         available,
//         borrowed,
//         interest,
//         fee,
//         interest_fee_rate,
//         max_borrow_utilization_rate,
//     ) = array_refs![
//         src,
//         4,
//         PUBKEY_BYTES,
//         8,
//         8,
//         16,
//         8,
//         8,
//         1
//     ];

//     match *tag {
//         [0, 0, 0, 0] => Ok(COption::None),
//         [1, 0, 0, 0] => Ok(COption::Some(
//             LiquidityInfo {
//                 rate_oracle: Pubkey::new_from_array(*rate_oracle),
//                 liquidity: Liquidity{
//                     available: u64::from_le_bytes(*available),
//                     borrowed: u64::from_le_bytes(*borrowed),
//                     interest: i128::from_le_bytes(*interest),
//                     fee: u64::from_le_bytes(*fee),
//                 },
//                 config: LiquidityConfig {
//                     interest_fee_rate: u64::from_le_bytes(*interest_fee_rate),
//                     max_borrow_utilization_rate: u8::from_le_bytes(*max_borrow_utilization_rate),
//                 },
//             }
//         )),
//         _ => {
//             msg!("COption<Pubkey> cannot be unpacked");
//             Err(ProgramError::InvalidAccountData)
//         }
//     }
// }

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
///
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TokenInfo {
    ///
    pub account: Pubkey,
    ///
    pub price_oracle: Pubkey,
    ///
    pub decimal: u8,
}

impl Sealed for TokenInfo {}
///
pub const TOKEN_INFO_LEN: usize = 65;

impl Pack for TokenInfo {
    const LEN: usize = TOKEN_INFO_LEN;

    fn pack_into_slice(&self, output: &mut [u8]) {
        let output = array_mut_ref![output, 0, TOKEN_INFO_LEN];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            account,
            price_oracle,
            decimal,
        ) = mut_array_refs![
            output,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            1
        ];

        account.copy_from_slice(self.account.as_ref());
        price_oracle.copy_from_slice(self.price_oracle.as_ref());
        *decimal = self.decimal.to_le_bytes();
    }

    fn unpack_from_slice(input: &[u8]) -> Result<Self, ProgramError> {
        let input = array_ref![input, 0, TOKEN_INFO_LEN];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            account,
            price_oracle,
            decimal,
        ) = array_refs![
            input,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            1
        ];

        Ok(Self{
            account: Pubkey::new_from_array(*account),
            price_oracle: Pubkey::new_from_array(*price_oracle),
            decimal: u8::from_le_bytes(*decimal),
        })
    }
}
///
#[derive(Clone, Copy, Debug)]
pub struct Settle {
    ///
    pub total: u64,
    ///
    pub interest: u64,
}
///
#[derive(Clone, Copy, Debug)]
pub struct PriceInfo {
    ///
    pub price_oracle: Pubkey,
    ///
    pub price: Decimal,
}
///
#[inline(always)]
pub fn calculate_decimals(decimal: u8) -> Result<u64, ProgramError> {
    10u64.checked_pow(decimal as u32).ok_or(LendingError::MathOverflow.into())
}
///
#[inline(always)]
pub fn calculate_borrow_interest(base: u64, rate: Rate, elapsed: Slot) -> Result<u64, ProgramError> {
    Decimal::from(base)
        .try_mul(elapsed)?
        .try_mul(rate)?
        .try_ceil_u64()
}
///
#[inline(always)]
pub fn calculate_compound_sum(base: u64, rate: Rate, elapsed: Slot) -> Result<u64, ProgramError> {
    let compounded_interest_rate = rate
        .try_add(Rate::one())?
        .try_pow(elapsed)?;
    
    Decimal::from(base)
        .try_mul(compounded_interest_rate)?
        .try_ceil_u64()
}
///
#[inline(always)]
pub fn calculate_interest_fee(interest: u64, fee_rate: Rate) -> Result<u64, ProgramError> {
    Decimal::from(interest)
        .try_mul(fee_rate)?
        .try_ceil_u64()
}
///
#[inline(always)]
#[allow(clippy::too_many_arguments)]
pub fn calculate_liquidation_fee(
    collateral_price: Decimal,
    collateral_decimals: u64,
    collateral_amount: u64,
    loan_price: Decimal,
    loan_decimals: u64,
    loan_amount: u64,
    fee_rate: Rate,
) -> Result<u64, ProgramError> {
    let equivalent_amount = collateral_price
        .try_mul(collateral_amount)?
        .try_div(collateral_decimals)?
        .try_mul(loan_decimals)?
        .try_div(loan_amount)?
        .try_div(loan_price)?
        .try_round_u64()?;

    let bonus = equivalent_amount
        .checked_sub(loan_amount)
        .ok_or(LendingError::MathOverflow)?;

    Decimal::from(bonus)
        .try_mul(fee_rate)?
        .try_ceil_u64()
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_calculate_borrow_interest() {
        assert_eq!(
            calculate_borrow_interest(100_000_000_000, Rate::from_percent(10), 78840000).unwrap(),
            10_000_000_000,
        );
    }

    #[test]
    fn test_calculate_compound_sum() {
        assert_eq!(
            calculate_compound_sum(100_000_000_000, Rate::from_percent(10), 78840000).unwrap(),
            110_517_091_793,
        );
    }
}