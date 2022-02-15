#![allow(missing_docs)]
use std::convert::TryInto;
use solana_program::{
    msg,
    clock::Clock,
    program_error::ProgramError,
};
use switchboard_program_v1::fast_parse_switchboard_result as fast_parse_switchboard_result_v1;
use switchboard_program_v2::fast_parse_switchboard_result as fast_parse_switchboard_result_v2;

use crate::{error::LendingError, math::{Decimal, TryDiv}};

macro_rules! get_switchboard_price {
    ($func:ident, $parse:ident) => {
        pub fn $func(data: &[u8], clock: &Clock) -> Result<Decimal, ProgramError> {
            #[cfg(not(feature = "devnet"))]
            const STALE_AFTER_SLOTS_ELAPSED: u64 = 10;
            #[cfg(feature = "devnet")]
            const STALE_AFTER_SLOTS_ELAPSED: u64 = 1000;
    
            let result = $parse(data).result;
    
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
    };
}

get_switchboard_price!(get_switchboard_price_v1, fast_parse_switchboard_result_v1);
get_switchboard_price!(get_switchboard_price_v2, fast_parse_switchboard_result_v2);
