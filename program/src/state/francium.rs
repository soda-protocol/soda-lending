
use super::*;
use std::{cmp::Ordering, convert::TryInto};
use solana_program::{entrypoint::ProgramResult, pubkey::Pubkey};

///
pub struct FranciumLend {
    ///
    pub manager: Pubkey,
    ///
    pub reserve: Pubkey,
    ///
    pub borrower: Pubkey,
    ///
    pub credit_mint: Pubkey,
    ///
    pub credit_account: Pubkey,
    ///
    pub credit_amount: u64,
    ///
    pub acc_borrow_rate_wads: Decimal,
    /// Amount of liquidity borrowed plus interest
    pub borrowed_amount_wads: Decimal,
}

impl FranciumLend {
    pub fn accrue_interest(&mut self, reserve: &MarketReserve) -> ProgramResult {
        match reserve.liquidity_info.acc_borrow_rate_wads.cmp(&self.acc_borrow_rate_wads) {
            Ordering::Less => Err(LendingError::NegativeInterestRate.into()),
            Ordering::Equal => Ok(()),
            Ordering::Greater => {
                let compounded_interest_rate: Rate = reserve.liquidity_info.acc_borrow_rate_wads
                    .try_div(self.acc_borrow_rate_wads)?
                    .try_into()?;

                self.borrowed_amount_wads = self.borrowed_amount_wads.try_mul(compounded_interest_rate)?;
                self.acc_borrow_rate_wads = reserve.liquidity_info.acc_borrow_rate_wads;

                Ok(())
            }
        }
    }

    pub fn borrow_in(&mut self, amount: u64, reserve: &MarketReserve) -> Result<u64, ProgramError> {
        let amount = calculate_amount(amount, reserve.liquidity_info.available);
        self.borrowed_amount_wads = self.borrowed_amount_wads.try_add(Decimal::from(amount))?;

        Ok(amount)
    }

    pub fn repay(&mut self, amount: u64) -> Result<u64, ProgramError> {
        let (amount, amount_decimal) = calculate_amount_and_decimal(amount, self.borrowed_amount_wads)?;
        self.borrowed_amount_wads = self.borrowed_amount_wads
            .try_sub(amount_decimal)
            .map_err(|_| LendingError::RepayTooMuch)?;

        Ok(amount)
    }

    pub fn deposit_credit(&mut self, amount: u64) -> ProgramResult {
        self.credit_amount = self.credit_amount
            .checked_add(amount)
            .ok_or(LendingError::MathOverflow)?;

        Ok(())
    }

    pub fn redeem_credit(&mut self, amount: u64) -> ProgramResult {
        self.credit_amount = self.credit_amount
            .checked_sub(amount)
            .ok_or(LendingError::MathOverflow)?;

        Ok(())
    }
}

