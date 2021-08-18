#![allow(missing_docs)]
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
    pub decimal: u8,
    ///
    pub borrow_value_ratio: u8,
    ///
    pub liquidation_value_ratio: u8,
    ///
    pub amount: u64,
}

impl Sealed for Collateral {}

const COLLATERAL_LEN: usize = 43;

impl Pack for Collateral {
    const LEN: usize = COLLATERAL_LEN;

    fn pack_into_slice(&self, output: &mut [u8]) {
        let output = array_mut_ref![output, 0, COLLATERAL_LEN];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            price_oracle,
            decimal,
            borrow_value_ratio,
            liquidation_value_ratio,
            amount,
        ) = mut_array_refs![
            output,
            PUBKEY_BYTES,
            1,
            1,
            1,
            8
        ];

        price_oracle.copy_from_slice(self.price_oracle.as_ref());
        *decimal = self.decimal.to_le_bytes();
        *borrow_value_ratio = self.borrow_value_ratio.to_le_bytes();
        *liquidation_value_ratio = self.liquidation_value_ratio.to_le_bytes();
        *amount = self.amount.to_le_bytes();
    }

    fn unpack_from_slice(input: &[u8]) -> Result<Self, ProgramError> {
        let input = array_ref![input, 0, COLLATERAL_LEN];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            price_oracle,
            decimal,
            borrow_value_ratio,
            liquidation_value_ratio,
            amount,
        ) = array_refs![
            input,
            PUBKEY_BYTES,
            1,
            1,
            1,
            8
        ];

        Ok(Self{
            price_oracle: Pubkey::new_from_array(*price_oracle),
            decimal: u8::from_le_bytes(*decimal),
            borrow_value_ratio: u8::from_le_bytes(*borrow_value_ratio),
            liquidation_value_ratio: u8::from_le_bytes(*liquidation_value_ratio),
            amount: u64::from_le_bytes(*amount),
        })
    }
}

impl Collateral {
    ///
    pub fn borrow_effective_value(&self, price: Decimal) -> Result<Decimal, ProgramError> {
        let decimals = 10u64
            .checked_pow(self.decimal as u32)
            .ok_or(LendingError::MathOverflow)?;

        price
            .try_mul(self.amount)?
            .try_div(decimals)?
            .try_mul(Rate::from_percent(self.borrow_value_ratio))
    }
    ///
    pub fn liquidation_effective_value(&self, price: Decimal) -> Result<Decimal, ProgramError> {
        let decimals = 10u64
            .checked_pow(self.decimal as u32)
            .ok_or(LendingError::MathOverflow)?;

        price
            .try_mul(self.amount)?
            .try_div(decimals)?
            .try_mul(Rate::from_percent(self.liquidation_value_ratio))
    }
    ///
    pub fn liquidation_max_value(&self, price: Decimal) -> Result<Decimal, ProgramError> {
        let decimals = 10u64
            .checked_pow(self.decimal as u32)
            .ok_or(LendingError::MathOverflow)?;

        price
            .try_mul(self.amount)?
            .try_div(decimals)
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
    ///
    pub owner: Pubkey,
    ///
    pub last_update: LastUpdate,
    /// 
    pub collaterals: Vec<Collateral>,
    ///
    pub collaterals_value: (Decimal, Decimal, Decimal),
    ///
    pub loan_market_price: Decimal,
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

const USER_OBLIGATITION_LEN: usize = 370;

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
            min,
            mid,
            max,
            loan_market_price,
            borrowed_amount,
            dept_amount,
        ) = mut_array_refs![
            output,
            1,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            LAST_UPDATE_LEN,
            1 + MAX_OBLIGATION_COLLATERALS * COLLATERAL_LEN,
            16,
            16,
            16,
            16,
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
        
        pack_decimal(self.collaterals_value.0, min);
        pack_decimal(self.collaterals_value.1, mid);
        pack_decimal(self.collaterals_value.2, max);
        pack_decimal(self.loan_market_price, loan_market_price);
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
            min,
            mid,
            max,
            loan_market_price,
            borrowed_amount,
            dept_amount,
        ) = array_refs![
            input,
            1,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            LAST_UPDATE_LEN,
            1 + MAX_OBLIGATION_COLLATERALS * COLLATERAL_LEN,
            16,
            16,
            16,
            16,
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
            collaterals_value: (unpack_decimal(min), unpack_decimal(mid), unpack_decimal(max)),
            loan_market_price: unpack_decimal(loan_market_price),
            borrowed_amount: u64::from_le_bytes(*borrowed_amount),
            dept_amount: u64::from_le_bytes(*dept_amount),
        })
    }
}

