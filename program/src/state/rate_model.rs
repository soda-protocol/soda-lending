#![allow(missing_docs)]
///
use super::*;
use crate::{error::LendingError, math::WAD};
use solana_program::{
    clock::{DEFAULT_TICKS_PER_SECOND, DEFAULT_TICKS_PER_SLOT, SECONDS_PER_DAY},
    entrypoint::ProgramResult, 
    program_error::ProgramError,
};

const SLOTS_PER_YEAR: u64 = DEFAULT_TICKS_PER_SECOND * SECONDS_PER_DAY * 365 / DEFAULT_TICKS_PER_SLOT;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RateModel {
    pub a: u64,
    pub c: u64,
    pub l_u: u8,
    pub k_u: u128,
}

impl Param for RateModel {
    fn assert_valid(&self) -> ProgramResult {
        if self.c < WAD && self.l_u < 100 {
            Ok(())
        } else {
            Err(LendingError::InvalidRateModel.into())
        }
    }
}

impl RateModel {
    pub fn calculate_borrow_rate(&self, utilization: Rate) -> Result<Rate, ProgramError> {
        let utilization_threshold = Rate::from_percent(self.l_u);
        let a = Rate::from_scaled_val(self.a);
        let c = Rate::from_scaled_val(self.c);
        let k_u = Rate::from_raw_val(self.k_u);

        let borrow_rate_per_year = if utilization <= utilization_threshold {
            utilization
                .try_mul(a)?
                .try_add(c)?
        } else {
            let z1 = utilization_threshold.try_mul(a)?;
            let z2 = utilization
                .try_sub(utilization_threshold)?
                .try_mul(a)?
                .try_mul(k_u)?;
            
            z1.try_add(z2)?.try_add(c)?
        };

        borrow_rate_per_year.try_div(SLOTS_PER_YEAR)
    }
}
