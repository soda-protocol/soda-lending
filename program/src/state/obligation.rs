#![allow(missing_docs)]
use super::*;
use crate::{
    error::LendingError,
    math::{Decimal, Rate, TryAdd, TryDiv, TryMul, TrySub, PERCENT_SCALER}
};
use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use solana_program::{
    entrypoint::ProgramResult,
    program_error::ProgramError,
    program_pack::{IsInitialized, Pack, Sealed},
    pubkey::{Pubkey, PUBKEY_BYTES}
};
use std::{convert::TryInto, cmp::Ordering, iter::Iterator};

/// 
const MAX_OBLIGATION_RESERVES: usize = 8;

///
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Collateral {
    ///
    pub reserve: Pubkey,
    ///
    pub amount: u64,
    ///
    pub borrow_value_ratio: u64,
    ///
    pub liquidation_value_ratio: u64,
    ///
    pub close_factor: u64,
}

impl Collateral {
    ///
    pub fn borrow_effective_value(&self, reserve: &MarketReserve) -> Result<Decimal, ProgramError> {
        reserve.market_price
                .try_mul(reserve.exchange_collateral_to_liquidity(self.amount)?)?
                .try_div(calculate_decimals(reserve.token_info.decimal)?)?
                .try_mul(Rate::from_scaled_val(self.borrow_value_ratio))
    }
    ///
    pub fn liquidation_effective_value(&self, reserve: &MarketReserve) -> Result<Decimal, ProgramError> {
        reserve.market_price
            .try_mul(reserve.exchange_collateral_to_liquidity(self.amount)?)?
            .try_div(calculate_decimals(reserve.token_info.decimal)?)?
            .try_mul(Rate::from_scaled_val(self.liquidation_value_ratio))
    }
}

impl Sealed for Collateral {}

const COLLATERAL_RESERVE_LEN: usize = 64;
const COLLATERAL_LEN: usize = 128;

impl Pack for Collateral {
    const LEN: usize = COLLATERAL_LEN;

    fn pack_into_slice(&self, output: &mut [u8]) {
        let output = array_mut_ref![output, 0, COLLATERAL_LEN];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            reserve,
            amount,
            borrow_value_ratio,
            liquidation_value_ratio,
            close_factor,
            _rest,
        ) = mut_array_refs![
            output,
            PUBKEY_BYTES,
            8,
            8,
            8,
            8,
            COLLATERAL_RESERVE_LEN
        ];

        reserve.copy_from_slice(self.reserve.as_ref());
        *amount = self.amount.to_le_bytes();
        *borrow_value_ratio = self.borrow_value_ratio.to_le_bytes();
        *liquidation_value_ratio = self.liquidation_value_ratio.to_le_bytes();
        *close_factor = self.close_factor.to_le_bytes();
    }

    fn unpack_from_slice(input: &[u8]) -> Result<Self, ProgramError> {
        let input = array_ref![input, 0, COLLATERAL_LEN];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            reserve,
            amount,
            borrow_value_ratio,
            liquidation_value_ratio,
            close_factor,
            _rest,
        ) = array_refs![
            input,
            PUBKEY_BYTES,
            8,
            8,
            8,
            8,
            COLLATERAL_RESERVE_LEN
        ];

        Ok(Self{
            reserve: Pubkey::new_from_array(*reserve),
            amount: u64::from_le_bytes(*amount),
            borrow_value_ratio: u64::from_le_bytes(*borrow_value_ratio),
            liquidation_value_ratio: u64::from_le_bytes(*liquidation_value_ratio),
            close_factor: u64::from_le_bytes(*close_factor),
        })
    }
}

///
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Loan {
    ///
    pub reserve: Pubkey,
    /// Borrow rate used for calculating interest
    pub acc_borrow_rate_wads: Decimal,
    /// Amount of liquidity borrowed plus interest
    pub borrowed_amount_wads: Decimal,
}

impl Loan {
    ///
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
    ///
    pub fn calculate_loan_value(&self, reserve: &MarketReserve) -> Result<Decimal, ProgramError> {
        reserve.market_price
            .try_mul(self.borrowed_amount_wads.try_ceil_u64()?)?
            .try_div(calculate_decimals(reserve.token_info.decimal)?)
    }
}

