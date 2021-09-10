#![allow(missing_docs)]
use super::*;
use crate::{
    error::LendingError,
    math::{Decimal, Rate, TryAdd, TryDiv, TryMul, TrySub}
};
use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use solana_program::{
    entrypoint::ProgramResult,
    program_error::ProgramError,
    program_pack::{IsInitialized, Pack, Sealed},
    pubkey::{Pubkey, PUBKEY_BYTES}
};
use std::{convert::TryInto, cmp::Ordering, iter::Iterator, any::Any};
use typenum::Bit;

/// compute unit comsumed 160000-170000 for 12
const MAX_OBLIGATION_RESERVES: usize = 10;

/// min borrow value (to avoid dust attack), set 1 dollar as default
const MIN_LOANS_VALUE: u128 = 1_000_000_000_000_000_000;

///
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Collateral {
    ///
    pub reserve: Pubkey,
    ///
    pub amount: u64,
    ///
    pub borrow_value_ratio: u8,
    ///
    pub liquidation_value_ratio: u8,
}

impl Collateral {
    ///
    fn calculate_collateral_value(&self, reserve: &MarketReserve) -> Result<Decimal, ProgramError> {
        reserve.market_price
            .try_mul(amount_mul_rate(self.amount, reserve.collateral_to_liquidity_rate()?)?)?
            .try_div(calculate_decimals(reserve.token_info.decimal)?)
    }
}

impl Sealed for Collateral {}

