#![allow(missing_docs)]
use std::convert::TryInto;
use super::*;
use crate::{error::LendingError, math::{Decimal, Rate, TryAdd, TryDiv, TryMul}};
use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use solana_program::{
    clock::Slot,
    entrypoint::ProgramResult,
    program_error::ProgramError,
    program_pack::{IsInitialized, Pack, Sealed},
    pubkey::{Pubkey, PUBKEY_BYTES}
};

/// 
const MAX_OBLIGATION_COLLATERALS: usize = 5;
///
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Collateral {
    ///
    pub price_oracle: Pubkey,
    ///
    pub liquidate_limit: u8,
    ///
    pub effective_value_rate: u8,
    ///
    pub amount: u64,
}

impl Sealed for Collateral {}

const COLLATERAL_LEN: usize = 42;

impl Pack for Collateral {
    const LEN: usize = COLLATERAL_LEN;

    fn pack_into_slice(&self, output: &mut [u8]) {
        let output = array_mut_ref![output, 0, COLLATERAL_LEN];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            price_oracle,
            liquidate_limit,
            effective_value_rate,
            amount,
        ) = mut_array_refs![
            output,
            PUBKEY_BYTES,
            1,
            1,
            8
        ];

        price_oracle.copy_from_slice(self.price_oracle.as_ref());
        *liquidate_limit = self.liquidate_limit.to_le_bytes();
        *effective_value_rate = self.effective_value_rate.to_le_bytes();
        *amount = self.amount.to_le_bytes();
    }

    fn unpack_from_slice(input: &[u8]) -> Result<Self, ProgramError> {
        let input = array_ref![input, 0, COLLATERAL_LEN];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            price_oracle,
            liquidate_limit,
            effective_value_rate,
            amount,
        ) = array_refs![
            input,
            PUBKEY_BYTES,
            1,
            1,
            8
        ];

        Ok(Self{
            price_oracle: Pubkey::new_from_array(*price_oracle),
            liquidate_limit: u8::from_le_bytes(*liquidate_limit),
            effective_value_rate: u8::from_le_bytes(*effective_value_rate),
            amount: u64::from_le_bytes(*amount),
        })
    }
}

impl Collateral {
    ///
    pub fn liquidate_value(&self, price: Decimal) -> Result<Decimal, ProgramError> {
        price
            .try_mul(self.amount)?
            .try_mul(Rate::from_percent(self.liquidate_limit))
    }
    ///
    pub fn effective_value(&self, price: Decimal) -> Result<Decimal, ProgramError> {
        price
            .try_mul(self.amount)?
            .try_mul(Rate::from_percent(self.effective_value_rate))
    }
    ///
    pub fn max_value(&self, price: Decimal) -> Result<Decimal, ProgramError> {
        price.try_mul(self.amount)
    }
    ///
    pub fn deposit(&mut self, amount: u64) -> ProgramResult {
        self.amount = self.amount
            .checked_add(amount)
            .ok_or(LendingError::MathOverflow)?;
        Ok(())
    }
    ///
    pub fn redeem(&mut self, amount: u64) -> Result<bool, ProgramError> {
        self.amount = self.amount
            .checked_sub(amount)
            .ok_or(LendingError::ObligationCollateralAmountInsufficient)?;
        Ok(self.amount == 0)
    }
}

/// Lending market obligation state
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UserObligation {
    /// Version of the struct
    pub version: u8,
    ///
    pub reserve: Pubkey,
    /// Owner authority which can borrow liquidity
    pub owner: Pubkey,
    ///
    pub last_update: LastUpdate,
    /// 
    pub collaterals: Vec<Collateral>,
    /// 
    pub borrowed_amount: u64,
    ///
    pub dept_amount: u64,
}

impl Sealed for UserObligation {}
impl IsInitialized for UserObligation {
    fn is_initialized(&self) -> bool {
        self.version != UNINITIALIZED_VERSION
    }
}

const USER_OBLIGATITION_LEN: usize = 301;

impl Pack for UserObligation {
    const LEN: usize = USER_OBLIGATITION_LEN;

    fn pack_into_slice(&self, output: &mut [u8]) {
        let output = array_mut_ref![output, 0, USER_OBLIGATITION_LEN];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            version,
            reserve,
            owner,
            last_update,
            collaterals,
            borrowed_amount,
            dept_amount,
        ) = mut_array_refs![
            output,
            1,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            LAST_UPDATE_LEN,
            1 + MAX_OBLIGATION_COLLATERALS * COLLATERAL_LEN,
            8,
            8
        ];

        *version = self.version.to_le_bytes();
        reserve.copy_from_slice(self.reserve.as_ref());
        owner.copy_from_slice(self.owner.as_ref());
        self.last_update.pack_into_slice(&mut last_update[..]);