impl Sealed for Loan {}

const LOAN_RESERVE_LEN: usize = 64;
const LOAN_LEN: usize = 128;

impl Pack for Loan {
    const LEN: usize = LOAN_LEN;

    fn pack_into_slice(&self, output: &mut [u8]) {
        let output = array_mut_ref![output, 0, LOAN_LEN];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            reserve,
            acc_borrow_rate_wads,
            borrowed_amount_wads,
            _rest,
        ) = mut_array_refs![
            output,
            PUBKEY_BYTES,
            16,
            16,
            LOAN_RESERVE_LEN
        ];

        reserve.copy_from_slice(self.reserve.as_ref());
        pack_decimal(self.acc_borrow_rate_wads, acc_borrow_rate_wads);
        pack_decimal(self.borrowed_amount_wads, borrowed_amount_wads);
    }

    fn unpack_from_slice(input: &[u8]) -> Result<Self, ProgramError> {
        let input = array_ref![input, 0, LOAN_LEN];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            reserve,
            acc_borrow_rate_wads,
            borrowed_amount_wads,
            _rest,
        ) = array_refs![
            input,
            PUBKEY_BYTES,
            16,
            16,
            LOAN_RESERVE_LEN
        ];

        Ok(Self{
            reserve: Pubkey::new_from_array(*reserve),
            acc_borrow_rate_wads: unpack_decimal(acc_borrow_rate_wads),
            borrowed_amount_wads: unpack_decimal(borrowed_amount_wads),
        })
    }
}

/// Lending market obligation state
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UserObligation {
    /// Version of the struct
    pub version: u8,
    ///
    pub manager: Pubkey,
    ///
    pub owner: Pubkey,
    ///
    pub last_update: LastUpdate,
    ///
    pub friend: COption<Pubkey>,
    /// 
    pub collaterals: Vec<Collateral>,
    ///
    pub collaterals_borrow_value: Decimal,
    ///
    pub collaterals_liquidation_value: Decimal,
    ///
    pub loans: Vec<Loan>,
    ///
    pub loans_value: Decimal,
}