const COLLATERAL_PADDING_LEN: usize = 32;
const COLLATERAL_LEN: usize = 74;

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
            _padding,
        ) = mut_array_refs![
            output,
            PUBKEY_BYTES,
            8,
            1,
            1,
            COLLATERAL_PADDING_LEN
        ];

        reserve.copy_from_slice(self.reserve.as_ref());
        *amount = self.amount.to_le_bytes();
        *borrow_value_ratio = self.borrow_value_ratio.to_le_bytes();
        *liquidation_value_ratio = self.liquidation_value_ratio.to_le_bytes();
    }

    fn unpack_from_slice(input: &[u8]) -> Result<Self, ProgramError> {
        let input = array_ref![input, 0, COLLATERAL_LEN];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            reserve,
            amount,
            borrow_value_ratio,
            liquidation_value_ratio,
            _padding,
        ) = array_refs![
            input,
            PUBKEY_BYTES,
            8,
            1,
            1,
            COLLATERAL_PADDING_LEN
        ];

        Ok(Self{
            reserve: Pubkey::new_from_array(*reserve),
            amount: u64::from_le_bytes(*amount),
            borrow_value_ratio: u8::from_le_bytes(*borrow_value_ratio),
            liquidation_value_ratio: u8::from_le_bytes(*liquidation_value_ratio),
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
    ///
    pub close_ratio: u8,
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
    fn calculate_loan_value(&self, reserve: &MarketReserve) -> Result<Decimal, ProgramError> {
        reserve.market_price
            .try_mul(self.borrowed_amount_wads.try_ceil_u64()?)?
            .try_div(calculate_decimals(reserve.token_info.decimal)?)
    }
}

impl Sealed for Loan {}

const LOAN_PADDING_LEN: usize = 32;
const LOAN_LEN: usize = 97;

impl Pack for Loan {
    const LEN: usize = LOAN_LEN;

    fn pack_into_slice(&self, output: &mut [u8]) {
        let output = array_mut_ref![output, 0, LOAN_LEN];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            reserve,
            acc_borrow_rate_wads,
            borrowed_amount_wads,
            close_ratio,
            _padding,
        ) = mut_array_refs![
            output,
            PUBKEY_BYTES,
            16,
            16,
            1,
            LOAN_PADDING_LEN
        ];

        reserve.copy_from_slice(self.reserve.as_ref());
        pack_decimal(self.acc_borrow_rate_wads, acc_borrow_rate_wads);
        pack_decimal(self.borrowed_amount_wads, borrowed_amount_wads);
        *close_ratio = self.close_ratio.to_le_bytes();
    }

    fn unpack_from_slice(input: &[u8]) -> Result<Self, ProgramError> {
        let input = array_ref![input, 0, LOAN_LEN];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            reserve,
            acc_borrow_rate_wads,
            borrowed_amount_wads,
            close_ratio,
            _padding,
        ) = array_refs![
            input,
            PUBKEY_BYTES,
            16,
            16,
            1,
            LOAN_PADDING_LEN
        ];

        Ok(Self{
            reserve: Pubkey::new_from_array(*reserve),
            acc_borrow_rate_wads: unpack_decimal(acc_borrow_rate_wads),
            borrowed_amount_wads: unpack_decimal(borrowed_amount_wads),
            close_ratio: u8::from_le_bytes(*close_ratio),
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
    fn validate_health(&self, other: Option<Self>) -> ProgramResult {
        let (collaterals_borrow_value, loans_value) = if let Some(other) = other {
            let collaterals_borrow_value = self.collaterals_borrow_value
                .try_add(other.collaterals_borrow_value)?;
            let loans_value = self.loans_value.try_add(other.loans_value)?;

            (collaterals_borrow_value, loans_value)
        } else {
            (self.collaterals_borrow_value, self.loans_value)
        };

        if collaterals_borrow_value >= loans_value {
            Ok(())
        } else {
            Err(LendingError::ObligationNotHealthy.into())
        }
    }
    ///
    fn validate_liquidation(&self, other: Option<Self>) -> ProgramResult {
        let (collaterals_liquidation_value, loans_value) = if let Some(other) = other {
            let collaterals_liquidation_value = self.collaterals_liquidation_value
                .try_add(other.collaterals_liquidation_value)?;
            let loans_value = self.loans_value.try_add(other.loans_value)?;

            (collaterals_liquidation_value, loans_value)
        } else {
            (self.collaterals_liquidation_value, self.loans_value)
        };

        // valid liquidation
        if loans_value >= collaterals_liquidation_value {
            Ok(())
        } else {
            return Err(LendingError::LiquidationNotAvailable.into());
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
            Err(LendingError::ObligationAlreadyBindFriend.into())
        }
    }
    ///
    // need refresh obligation before
    pub fn unbind_friend(&mut self) -> ProgramResult {
        if self.collaterals_liquidation_value > self.loans_value {
            self.friend = COption::None;

            Ok(())
        } else {
            Err(LendingError::ObligationNotHealthy.into())
        }
    }
    ///
    // need refresh reserves before
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

                let collateral_value = collateral.calculate_collateral_value(&reserve)?;
                let borrow_effective_value = collateral_value
                    .try_mul(Rate::from_percent(collateral.borrow_value_ratio))?
                    .try_add(acc_0)?;
                let liquidation_effective_value = collateral_value
                    .try_mul(Rate::from_percent(collateral.liquidation_value_ratio))?
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
    // need refresh obligation before
    pub fn borrow_in(
        &mut self,
        amount: u64,
        index: usize,
        reserve: &MarketReserve,
        other: Option<Self>,
    ) -> Result<u64, ProgramError> {
        let amount = calculate_amount(amount, reserve.liquidity_info.available);
        let value = reserve.market_price
            .try_mul(amount)?
            .try_div(calculate_decimals(reserve.token_info.decimal)?)?;
        self.loans_value = self.loans_value.try_add(value)?;

        if self.loans_value < Decimal::from_scaled_val(MIN_LOANS_VALUE) {
            return Err(LendingError::BorrowTooSmall.into());
        }
        
        self.validate_health(other)?;

        self.loans[index].borrowed_amount_wads = self.loans[index].borrowed_amount_wads.try_add(Decimal::from(amount))?;

        Ok(amount)
    }
    ///
    // need refresh obligation before
    pub fn new_borrow_in(
        &mut self,
        amount: u64,
        key: Pubkey,
        reserve: &MarketReserve,
        other: Option<Self>,
    ) -> Result<u64, ProgramError> {
        if self.collaterals.len() + self.loans.len() >= MAX_OBLIGATION_RESERVES {
            return Err(LendingError::ObligationReservesFull.into());
        }

        let amount = calculate_amount(amount, reserve.liquidity_info.available);
        let value = reserve.market_price
            .try_mul(amount)?
            .try_div(calculate_decimals(reserve.token_info.decimal)?)?;
        self.loans_value = self.loans_value.try_add(value)?;

        if self.loans_value < Decimal::from_scaled_val(MIN_LOANS_VALUE) {
            return Err(LendingError::BorrowTooSmall.into());
        }

        self.validate_health(other)?;

        self.loans.push(Loan{
            reserve: key,
            acc_borrow_rate_wads: reserve.liquidity_info.acc_borrow_rate_wads,
            borrowed_amount_wads: Decimal::from(amount),
            close_ratio: reserve.liquidity_info.config.close_ratio,
        });

        Ok(amount)
    }
    ///
    // need accure reserve and obligation interest before
    pub fn repay(
        &mut self,
        amount: u64,
        balance: u64,
        index: usize,
    ) -> Result<RepaySettle, ProgramError> {
        let (amount, amount_decimal) =
            calculate_amount_and_decimal(amount, self.loans[index].borrowed_amount_wads.min(Decimal::from(balance)))?;

        self.loans[index].borrowed_amount_wads = self.loans[index].borrowed_amount_wads
            .try_sub(amount_decimal)
            .map_err(|_| LendingError::RepayTooMuch)?;

        if self.loans[index].borrowed_amount_wads == Decimal::zero() {
            self.loans.remove(index);
        }

        Ok(RepaySettle {
            amount,
            amount_decimal
        })
    }
    /// mark stale later
    pub fn pledge(
        &mut self,
        amount: u64,
        index: usize,
    ) -> ProgramResult {
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
            Err(LendingError::ObligationReservesFull.into())
        } else {
            self.collaterals.push(Collateral {
                reserve: key,
                amount,
                borrow_value_ratio: reserve.collateral_info.config.borrow_value_ratio,
                liquidation_value_ratio: reserve.collateral_info.config.liquidation_value_ratio,
            });

            Ok(())
        }
    }
    ///
    // need refresh obligation before
    pub fn redeem(
        &mut self,
        amount: u64,
        index: usize,
        reserve: &MarketReserve,
        other: Option<Self>,
    ) -> Result<u64, ProgramError> {
        let amount = calculate_amount(amount, self.collaterals[index].amount);
        let ctl = reserve.collateral_to_liquidity_rate()?;
        let borrow_value_ratio = Rate::from_percent(self.collaterals[index].borrow_value_ratio);
        let before_amount = self.collaterals[index].amount;
        let after_amount = self.collaterals[index].amount
            .checked_sub(amount)
            .ok_or(LendingError::ObligationCollateralInsufficient)?;
        if after_amount == 0 {
            self.collaterals.remove(index);
        } else {
            self.collaterals[index].amount = after_amount;
        }

        let changed_liquidity_amount = amount_mul_rate(before_amount, ctl)?
            .checked_sub(amount_mul_rate(after_amount, ctl)?)
            .ok_or(LendingError::MathOverflow)?;
        let changed_borrow_value = reserve.market_price
            .try_mul(changed_liquidity_amount)?
            .try_div(calculate_decimals(reserve.token_info.decimal)?)?
            .try_mul(borrow_value_ratio)?;

        // update value
        self.collaterals_borrow_value = self.collaterals_borrow_value.try_sub(changed_borrow_value)?;

        self.validate_health(other)?;

        Ok(amount)
    }
    ///
    pub fn redeem_without_loan(&mut self, amount: u64, index: usize, other: Option<Self>) -> Result<u64, ProgramError> {
        let is_empty = other
            .map(|obligation| obligation.loans.is_empty())
            .unwrap_or(true);

        if is_empty && self.loans.is_empty() {
            let amount = calculate_amount(amount, self.collaterals[index].amount);

            self.collaterals[index].amount = self.collaterals[index].amount
                .checked_sub(amount)
                .ok_or(LendingError::ObligationCollateralInsufficient)?;
            if self.collaterals[index].amount == 0 {
                self.collaterals.remove(index);
            }

            Ok(amount)
        } else {
            Err(LendingError::ObligationHasDept.into())
        }
    }
    ///
    // need refresh obligation before
    #[allow(clippy::too_many_arguments)]
    pub fn replace_collateral(
        &mut self,
        in_amount: u64,
        out_index: usize,
        in_key: Pubkey,
        out_reserve: &MarketReserve,
        in_reserve: &MarketReserve,
        other: Option<Self>,
    ) -> Result<u64, ProgramError> {
        let out_amount = self.collaterals[out_index].amount;
        let out_borrow_value_ratio = Rate::from_percent(self.collaterals[out_index].borrow_value_ratio);

        self.collaterals.remove(out_index);
        self.collaterals.push(Collateral {
            reserve: in_key,
            amount: in_amount,
            borrow_value_ratio: in_reserve.collateral_info.config.borrow_value_ratio,
            liquidation_value_ratio: in_reserve.collateral_info.config.liquidation_value_ratio,
        });

        let out_borrow_value = out_reserve.market_price
            .try_mul(amount_mul_rate(out_amount, out_reserve.collateral_to_liquidity_rate()?)?)?
            .try_div(calculate_decimals(out_reserve.token_info.decimal)?)?
            .try_mul(out_borrow_value_ratio)?;
        let in_borrow_value = in_reserve.market_price
            .try_mul(amount_mul_rate(in_amount, in_reserve.collateral_to_liquidity_rate()?)?)?
            .try_div(calculate_decimals(in_reserve.token_info.decimal)?)?
            .try_mul(Rate::from_percent(in_reserve.collateral_info.config.borrow_value_ratio))?;

        self.collaterals_borrow_value = self.collaterals_borrow_value
            .try_sub(out_borrow_value)?
            .try_add(in_borrow_value)?;

        self.validate_health(other)?;

        Ok(out_amount)
    }
    ///
    // need refresh obligation before
    #[allow(clippy::too_many_arguments)]
    pub fn liquidate<IsCollateral: Bit>(
        &mut self,
        amount: u64,
        collateral_index: usize,
        loan_index: usize,
        collateral_reserve: &MarketReserve,
        loan_reserve: &MarketReserve,
        other: Option<Self>,
    ) -> Result<(u64, RepaySettle), ProgramError> {
        // check valid
        self.validate_liquidation(other)?;

        if IsCollateral::BOOL {
            // input amount represents collateral
            let seize_amount = calculate_amount(amount, self.collaterals[collateral_index].amount);

            // update collteral amount
            self.collaterals[collateral_index].amount = self.collaterals[collateral_index].amount
                .checked_sub(seize_amount)
                .ok_or(LendingError::ObligationCollateralInsufficient)?;
            if self.collaterals[collateral_index].amount == 0 {
                self.collaterals.remove(collateral_index);
            }

            // calculate repay amount
            let repay_amount_decimal = collateral_reserve.market_price
                .try_mul(amount_mul_rate(seize_amount, collateral_reserve.collateral_to_liquidity_rate()?)?)?
                .try_div(calculate_decimals(collateral_reserve.token_info.decimal)?)?
                .try_mul(Rate::from_percent(collateral_reserve.collateral_info.config.liquidation_penalty_ratio))?
                .try_mul(calculate_decimals(loan_reserve.token_info.decimal)?)?
                .try_div(loan_reserve.market_price)?;

            // repay amount check
            if repay_amount_decimal == Decimal::zero() {
                return Err(LendingError::LiquidationRepayTooSmall.into());
            }
            let max_repay_amount_decimal = self.loans[loan_index].borrowed_amount_wads
                .try_mul(Rate::from_percent(loan_reserve.liquidity_info.config.close_ratio))?;
            if repay_amount_decimal > max_repay_amount_decimal {
                return Err(LendingError::LiquidationRepayTooMuch.into());
            }

            // update loans
            self.loans[loan_index].borrowed_amount_wads = self.loans[loan_index].borrowed_amount_wads.try_sub(repay_amount_decimal)?;

            Ok((seize_amount, RepaySettle {
                amount: repay_amount_decimal.try_ceil_u64()?,
                amount_decimal: repay_amount_decimal,
            }))
        } else {
            // input amount represents loan
            // calculate repay amount
            let max_repay_amount_decimal = self.loans[loan_index].borrowed_amount_wads
                .try_mul(Rate::from_percent(loan_reserve.liquidity_info.config.close_ratio))?;
            let (repay_amount, repay_amount_decimal) = calculate_amount_and_decimal(amount, max_repay_amount_decimal)?;
            if repay_amount_decimal > max_repay_amount_decimal {
                return Err(LendingError::LiquidationRepayTooMuch.into());
            }

            // update loans
            self.loans[loan_index].borrowed_amount_wads = self.loans[loan_index].borrowed_amount_wads.try_sub(repay_amount_decimal)?;

            // calculate seize amount
            let seize_amount = loan_reserve.market_price
                .try_mul(repay_amount)?
                .try_div(calculate_decimals(loan_reserve.token_info.decimal)?)?
                .try_mul(calculate_decimals(collateral_reserve.token_info.decimal)?)?
                .try_div(collateral_reserve.market_price)?
                .try_div(collateral_reserve.collateral_to_liquidity_rate()?)?
                .try_div(Rate::from_percent(collateral_reserve.collateral_info.config.liquidation_penalty_ratio))?
                .try_floor_u64()?;

            // update collaterals
            self.collaterals[collateral_index].amount = self.collaterals[collateral_index].amount
                .checked_sub(seize_amount)
                .ok_or(LendingError::ObligationCollateralInsufficient)?;
            if self.collaterals[collateral_index].amount == 0 {
                self.collaterals.remove(collateral_index);
            }

            Ok((seize_amount, RepaySettle {
                amount: repay_amount,
                amount_decimal: repay_amount_decimal,
            }))
        }
    }
}

impl Sealed for UserObligation {}
impl IsInitialized for UserObligation {
    fn is_initialized(&self) -> bool {
        self.version != UNINITIALIZED_VERSION
    }
}

// const MAX_PADDING_LEN: usize = max(COLLATERAL_LEN, LOAN_LEN);
const MAX_COLLATERAL_OR_LOAN_LEN: usize = LOAN_LEN;
const USER_OBLIGATITION_PADDING_LEN: usize = 128;
const USER_OBLIGATITION_LEN: usize = 1258;

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
            _padding,
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
            MAX_COLLATERAL_OR_LOAN_LEN * MAX_OBLIGATION_RESERVES,
            USER_OBLIGATITION_PADDING_LEN
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
            _padding,
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
            MAX_COLLATERAL_OR_LOAN_LEN * MAX_OBLIGATION_RESERVES,
            USER_OBLIGATITION_PADDING_LEN
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

impl<P: Any + Param + Copy> Operator<P> for UserObligation {
    fn operate_unchecked(&mut self, param: P) -> ProgramResult {
        if let Some(config) = <dyn Any>::downcast_ref::<IndexedCollateralConfig>(&param) {
            let collateral = self.collaterals
                .get_mut(config.index as usize)
                .ok_or(LendingError::ObligationInvalidCollateralIndex)?;
            collateral.borrow_value_ratio = config.borrow_value_ratio;
            collateral.liquidation_value_ratio = config.liquidation_value_ratio;

            return Ok(());
        }

        if let Some(config) = <dyn Any>::downcast_ref::<IndexedLoanConfig>(&param) {
            let loan = self.loans
                .get_mut(config.index as usize)
                .ok_or(LendingError::ObligationInvalidLoanIndex)?;
            loan.close_ratio = config.close_ratio;

            return Ok(());
        }

        panic!("unexpected param type");
    }
}

#[derive(Clone, Debug, Copy, Default, PartialEq)]
pub struct IndexedCollateralConfig {
    ///
    pub index: u8,
    ///
    pub borrow_value_ratio: u8,
    ///
    pub liquidation_value_ratio: u8,
}

impl Param for IndexedCollateralConfig {
    fn assert_valid(&self) -> ProgramResult {
        if self.borrow_value_ratio < self.liquidation_value_ratio &&
            self.liquidation_value_ratio < 100 {
            Ok(())
        } else {
            Err(LendingError::InvalidIndexedCollateralConfig.into())
        }
    }
}

#[derive(Clone, Debug, Copy, Default, PartialEq)]
pub struct IndexedLoanConfig {
    ///
    pub index: u8,
    ///
    pub close_ratio: u8,
}

impl Param for IndexedLoanConfig {
    fn assert_valid(&self) -> ProgramResult {
        if self.close_ratio < 100 {
            Ok(())
        } else {
            Err(LendingError::InvalidIndexedLoanConfig.into())
        }
    }
}

#[cfg(test)]
mod test {
    
}