impl UserObligation {
    ///
    pub fn update_borrow_interest(&mut self, slot: Slot, borrow_rate: Rate) -> ProgramResult {
        let elapsed = self.last_update.slots_elapsed(slot)?;
        let interest = calculate_borrow_interest(self.borrowed_amount, borrow_rate, elapsed)?;

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
    pub fn update_temp_data(&mut self, prices: &[PriceInfo], loan_price: Decimal) -> ProgramResult {
        self.loan_market_price = loan_price;
        self.collaterals_value = self.collaterals
            .iter()
            .zip(prices.iter())
            .try_fold((Decimal::zero(), Decimal::zero(), Decimal::zero()),
                |(acc_1, acc_2, acc_max), (collateral, price)| ->
                Result<(Decimal, Decimal, Decimal), ProgramError> {
                if collateral.price_oracle == price.price_oracle {
                    let borrow_effective_value = collateral
                        .borrow_effective_value(price.price)?
                        .try_add(acc_1)?;
                    let liquidation_effective_value = collateral
                        .liquidation_effective_value(price.price)?
                        .try_add(acc_2)?;
                    let liquidation_max_value = collateral
                        .liquidation_max_value(price.price)?
                        .try_add(acc_max)?;

                    Ok((borrow_effective_value, liquidation_effective_value, liquidation_max_value))
                } else {
                    Err(LendingError::ObligationCollateralsNotMatched.into())
                }
            })?;

        Ok(())
    }
    ///
    pub fn validate_borrow(&self, loan_decimals: u64) -> ProgramResult {
        let loan_value = self.loan_market_price
            .try_mul(self.dept_amount)?
            .try_div(loan_decimals)?;

        if self.collaterals_value.0 > loan_value {
            Ok(())
        } else {
            Err(LendingError::ObligationNotHealthy.into())
        }
    }
    ///
    pub fn validate_liquidation(&self, loan_decimals: u64) -> ProgramResult {
        let loan_value = self.loan_market_price
            .try_mul(self.dept_amount)?
            .try_div(loan_decimals)?;

        if loan_value >= self.collaterals_value.1 && loan_value < self.collaterals_value.2 {
            Ok(())
        } else {
            Err(LendingError::ObligationLiquidationNotAvailable.into())
        }
    }
    ///
    pub fn validate_liquidation_2(&self, loan_decimals: u64) -> ProgramResult {
        let loan_value = self.loan_market_price
            .try_mul(self.dept_amount)?
            .try_div(loan_decimals)?;

        if loan_value >= self.collaterals_value.2 {
            Ok(())
        } else {
            Err(LendingError::ObligationLiquidationNotAvailable.into())
        }
    }
    ///
    pub fn liquidate(
        &mut self,
        index: usize,
        amount: u64,
        close_factor: Rate,
        collateral_price: Decimal,
    ) -> Result<Settle, ProgramError> {
        // max liquidation limit check
        let liquidation_ratio = Rate::from_scaled_val(amount).try_div(self.collaterals[index].amount)?;
        if liquidation_ratio >= close_factor {
            return Err(LendingError::LiquidationTooMuch.into())
        }

        // calculate repay amount
        let repay_amount = self.collaterals[index]
            .liquidation_effective_value(collateral_price)?
            .try_div(self.collaterals_value.0)?
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
    pub fn liquidate_2(
        &mut self,
        index: usize,
        amount: u64,
        repay_ratio: Rate,
        loan_decimals: u64,
        collateral_price: Decimal,
    ) -> Result<Settle, ProgramError> {
        let decimals = 10u64
            .checked_pow(self.collaterals[index].decimal as u32)
            .ok_or(LendingError::MathOverflow)?;

        let repay_amount = collateral_price
            .try_mul(amount)?
            .try_div(decimals)?
            .try_mul(loan_decimals)?
            .try_div(self.loan_market_price)?
            .try_mul(repay_ratio)?
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
}

