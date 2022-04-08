#![allow(missing_docs)]
use std::convert::TryInto;

use num_traits::ToPrimitive;
use pyth_sdk_solana::load_price_feed_from_account_info;
use solana_program::{msg, clock::Clock, program_error::ProgramError, account_info::AccountInfo};

use crate::{math::{Decimal, TryMul, TryDiv}, error::LendingError};

pub fn get_pyth_price(account_info: &AccountInfo, clock: &Clock) -> Result<Decimal, ProgramError> {
    #[cfg(not(feature = "devnet"))]
    const STALE_AFTER_SECS_ELAPSED: i64 = 30;
    #[cfg(feature = "devnet")]
    const STALE_AFTER_SECS_ELAPSED: i64 = 1000;

    let price_feed = load_price_feed_from_account_info(account_info)?;
    let price = if let Some(price) = price_feed.get_current_price() {
        price
    } else {
        let (price, timestamp) = price_feed.get_prev_price_unchecked();
        let time_elapsed = clock.unix_timestamp
            .checked_sub(timestamp)
            .ok_or(LendingError::InvalidPriceOracle)?;

        if time_elapsed >= STALE_AFTER_SECS_ELAPSED {
            msg!("Pyth oracle price is stale");
            return Err(LendingError::InvalidPriceOracle.into());
        }

        price
    };

    if price.expo >= 0 {
        let exponent = price.expo
            .try_into()
            .map_err(|_| LendingError::MathOverflow)?;
        let zeros = 10u64
            .checked_pow(exponent)
            .ok_or(LendingError::MathOverflow)?;
        Decimal::from(price.price.to_u64().ok_or(LendingError::MathOverflow)?).try_mul(zeros)
    } else {
        let exponent = price.expo
            .checked_abs()
            .ok_or(LendingError::MathOverflow)?
            .try_into()
            .map_err(|_| LendingError::MathOverflow)?;
        let decimals = 10u64
            .checked_pow(exponent)
            .ok_or(LendingError::MathOverflow)?;
        Decimal::from(price.price.to_u64().ok_or(LendingError::MathOverflow)?).try_div(decimals)
    }
}