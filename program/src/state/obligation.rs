use super::*;
use crate::{
    error::LendingError,
    math::{Decimal, Rate, TryAdd, TryDiv, TryMul},
};
use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use solana_program::{
    clock::Slot,
    entrypoint::ProgramResult,
    program_error::ProgramError,
    program_pack::{IsInitialized, Pack, Sealed},
    pubkey::{Pubkey, PUBKEY_BYTES}
};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Collateral {
    pub price_oracle: Pubkey,
    pub liquidate_limit_rate: u64,
    pub amount: u64,
}

impl Sealed for Collateral {}

const COLLATERAL_LEN: usize = 48;

impl Pack for Collateral {
    const LEN: usize = COLLATERAL_LEN;

    fn pack_into_slice(&self, output: &mut [u8]) {
        let output = array_mut_ref![output, 0, COLLATERAL_LEN];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            price_oracle,
            liquidate_limit_rate,
            amount,
        ) = mut_array_refs![
            output,
            PUBKEY_BYTES,
            8,
            8
        ];

        price_oracle.copy_from_slice(self.price_oracle.as_ref());
        *liquidate_limit_rate = self.liquidate_limit_rate.to_le_bytes();
        *amount = self.amount.to_le_bytes();
    }

    fn unpack_from_slice(input: &[u8]) -> Result<Self, ProgramError> {
        let input = array_ref![input, 0, COLLATERAL_LEN];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            price_oracle,
            liquidate_limit_rate,
            amount,
        ) = array_refs![
            input,
            PUBKEY_BYTES,
            8,
            8
        ];

        Ok(Self{
            price_oracle: Pubkey::new_from_array(*price_oracle),
            liquidate_limit_rate: u64::from_le_bytes(*liquidate_limit_rate),
            amount: u64::from_le_bytes(*amount),
        })
    }
}

impl Collateral {
    pub fn value(&self, price: &Decimal) -> Result<Decimal, ProgramError> {
        price
            .try_mul(self.amount)?
            .try_mul(Rate::from_scaled_val(self.liquidate_limit_rate))
    }

    pub fn actual_value(&self, price: &Decimal) -> Result<Decimal, ProgramError> {
        price
            .try_mul(self.amount)
    }

    pub fn deposit(&mut self, amount: u64) -> ProgramResult {
        self.amount = self.amount
            .checked_add(amount)
            .ok_or(LendingError::MathOverflow)?;
        Ok(())
    }

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

const USER_OBLIGATITION_LEN: usize = 235;

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
        self.last_update.pack_into_slice(last_update);

        collaterals[0] = self.collaterals.len() as u8;
        collaterals[1..1 + COLLATERAL_LEN * self.collaterals.len()]
            .chunks_mut(COLLATERAL_LEN)
            .zip(self.collaterals.iter())
            .for_each(|(data, collateral)| collateral.pack_into_slice(data));
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

        let last_update = LastUpdate::unpack_from_slice(last_update)?;
        let collaterals = collaterals[1..1 + COLLATERAL_LEN * collaterals[0] as usize]
            .chunks(COLLATERAL_LEN)
            .map(|data| Collateral::unpack_from_slice(data))
            .collect::<Result<Vec<_>, ProgramError>>()?;

        Ok(Self{
            version,
            reserve: Pubkey::new_from_array(*reserve),
            owner: Pubkey::new_from_array(*owner),
            last_update,
            collaterals,
            borrowed_amount: u64::from_le_bytes(*borrowed_amount),
            dept_amount: u64::from_le_bytes(*dept_amount),
        })
    }
}

impl UserObligation {
    pub fn update_borrow_interest(&mut self, slot: Slot, borrow_rate: Rate) -> ProgramResult {
        let elapsed = self.last_update.slots_elapsed(slot)?;
        let interest = calculate_interest(self.borrowed_amount, borrow_rate, elapsed)?;

        self.dept_amount = self.dept_amount
            .checked_add(interest)
            .ok_or(LendingError::MathOverflow)?;

        Ok(())
    }

    pub fn borrow_out(&mut self, amount: u64) -> ProgramResult {
        self.dept_amount = self.dept_amount
            .checked_add(amount)
            .ok_or(LendingError::MathOverflow)?;

        self.borrowed_amount = self.borrowed_amount
            .checked_add(amount)
            .ok_or(LendingError::MathOverflow)?;

        Ok(())
    }