        collaterals[0] = self.collaterals.len() as u8;
        collaterals[1..]
            .chunks_mut(COLLATERAL_LEN)
            .zip(self.collaterals.iter())
            .for_each(|(data, collateral)| collateral.pack_into_slice(data));
        *borrowed_amount = self.borrowed_amount.to_le_bytes();
        *dept_amount = self.dept_amount.to_le_bytes();
    }

    fn unpack_from_slice(input: &[u8]) -> Result<Self, ProgramError> {
        let input = array_ref![input, 0, USER_OBLIGATITION_LEN];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            version,
            reserve,
            owner,
            last_update,
            collaterals,
            borrowed_amount,
            dept_amount,
        ) = array_refs![
            input,
            1,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            LAST_UPDATE_LEN,
            1 + MAX_OBLIGATION_COLLATERALS * COLLATERAL_LEN,
            8,
            8
        ];

        let version = u8::from_le_bytes(*version);
        if version > PROGRAM_VERSION {
            msg!("UserObligation version does not match lending program version");
            return Err(ProgramError::InvalidAccountData);
        }

        let collaterals = collaterals[1..1 + COLLATERAL_LEN * collaterals[0] as usize]
            .chunks(COLLATERAL_LEN)
            .map(|data| Collateral::unpack_from_slice(data))
            .collect::<Result<Vec<_>, ProgramError>>()?;

        Ok(Self{
            version,
            reserve: Pubkey::new_from_array(*reserve),
            owner: Pubkey::new_from_array(*owner),
            last_update: LastUpdate::unpack_from_slice(&last_update[..])?,
            collaterals,
            borrowed_amount: u64::from_le_bytes(*borrowed_amount),
            dept_amount: u64::from_le_bytes(*dept_amount),
        })
    }
}