impl UserObligation {
    ///
    fn validate_borrow(&self, other: Option<Self>) -> ProgramResult {
        let (collaterals_borrow_value, loans_value) = if let Some(other) = other {
            let collaterals_borrow_value = self.collaterals_borrow_value
                .try_add(other.collaterals_borrow_value)?;
            let loans_value = self.loans_value.try_add(other.loans_value)?;

            (collaterals_borrow_value, loans_value)
        } else {
            (self.collaterals_borrow_value, self.loans_value)
        };

        if collaterals_borrow_value > loans_value {
            Ok(())
        } else {
            Err(LendingError::ObligationNotHealthy.into())
        }
    }
    ///
    pub fn find_loan(&self, key: Pubkey) -> Result<usize, ProgramError> {
        self.loans
            .iter()
            .position(|loan| loan.reserve == key)
            .ok_or(LendingError::ObligationLoanNotFound.into())
    }
    ///
    pub fn find_collateral(&self, key: Pubkey) -> Result<usize, ProgramError> {
        self.collaterals
            .iter()
            .position(|collateral| collateral.reserve == key)
            .ok_or(LendingError::ObligationCollateralNotFound.into())
    }
    ///
    pub fn bind_friend(&mut self, other: Pubkey) -> ProgramResult {
        if self.friend.is_none() {
            self.friend = COption::Some(other);

            Ok(())
        } else {
            Err(LendingError::ObligationAlreadyBinding.into())
        }
    }
    ///
    // need update obligation before
    pub fn unbind_friend(&mut self) -> ProgramResult {
        if self.collaterals_liquidation_value < self.loans_value {
            self.friend = COption::None;

            Ok(())
        } else {
            Err(LendingError::ObligationNotHealthy.into())
        }
    }
    ///
    // need update reserves before
    pub fn update_user_obligation<'a, I>(&mut self, reserve_iter: &mut I) -> ProgramResult
    where
        I: Iterator<Item = &'a (Pubkey, MarketReserve)>,
    {
        let (collaterals_borrow_value, collaterals_liquidation_value) = self.collaterals
            .iter()
            .try_fold((Decimal::zero(), Decimal::zero()),
                |(acc_0, acc_1), collateral| -> Result<_, ProgramError> {
                let (key, reserve) = reserve_iter
                    .next()
                    .ok_or(ProgramError::NotEnoughAccountKeys)?;

                if key != &collateral.reserve {
                    return Err(LendingError::ObligationCollateralsNotMatched.into());
                }

                let borrow_effective_value = collateral
                    .borrow_effective_value(&reserve)?
                    .try_add(acc_0)?;

                let liquidation_effective_value = collateral
                    .liquidation_effective_value(&reserve)?
                    .try_add(acc_1)?;

                Ok((borrow_effective_value, liquidation_effective_value))
            })?;

        self.collaterals_borrow_value = collaterals_borrow_value;
        self.collaterals_liquidation_value = collaterals_liquidation_value;
        self.loans_value = self.loans
            .iter_mut()
            .try_fold(Decimal::zero(), |acc, loan| {
                let (key, reserve) = reserve_iter
                    .next()
                    .ok_or(ProgramError::NotEnoughAccountKeys)?;

                if key != &loan.reserve {
                    return Err(LendingError::ObligationLoansNotMatched.into());
                }

                loan.accrue_interest(reserve)?;
                loan
                    .calculate_loan_value(reserve)?
                    .try_add(acc)
            })?;

        Ok(())
    }
    ///
    // need update obligation before
    pub fn borrow_in(
        &mut self,
        amount: u64,
        index: usize,
        reserve: &MarketReserve,
        other: Option<Self>,
    ) -> Result<BorrowWithFee, ProgramError> {
        self.loans[index].borrowed_amount_wads = self.loans[index].borrowed_amount_wads
            .try_add(Decimal::from(amount))?;

        let value = reserve.market_price
            .try_mul(amount)?
            .try_div(calculate_decimals(reserve.token_info.decimal)?)?;
        self.loans_value = self.loans_value.try_add(value)?;

        self.validate_borrow(other)?;

        BorrowWithFee::new(amount, Rate::from_scaled_val(reserve.liquidity_info.config.borrow_fee_rate))
    }
    ///
    // need update obligation before
    pub fn new_borrow_in(
        &mut self,
        amount: u64, 
        key: Pubkey,
        reserve: &MarketReserve,
        other: Option<Self>,
    ) -> Result<BorrowWithFee, ProgramError> {
        if self.collaterals.len() + self.loans.len() >= MAX_OBLIGATION_RESERVES {
            return Err(LendingError::ObligationReserveLimitExceed.into());
        }

        self.loans.push(Loan{
            reserve: key,
            acc_borrow_rate_wads: reserve.liquidity_info.acc_borrow_rate_wads,
            borrowed_amount_wads: Decimal::from(amount),
        });

        let value = reserve.market_price
            .try_mul(amount)?
            .try_div(calculate_decimals(reserve.token_info.decimal)?)?;
        self.loans_value = self.loans_value.try_add(value)?;

        self.validate_borrow(other)?;

        BorrowWithFee::new(amount, Rate::from_scaled_val(reserve.liquidity_info.config.borrow_fee_rate))
    }
    ///
    // need accure reserve and obligation interest before
    pub fn repay(
        &mut self,
        amount: u64,
        index: usize,
    ) -> Result<u64, ProgramError> {
        let amount_decimal = Decimal::from(amount);
        if amount_decimal >= self.loans[index].borrowed_amount_wads {
            let amount = self.loans[index].borrowed_amount_wads.try_ceil_u64()?;
            self.loans.remove(index);

            Ok(amount)
        } else {
            self.loans[index].borrowed_amount_wads = self.loans[index].borrowed_amount_wads.try_sub(amount_decimal)?;

            Ok(amount)
        }
    }
    ///
    pub fn pledge(&mut self, amount: u64, index: usize) -> ProgramResult {
        self.collaterals[index].amount = self.collaterals[index].amount
            .checked_add(amount)
            .ok_or(LendingError::MathOverflow)?;

        Ok(())
    }
    ///
    pub fn new_pledge(
        &mut self,
        amount: u64,
        key: Pubkey,
        reserve: &MarketReserve,
    ) -> ProgramResult {
        if self.collaterals.len() + self.loans.len() >= MAX_OBLIGATION_RESERVES {
            Err(LendingError::ObligationReserveLimitExceed.into())
        } else {
            self.collaterals.push(Collateral {
                reserve: key,
                amount,
                borrow_value_ratio: reserve.collateral_info.config.borrow_value_ratio as u64 * PERCENT_SCALER,
                liquidation_value_ratio: reserve.collateral_info.config.liquidation_value_ratio as u64 * PERCENT_SCALER,
                close_factor: reserve.collateral_info.config.close_factor as u64 * PERCENT_SCALER,
            });

            Ok(())
        }
    }
    ///
    // need update obligation before
    pub fn redeem(
        &mut self,
        amount: u64,
        index: usize,
        reserve: &MarketReserve,
        other: Option<Self>,
    ) -> Result<u64, ProgramError> {
        let amount = if amount >= self.collaterals[index].amount {
            let amount = self.collaterals[index].amount;
            self.collaterals.remove(index);

            amount
        } else {
            self.collaterals[index].amount -= amount;

            amount
        };

        let value = reserve.market_price
            .try_mul(reserve.exchange_collateral_to_liquidity(amount)?)?
            .try_div(calculate_decimals(reserve.token_info.decimal)?)?;
        self.collaterals_borrow_value = self.collaterals_borrow_value.try_sub(value)?;

        self.validate_borrow(other)?;

        Ok(amount)
    }
    ///
    pub fn redeem_without_loan(&mut self, amount: u64, index: usize, other: Option<Self>) -> Result<u64, ProgramError> {
        let loan_amount = if let Some(other) = other {
            self.loans
                .iter()
                .chain(other.loans.iter())
                .try_fold(Decimal::one(), |acc, loan| loan.borrowed_amount_wads.try_add(acc))?
        } else {
            self.loans
                .iter()
                .try_fold(Decimal::one(), |acc, loan| loan.borrowed_amount_wads.try_add(acc))?
        };

        if loan_amount > Decimal::zero() {
            Err(LendingError::ObligationDeptNotEmpty.into())
        } else {
            if amount >= self.collaterals[index].amount {
                let amount = self.collaterals[index].amount;
                self.collaterals.remove(index);

                Ok(amount)
            } else {
                self.collaterals[index].amount -= amount;

                Ok(amount)
            }
        }
    }
    ///
    // need update obligation before
    #[allow(clippy::too_many_arguments)]
    pub fn replace_collateral(
        &mut self,
        in_amount: u64,
        out_index: usize,
        in_index: usize,
        out_reserve: &MarketReserve,
        in_reserve: &MarketReserve,
        other: Option<Self>,
    ) -> Result<u64, ProgramError> {
        let out_amount = self.collaterals[out_index].amount;
        self.collaterals.remove(out_index);

        self.collaterals[in_index].amount = self.collaterals[in_index].amount
            .checked_add(in_amount)
            .ok_or(LendingError::MathOverflow)?;

        let out_value = out_reserve.market_price
            .try_mul(out_reserve.exchange_collateral_to_liquidity(out_amount)?)?
            .try_div(calculate_decimals(out_reserve.token_info.decimal)?)?;
        let in_value = in_reserve.market_price
            .try_mul(in_reserve.exchange_collateral_to_liquidity(in_amount)?)?
            .try_div(calculate_decimals(in_reserve.token_info.decimal)?)?;
        self.collaterals_borrow_value = self.collaterals_borrow_value
            .try_sub(out_value)?
            .try_add(in_value)?;

        self.validate_borrow(other)?;

        Ok(out_amount)
    }
    ///
    // need update obligation before
    #[allow(clippy::too_many_arguments)]
    pub fn replace_collateral_for_new(
        &mut self,
        in_amount: u64,
        out_index: usize,
        in_key: Pubkey,
        out_reserve: &MarketReserve,
        in_reserve: &MarketReserve,
        other: Option<Self>,
    ) -> Result<u64, ProgramError> {
        let out_amount = self.collaterals[out_index].amount;
        self.collaterals.remove(out_index);

        self.collaterals.push(Collateral {
            reserve: in_key,
            amount: in_amount,
            borrow_value_ratio: in_reserve.collateral_info.config.borrow_value_ratio as u64 * PERCENT_SCALER,
            liquidation_value_ratio: in_reserve.collateral_info.config.liquidation_value_ratio as u64 * PERCENT_SCALER,
            close_factor: in_reserve.collateral_info.config.close_factor as u64 * PERCENT_SCALER,
        });

        let out_value = out_reserve.market_price
            .try_mul(out_reserve.exchange_collateral_to_liquidity(out_amount)?)?
            .try_div(calculate_decimals(out_reserve.token_info.decimal)?)?;
        let in_value = in_reserve.market_price
            .try_mul(in_reserve.exchange_collateral_to_liquidity(in_amount)?)?
            .try_div(calculate_decimals(in_reserve.token_info.decimal)?)?;
        self.collaterals_borrow_value = self.collaterals_borrow_value
            .try_sub(out_value)?
            .try_add(in_value)?;

        self.validate_borrow(other)?;

        Ok(out_amount)
    }
    ///
    // need update obligation before
    #[allow(clippy::too_many_arguments)]
    pub fn liquidate(
        &mut self,
        amount: u64,
        collateral_index: usize,
        loan_index: usize,
        collateral_reserve: &MarketReserve,
        loan_reserve: &MarketReserve,
        other: Option<Self>,
    ) -> Result<(u64, LiquidationWithFee), ProgramError> {
        let (collaterals_liquidation_value, loans_value) = if let Some(other) = other {
            let collaterals_liquidation_value = self.collaterals_liquidation_value
                .try_add(other.collaterals_liquidation_value)?;
            let loans_value = self.loans_value.try_add(self.loans_value)?;

            (collaterals_liquidation_value, loans_value)
        } else {
            (self.collaterals_liquidation_value, self.loans_value)
        };

        // valid liquidation
        if loans_value < collaterals_liquidation_value {
            return Err(LendingError::ObligationLiquidationNotAvailable.into());
        }
        
        // max liquidation limit check
        let max_liquidation_amount = Decimal::from(self.collaterals[collateral_index].amount)
            .try_mul(Rate::from_scaled_val(self.collaterals[collateral_index].close_factor))?
            .try_floor_u64()?;
        let amount = amount.min(max_liquidation_amount);

        let collateral_amount = collateral_reserve.exchange_collateral_to_liquidity(amount)?;
        let collateral_value = collateral_reserve.market_price
            .try_mul(collateral_amount)?
            .try_div(calculate_decimals(collateral_reserve.token_info.decimal)?)?;

        let liquidation_ratio: Rate = collateral_value
            .try_mul(Rate::from_scaled_val(self.collaterals[collateral_index].liquidation_value_ratio))?
            .try_div(collaterals_liquidation_value)?
            .try_into()?;

        // calculate repay amount
        let loan_decimals = calculate_decimals(loan_reserve.token_info.decimal)?;
        let repay_amount = loans_value
            .try_mul(loan_decimals)?
            .try_div(loan_reserve.market_price)?
            .try_mul(liquidation_ratio)?;
    
        // update collaterals and loans        
        self.collaterals[collateral_index].amount -= amount;
        self.loans[loan_index].borrowed_amount_wads = self.loans[loan_index].borrowed_amount_wads
            .try_sub(repay_amount)?;

        let liquidation_with_fee = LiquidationWithFee::new(
            collateral_value,
            loan_reserve.market_price,
            repay_amount,
            loan_decimals,
            Rate::from_scaled_val(loan_reserve.liquidity_info.config.liquidation_fee_rate),
        )?;

        Ok((amount, liquidation_with_fee))
    }
}