    pub fn repay(&mut self, amount: u64) -> Result<Fund, ProgramError> {
        self.dept_amount = self.dept_amount
            .checked_sub(amount)
            .ok_or(LendingError::ObligationDeptAmountInsufficient)?;

        if self.dept_amount < self.borrowed_amount {
            let principal = self.borrowed_amount - self.dept_amount;
            self.borrowed_amount = self.dept_amount;

            Ok(Fund{
                principal,
                interest: amount - principal,
            })
        } else {
            Ok(Fund{
                principal: 0,
                interest: amount,
            })
        }
    }

    pub fn pledge(&mut self, index: usize, amount: u64) -> ProgramResult {
        self.collaterals[index].amount = self.collaterals[index].amount
            .checked_add(amount)
            .ok_or(LendingError::MathOverflow)?;

        Ok(())
    }

    pub fn new_pledge(&mut self, collateral: Collateral) -> ProgramResult {
        if self.collaterals.len() >= MAX_OBLIGATION_COLLATERALS {
            Err(LendingError::ObligationCollateralsLimitExceed.into())
        } else {
            self.collaterals.push(collateral);

            Ok(())
        }
    }

    pub fn redeem(&mut self, index: usize, amount: u64) -> ProgramResult {
        self.collaterals[index].amount = self.collaterals[index].amount
            .checked_sub(amount)
            .ok_or(LendingError::ObligationCollateralAmountInsufficient)?;

        if self.collaterals[index].amount == 0 {
            self.collaterals.remove(index);
        }

        Ok(())
    }

    pub fn redeem_all(&mut self) -> ProgramResult {
        if self.dept_amount > 0 {
            Err(LendingError::ObligationCollateralsNotEmpty.into())
        } else {
            self.collaterals.clear();
            Ok(())
        }
    }

    pub fn liquidate(
        &mut self,
        index: usize,
        liquidate_amount: u64,
        collateral_value: Decimal,
        collaterals_value: Decimal,
    ) -> Result<Fund, ProgramError> {
        // calculate rate
        let rate = collateral_value
            .try_div(collaterals_value)?
            .try_mul(liquidate_amount)?
            .try_div(self.collaterals[index].amount)?;

        // update collaterals
        self.collaterals[index].amount = self.collaterals[index].amount
            .checked_sub(liquidate_amount)
            .ok_or(LendingError::ObligationCollateralAmountInsufficient)?;
        
        if self.collaterals[index].amount == 0 {
            self.collaterals.remove(index);
        }

        // calculate dept
        let repay_amount = Decimal::from(self.dept_amount)
            .try_mul(rate)?
            .try_floor_u64()?;
    
        self.repay(repay_amount)
    }

    pub fn liquidate_all(&mut self) -> Result<Fund, ProgramError> {
        self.collaterals.clear();
        self.repay(self.dept_amount)
    }

    pub fn collaterals_value(&self, settles: &[Settle]) -> Result<Decimal, ProgramError> {
        self.collaterals
            .iter()
            .zip(settles.iter())
            .try_fold(Decimal::zero(), |acc, (collateral, settle)| {
                if collateral.price_oracle == settle.price_oracle {
                    let value = collateral.value(&settle.price)?;
                    acc.try_add(value)
                } else {
                    Err(LendingError::ObligationCollateralsNotMatched.into())
                }
            })
    }

    pub fn collaterals_actual_value(&self, settles: &[Settle]) -> Result<Decimal, ProgramError> {
        self.collaterals
            .iter()
            .zip(settles.iter())
            .try_fold(Decimal::zero(), |acc, (collateral, settle)| {
                if collateral.price_oracle == settle.price_oracle {
                    let value = collateral.actual_value(&settle.price)?;
                    acc.try_add(value)
                } else {
                    Err(LendingError::ObligationCollateralsNotMatched.into())
                }
            })
    }

    pub fn loan_value(&self, price: Decimal) -> Result<Decimal, ProgramError> {
        price.try_mul(self.dept_amount)
    }

    pub fn find_collateral(&self, price_oracle: &Pubkey) -> Option<usize> {
        self.collaterals
            .iter()
            .position(|collateral| &collateral.price_oracle == price_oracle)
    }
}

