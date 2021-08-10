use super::*;
use crate::{
    error::LendingError,
    math::{Decimal, Rate, TryAdd, TryDiv, TryMul},
};
use solana_program::{
    clock::Slot,
    program_pack::{Pack, Sealed},
    entrypoint::ProgramResult,
    program_error::ProgramError
};
use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};

///
#[derive(Clone, Debug, PartialEq)]
pub struct CollateralConfig {
    ///
    pub liquidate_fee_rate: u64,
    ///
    pub liquidate_limit_rate: u64,
}

///
#[derive(Clone, Debug, PartialEq)]
pub struct LiquidityConfig {
    ///
    pub min_borrow_utilization_rate: u64,
    ///
    pub max_borrow_utilization_rate: u64,
    ///
    pub interest_fee_rate: u64, 
}
///
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TokenInfo {
    ///
    pub account: Pubkey,
    ///
    pub price_oracle: Pubkey,
}

impl Sealed for TokenInfo {}
///
pub const TOKEN_INFO_LEN: usize = 64;

impl Pack for TokenInfo {
    const LEN: usize = TOKEN_INFO_LEN;

    fn pack_into_slice(&self, output: &mut [u8]) {
        let output = array_mut_ref![output, 0, TOKEN_INFO_LEN];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            account,
            price_oracle,
        ) = mut_array_refs![
            output,
            PUBKEY_BYTES,
            PUBKEY_BYTES
        ];

        account.copy_from_slice(self.account.as_ref());
        price_oracle.copy_from_slice(self.price_oracle.as_ref());
    }

    fn unpack_from_slice(input: &[u8]) -> Result<Self, ProgramError> {
        let input = array_ref![input, 0, TOKEN_INFO_LEN];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            account,
            price_oracle,
        ) = array_refs![
            input,
            PUBKEY_BYTES,
            PUBKEY_BYTES
        ];

        Ok(Self{
            account: Pubkey::new_from_array(*account),
            price_oracle: Pubkey::new_from_array(*price_oracle),
        })
    }
}
///
#[derive(Clone, Copy, Debug)]
pub struct Fund {
    ///
    pub principal: u64,
    ///
    pub interest: u64,
}
///
#[derive(Clone, Copy, Debug)]
pub struct Pair {
    ///
    pub amount: u64,
    ///
    pub price: Decimal,
}
///
#[derive(Clone, Copy, Debug)]
pub struct Settle {
    ///
    pub price_oracle: Pubkey,
    ///
    pub price: Decimal,
}
///
#[inline(always)]
pub fn price_conversion(price: u64, decimal: u8) -> Result<Decimal, ProgramError> {
    let decimals = 10u64
        .checked_pow(decimal as u32)
        .ok_or(LendingError::MathOverflow)?;

    Decimal::from(price).try_div(decimals)
}
///
#[inline(always)]
pub fn calculate_interest(base: u64, rate: Rate, elapsed: Slot) -> Result<u64, ProgramError> {
    Decimal::from(base)
        .try_mul(elapsed)?
        .try_mul(rate)?
        .try_div(SLOTS_PER_YEAR)?
        .try_ceil_u64()
}
///
#[inline(always)]
pub fn calculate_compound_interest(base: u64, rate: Rate, elapsed: Slot) -> Result<u64, ProgramError> {
    let compounded_interest_rate = rate
        .try_div(SLOTS_PER_YEAR)?
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
pub fn validate_liquidation_limit(loan_value: Decimal, collaterals_value: Decimal) -> ProgramResult {
    if collaterals_value > loan_value {
        Ok(())
    } else {
        Err(LendingError::ObligationCollateralsLiquidatitonLimit.into())
    }
}
///
#[inline(always)]
pub fn calculate_liquidation_fee(
    collaterals_value: Decimal,
    loan: Pair,
    fee_rate: Rate,
) -> Result<u64, ProgramError> {
    let equivalent_amount = collaterals_value
        .try_div(loan.price)?
        .try_round_u64()?;

    if equivalent_amount > loan.amount {
        Decimal::from(equivalent_amount - loan.amount)
            .try_mul(fee_rate)?
            .try_ceil_u64()
    } else {
        Ok(0)
    }
}