impl UserObligation {
    ///
    pub fn check_valid(&mut self, slot: Slot) -> ProgramResult {
        if self.last_update.is_stale(slot)? {
            Err(LendingError::ObligationStale.into())
        } else {
            self.last_update.mark_stale();
            
            Ok(())
        }
    }
    ///
    pub fn update_borrow_interest(&mut self, slot: Slot, borrow_rate: Rate) -> ProgramResult {
        let elapsed = self.last_update.slots_elapsed(slot)?;
        let interest = calculate_interest(self.borrowed_amount, borrow_rate, elapsed)?;

        self.dept_amount = self.dept_amount
            .checked_add(interest)
            .ok_or(LendingError::MathOverflow)?;

        Ok(())
    }
    ///
    pub fn find_collateral(&self, price_oracle: &Pubkey) -> Result<usize, ProgramError> {
        self.collaterals
            .iter()
            .position(|collateral| &collateral.price_oracle == price_oracle)
            .ok_or(LendingError::ObligationCollateralNotFound.into())
    }
    ///
    pub fn borrow_out(&mut self, amount: u64) -> ProgramResult {
        self.dept_amount = self.dept_amount
            .checked_add(amount)
            .ok_or(LendingError::MathOverflow)?;

        self.borrowed_amount = self.borrowed_amount
            .checked_add(amount)
            .ok_or(LendingError::MathOverflow)?;

        Ok(())
    }
    ///
    pub fn repay(&mut self, amount: u64) -> Result<Settle, ProgramError> {
        let interest = self.dept_amount - self.borrowed_amount;
        self.dept_amount = self.dept_amount
            .checked_sub(amount)
            .ok_or(LendingError::ObligationRepayTooMuch)?;        

        if amount > interest {
            self.borrowed_amount = self.dept_amount;

            Ok(Settle{
                total: amount,
                interest,
            })
        } else {
            Ok(Settle{
                total: amount,
                interest: amount,
            })
        }
    }
    ///
    pub fn deposit(&mut self, index: usize, amount: u64) -> ProgramResult {
        self.collaterals[index].amount = self.collaterals[index].amount
            .checked_add(amount)
            .ok_or(LendingError::MathOverflow)?;

        Ok(())
    }
    ///
    pub fn new_deposit(&mut self, collateral: Collateral) -> ProgramResult {
        if self.collaterals.len() >= MAX_OBLIGATION_COLLATERALS {
            Err(LendingError::ObligationCollateralsLimitExceed.into())
        } else {
            self.collaterals.push(collateral);

            Ok(())
        }
    }
    ///
    pub fn redeem(&mut self, index: usize, amount: u64) -> ProgramResult {
        self.collaterals[index].amount = self.collaterals[index].amount
            .checked_sub(amount)
            .ok_or(LendingError::ObligationCollateralAmountInsufficient)?;

        if self.collaterals[index].amount == 0 {
            self.collaterals.remove(index);
        }

        Ok(())
    }
    ///
    pub fn redeem_all(&mut self) -> ProgramResult {
        if self.dept_amount > 0 {
            Err(LendingError::ObligationCollateralsNotEmpty.into())
        } else {
            self.collaterals.clear();
            Ok(())
        }
    }
    ///
    pub fn check_healthy(&self, prices: &[Price], price: Decimal) -> ProgramResult {
        let collaterals_value = self.collaterals_effective_value(prices)?;
        let loan_value = price.try_mul(self.dept_amount)?;

        if collaterals_value > loan_value {
            Ok(())
        } else {
            Err(LendingError::ObligationNotHealthy.into())
        }
    }
    ///
    pub fn liquidate(
        &mut self,
        index: usize,
        amount: u64,
        close_factor: Rate,
        collateral_prices: &[Price],
        liquidity_price: Decimal,
    ) -> Result<Settle, ProgramError> {
        // validation
        self.check_liquidation(collateral_prices, liquidity_price)?;

        // max liquidation limit check
        let liquidation_ratio: Rate = Decimal::from(amount)
            .try_div(self.collaterals[index].amount)?
            .try_into()?;
        if liquidation_ratio >= close_factor {
            return Err(LendingError::LiquidationTooMuch.into())
        }

        // calculate repay amount
        let collateral_value = self.collaterals[index].effective_value(collateral_prices[index].price)?;
        let collaterals_value = self.collaterals_effective_value(collateral_prices)?;
        let repay_amount = collateral_value
            .try_div(collaterals_value)?
            .try_mul(liquidation_ratio)?
            .try_mul(self.dept_amount)?
            .try_round_u64()?;

        // update collaterals
        self.collaterals[index].amount = self.collaterals[index].amount
            .checked_sub(amount)
            .ok_or(LendingError::ObligationCollateralAmountInsufficient)?;
        
        if self.collaterals[index].amount == 0 {
            self.collaterals.remove(index);
        }
    
        self.repay(repay_amount)
    }
    ///
    pub fn liquidate_arbitrary(
        &mut self,
        index: usize,
        amount: u64,
        collateral_prices: &[Price],
        liquidity_price: Decimal,
        arbitrary_liquidate_rate: Rate,
    ) -> Result<Settle, ProgramError> {
        // validation
        self.check_arbitrary_liquidation(collateral_prices, liquidity_price)?;

        let repay_amount = collateral_prices[index].price
            .try_mul(amount)?
            .try_div(liquidity_price)?
            .try_mul(arbitrary_liquidate_rate)?
            .try_round_u64()?;
        
        // update collaterals
        self.collaterals[index].amount = self.collaterals[index].amount
            .checked_sub(amount)
            .ok_or(LendingError::ObligationCollateralAmountInsufficient)?;

        if self.collaterals[index].amount == 0 {
            self.collaterals.remove(index);
        }

        self.repay(repay_amount)
    }
    ///
    fn collaterals_liquidation_value(&self, prices: &[Price]) -> Result<Decimal, ProgramError> {
        self.collaterals
            .iter()
            .zip(prices.iter())
            .try_fold(Decimal::zero(), |acc, (collateral, price)| {
                if collateral.price_oracle == price.price_oracle {
                    let value = collateral.liquidate_value(price.price)?;
                    acc.try_add(value)
                } else {
                    Err(LendingError::ObligationCollateralsNotMatched.into())
                }
            })
    }
    ///
    fn collaterals_effective_value(&self, prices: &[Price]) -> Result<Decimal, ProgramError> {
        self.collaterals
            .iter()
            .zip(prices.iter())
            .try_fold(Decimal::zero(), |acc, (collateral, price)| {
                if collateral.price_oracle == price.price_oracle {
                    let value = collateral.effective_value(price.price)?;
                    acc.try_add(value)
                } else {
                    Err(LendingError::ObligationCollateralsNotMatched.into())
                }
            })
    }
    ///
    fn collaterals_max_value(&self, prices: &[Price]) -> Result<Decimal, ProgramError> {
        self.collaterals
            .iter()
            .zip(prices.iter())
            .try_fold(Decimal::zero(), |acc, (collateral, price)| {
                if collateral.price_oracle == price.price_oracle {
                    let value = collateral.max_value(price.price)?;
                    acc.try_add(value)
                } else {
                    Err(LendingError::ObligationCollateralsNotMatched.into())
                }
            })
    }
    ///
    fn check_liquidation(&self, prices: &[Price], price: Decimal) -> ProgramResult {
        let collaterals_value = self.collaterals_liquidation_value(prices)?;
        let loan_value = price.try_mul(self.dept_amount)?;

        if collaterals_value <= loan_value {
            Ok(())
        } else {
            Err(LendingError::ObligationLiquidationNotAvailable.into())
        }
    }
    ///
    fn check_arbitrary_liquidation(&self, prices: &[Price], price: Decimal) -> ProgramResult {
        let collaterals_value = self.collaterals_max_value(prices)?;
        let loan_value = price.try_mul(self.dept_amount)?;

        if collaterals_value <= loan_value {
            Ok(())
        } else {
            Err(LendingError::ObligationLiquidationNotAvailable.into())
        }
    }
}

