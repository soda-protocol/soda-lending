use num_traits::ToPrimitive;
use solana_program::program_error::ProgramError;
use spl_token::state::{Account, Mint};
use spl_token_swap::{curve::calculator::RoundDirection, state::{SwapState, SwapVersion}};

use crate::{error::LendingError, math::{Decimal, TryAdd, TryDiv, TryMul}, state::calculate_decimals};
use super::{PoolBalance, TokenPrice};

fn standard_lp_withdraw_amount(
    token_swap: &SwapVersion,
    pool_balance: &PoolBalance,
    lp_amount: u64,
) -> Result<(u64, u64), ProgramError> {
    let fee = token_swap
        .fees()
        .owner_withdraw_fee(lp_amount as u128)
        .ok_or(LendingError::MathOverflow)?;
    let lp_amount = (lp_amount as u128)
        .checked_sub(fee)
        .ok_or(LendingError::MathOverflow)?;
    let results = &token_swap
        .swap_curve()
        .calculator
        .pool_tokens_to_trading_tokens(
            lp_amount,
            pool_balance.lp_supply as u128,
            pool_balance.balance_a as u128,
            pool_balance.balance_b as u128,
            RoundDirection::Floor,
        )
        .ok_or(LendingError::MathOverflow)?;

    let token_a_amount = results.token_a_amount
        .min(pool_balance.balance_a as u128)
        .to_u64()
        .ok_or(LendingError::MathOverflow)?;
    let token_b_amount = results.token_b_amount
        .min(pool_balance.balance_b as u128)
        .to_u64()
        .ok_or(LendingError::MathOverflow)?;
    
    Ok((token_a_amount, token_b_amount))
}

pub fn estimate_standard_lp_price(
    token_swap: &SwapVersion,
    pool_balance: &PoolBalance,
    price_a: &TokenPrice,
    price_b: &TokenPrice,
    total_lp_amount: u64,
    lp_decimal: u8,
) -> Result<Decimal, ProgramError> {
    let (amount_a, amount_b) = standard_lp_withdraw_amount(token_swap, pool_balance, total_lp_amount)?;
    let value_a = price_a.price
        .try_mul(amount_a)?
        .try_div(calculate_decimals(price_a.decimal)?)?;
    let value_b = price_b.price
        .try_mul(amount_b)?
        .try_div(calculate_decimals(price_a.decimal)?)?;

    value_a
        .try_add(value_b)?
        .try_mul(calculate_decimals(lp_decimal)?)?
        .try_div(total_lp_amount)
}