impl Sealed for UserObligation {}
impl IsInitialized for UserObligation {
    fn is_initialized(&self) -> bool {
        self.version != UNINITIALIZED_VERSION
    }
}

// const MAX_RESERVE_LEN: usize = max(COLLATERAL_LEN, LOAN_LEN);
const MAX_RESERVE_LEN: usize = 128;
const USER_OBLIGATITION_RESERVE_LEN: usize = 128;
const USER_OBLIGATITION_LEN: usize = 1312;

impl Pack for UserObligation {
    const LEN: usize = USER_OBLIGATITION_LEN;

    fn pack_into_slice(&self, output: &mut [u8]) {
        let output = array_mut_ref![output, 0, USER_OBLIGATITION_LEN];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            version,
            manager,
            owner,
            last_update,
            friend,
            collaterals_borrow_value,
            collaterals_liquidation_value,
            loans_value,
            collaterals_len,
            loans_len,
            data_flatten,
            _rest,
        ) = mut_array_refs![
            output,
            1,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            LAST_UPDATE_LEN,
            COPTION_LEN + PUBKEY_BYTES,
            16,
            16,
            16,
            1,
            1,
            MAX_RESERVE_LEN * MAX_OBLIGATION_RESERVES,
            USER_OBLIGATITION_RESERVE_LEN
        ];

