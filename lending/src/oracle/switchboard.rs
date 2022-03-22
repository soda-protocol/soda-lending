#![allow(missing_docs)]
use std::convert::TryInto;
use solana_program::{
    msg,
    clock::Clock,
    program_error::ProgramError,
};
use switchboard_program::fast_parse_switchboard_result;

use crate::{error::LendingError, math::{Decimal, TryDiv}};

pub fn get_switchboard_price(data: &[u8], clock: &Clock) -> Result<Decimal, ProgramError> {
    const STALE_AFTER_SLOTS_ELAPSED: u64 = 1000;

    let result = fast_parse_switchboard_result(data).result;

    let slots_eplased = clock.slot
        .checked_sub(result.round_open_slot)
        .ok_or(LendingError::MathOverflow)?;
    if slots_eplased >= STALE_AFTER_SLOTS_ELAPSED {
        msg!("Switchboard oracle price is stale");
        return Err(LendingError::InvalidPriceOracle.into());
    }

    let scale: u32 = result.decimal.scale
        .try_into()
        .map_err(|_| LendingError::MathOverflow)?;
    let decimals = 10u64
        .checked_pow(scale)
        .ok_or(LendingError::MathOverflow)?;
    let price: u128 = result.decimal.mantissa
        .try_into()
        .map_err(|_| LendingError::MathOverflow)?;

    Decimal::from(price).try_div(decimals)   
}