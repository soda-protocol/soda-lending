#![allow(missing_docs)]
///
use std::convert::TryInto;
use crate::error::LendingError;
use solana_program::{
    clock::{DEFAULT_TICKS_PER_SECOND, DEFAULT_TICKS_PER_SLOT, SECONDS_PER_DAY},
    entrypoint::ProgramResult, 
    program_error::ProgramError,
};
use super::*;

const SLOTS_PER_YEAR: u64 = DEFAULT_TICKS_PER_SECOND * SECONDS_PER_DAY * 365 / DEFAULT_TICKS_PER_SLOT;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RateModel {
    pub offset: u64,
    pub optimal: u64,
    pub kink: u8,
    pub max: u128,
}

impl Param for RateModel {
    fn assert_valid(&self) -> ProgramResult {
        if self.optimal > self.offset &&
            self.max > self.optimal as u128 &&
            self.kink > 0 && self.kink < 100 {
            Ok(())
        } else {
            Err(LendingError::InvalidRateModel.into())
        }
    }
}

impl RateModel {
    ///
    pub fn calculate_borrow_rate(&self, utilization: Rate) -> Result<Rate, ProgramError> {
        let kink_utilization = Rate::from_percent(self.kink);
        let offset = Rate::from_scaled_val(self.offset);
        let optimal = Rate::from_scaled_val(self.optimal);
        let max = Rate::from_raw_val(self.max);

        let borrow_rate_per_year: Rate = if utilization <= kink_utilization {
            Decimal::from(utilization)
                .try_mul(optimal.try_sub(offset)?)?
                .try_div(kink_utilization)?
                .try_add(Decimal::from(offset))?
                .try_into()?
        } else {
            Decimal::from(utilization.try_sub(kink_utilization)?)
                .try_mul(max.try_sub(optimal)?)?
                .try_div(Rate::one().try_sub(kink_utilization)?)?
                .try_add(Decimal::from(optimal))?
                .try_into()?
        };

        borrow_rate_per_year.try_div(SLOTS_PER_YEAR)
    }
}