        *version = self.version.to_le_bytes();
        manager.copy_from_slice(self.manager.as_ref());
        owner.copy_from_slice(self.owner.as_ref());
        self.last_update.pack_into_slice(&mut last_update[..]);
        pack_coption_pubkey(&self.friend, friend);
        pack_decimal(self.collaterals_borrow_value, collaterals_borrow_value);
        pack_decimal(self.collaterals_liquidation_value, collaterals_liquidation_value);
        pack_decimal(self.loans_value, loans_value);
        *collaterals_len = (self.collaterals.len() as u8).to_le_bytes();
        *loans_len = (self.loans.len() as u8).to_le_bytes();

        let collaterals_offset = self.collaterals.len() * COLLATERAL_LEN;
        let loans_offset = collaterals_offset + self.loans.len() * LOAN_LEN;

        data_flatten[..collaterals_offset]
            .chunks_exact_mut(COLLATERAL_LEN)
            .zip(self.collaterals.iter())
            .for_each(|(data, collateral)| collateral.pack_into_slice(data));
        data_flatten[collaterals_offset..loans_offset]
            .chunks_exact_mut(LOAN_LEN)
            .zip(self.loans.iter())
            .for_each(|(data, loan)| loan.pack_into_slice(data));
    }

    fn unpack_from_slice(input: &[u8]) -> Result<Self, ProgramError> {
        let input = array_ref![input, 0, USER_OBLIGATITION_LEN];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            version,
            manager,
            owner,
            last_update,
            friend,
            collaterals_borrow_value,
            collaterals_liquidation_value,
            loans_value,
            collaterals_len,
            loans_len,
            data_flatten,
            _rest,
        ) = array_refs![
            input,
            1,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            LAST_UPDATE_LEN,
            COPTION_LEN + PUBKEY_BYTES,
            16,
            16,
            16,
            1,
            1,
            MAX_RESERVE_LEN * MAX_OBLIGATION_RESERVES,
            USER_OBLIGATITION_RESERVE_LEN
        ];

        let version = u8::from_le_bytes(*version);
        if version > PROGRAM_VERSION {
            msg!("UserObligation version does not match lending program version");
            return Err(ProgramError::InvalidAccountData);
        }

        let collaterals_offset = u8::from_le_bytes(*collaterals_len) as usize * COLLATERAL_LEN;
        let loans_offset = collaterals_offset + u8::from_le_bytes(*loans_len) as usize * LOAN_LEN;

        let collaterals = data_flatten[..collaterals_offset]
            .chunks_exact(COLLATERAL_LEN)
            .map(|data| Collateral::unpack_from_slice(data))
            .collect::<Result<Vec<_>, ProgramError>>()?;

        let loans = data_flatten[collaterals_offset..loans_offset]
            .chunks_exact(LOAN_LEN)
            .map(|data| Loan::unpack_from_slice(data))
            .collect::<Result<Vec<_>, ProgramError>>()?;

        Ok(Self{
            version,
            manager: Pubkey::new_from_array(*manager),
            owner: Pubkey::new_from_array(*owner),
            last_update: LastUpdate::unpack_from_slice(&last_update[..])?,
            friend: unpack_coption_pubkey(friend)?,
            collaterals,
            collaterals_borrow_value: unpack_decimal(collaterals_borrow_value),
            collaterals_liquidation_value: unpack_decimal(collaterals_liquidation_value),
            loans,
            loans_value: unpack_decimal(loans_value),
        })
    }
}