#![allow(missing_docs)]
use std::{convert::TryInto, any::Any};
///
use super::*;
use crate::{error::LendingError, math::{Rate, TryDiv, TrySub, WAD}};
use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use solana_program::{
    clock::Slot, 
    entrypoint::ProgramResult, 
    msg, 
    program_error::ProgramError,
    program_pack::{IsInitialized, Pack, Sealed}, 
    pubkey::{Pubkey, PUBKEY_BYTES}
};

///
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TokenInfo {
    ///
    pub mint_pubkey: Pubkey,
    ///
    pub account: Pubkey,
    ///
    pub price_oracle: Pubkey,
    ///
    pub decimal: u8,
}

///
#[derive(Clone, Debug, Copy, Default, PartialEq)]
pub struct CollateralConfig {
    ///
    pub borrow_value_ratio: u8,
    ///
    pub liquidation_value_ratio: u8,
    ///
    pub liquidation_bonus_ratio: u8,
}

impl Param for CollateralConfig {
    fn assert_valid(&self) -> ProgramResult {
        if self.borrow_value_ratio < self.liquidation_value_ratio && self.liquidation_value_ratio < 100 {
            Ok(())
        } else {
            Err(LendingError::InvalidCollateralConfig.into())
        }
    }
}

///
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CollateralInfo {
    ///
    pub sotoken_mint_pubkey: Pubkey,
    ///
    pub total_mint: u64,
    ///
    pub config: CollateralConfig,
}

impl CollateralInfo {
    ///
    pub fn mint(&mut self, amount: u64) -> ProgramResult {
        self.total_mint = self.total_mint
            .checked_add(amount)
            .ok_or(LendingError::MathOverflow)?;

        Ok(())
    }
    ///
    pub fn burn(&mut self, amount: u64) -> ProgramResult {
        self.total_mint = self.total_mint
            .checked_sub(amount)
            .ok_or(LendingError::MathOverflow)?;
    
        Ok(())
    }
}

///
#[derive(Clone, Debug, Copy, Default, PartialEq)]
pub struct LiquidityConfig {
    ///
    pub close_factor: u8,
    ///
    pub borrow_tax_rate: u8,
    ///
    pub flash_loan_fee_rate: u64,
    ///
    pub max_deposit: u64,
    ///
    pub max_acc_deposit: u64,
}

impl Param for LiquidityConfig {
    fn assert_valid(&self) -> ProgramResult {
        if self.close_factor < 100 &&
            self.borrow_tax_rate < 100 &&
            self.flash_loan_fee_rate < WAD &&
            self.max_deposit <= self.max_acc_deposit {
            Ok(())
        } else {
            Err(LendingError::InvalidLiquidityConfig.into())
        }
    }
}

///
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LiquidityInfo {
    ///
    pub enable: bool,
    ///
    pub rate_oracle: Pubkey,
    ///
    pub available: u64,
    ///
    pub flash_loan_fee: u64,
    ///
    pub acc_borrow_rate_wads: Decimal,
    ///
    pub borrowed_amount_wads: Decimal,
    ///
    pub insurance_wads: Decimal,
    ///
    pub config: LiquidityConfig,
}

impl LiquidityInfo {
    ///
    fn total_supply(&self) -> Result<Decimal, ProgramError> {
        Decimal::from(self.available).try_add(self.borrowed_amount_wads)
    }
    ///
    pub fn utilization_rate(&self) -> Result<Rate, ProgramError> {
        let total_supply = self.total_supply()?;
        if total_supply == Decimal::zero() {
            Ok(Rate::zero())
        } else {
            self.borrowed_amount_wads
                .try_div(total_supply)?
                .try_into()
        }
    }
    ///
    pub fn deposit(&mut self, amount: u64) -> ProgramResult {
        if !self.enable {
            return Err(LendingError::MarketReserveDisabled.into());
        }
        
        if amount > self.config.max_deposit {
            return Err(LendingError::MarketReserveDepositTooMuch.into());
        }

        self.available = self.available
            .checked_add(amount)
            .ok_or(LendingError::MathOverflow)?;

        if self.available <= self.config.max_acc_deposit {
            Ok(())
        } else {
            Err(LendingError::MarketReserveAccDepositTooMuch.into())
        }
    }
    ///
    pub fn withdraw(&mut self, amount: u64) -> ProgramResult {
        if !self.enable {
            return Err(LendingError::MarketReserveDisabled.into());
        }

        self.available = self.available
            .checked_sub(amount)
            .ok_or(LendingError::MarketReserveInsufficentLiquidity)?;

        Ok(())
    }
    ///
    pub fn borrow_out(&mut self, amount: u64) -> ProgramResult {
        if !self.enable {
            return Err(LendingError::MarketReserveDisabled.into());
        }

        self.available = self.available
            .checked_sub(amount)
            .ok_or(LendingError::MarketReserveInsufficentLiquidity)?;
        self.borrowed_amount_wads = self.borrowed_amount_wads.try_add(Decimal::from(amount))?;

        Ok(())
    }
    ///
    pub fn flash_loan_borrow_out(&mut self, amount: u64) -> Result<(u64, u64), ProgramError> {
        if !self.enable {
            return Err(LendingError::MarketReserveDisabled.into());
        }

        self.available = self.available
            .checked_sub(amount)
            .ok_or(LendingError::MarketReserveInsufficentLiquidity)?;
        self.borrowed_amount_wads = self.borrowed_amount_wads.try_add(Decimal::from(amount))?;

        let fee = Decimal::from(amount)
            .try_mul(Rate::from_scaled_val(self.config.flash_loan_fee_rate))?
            .try_ceil_u64()?;

        let total_repay = amount
            .checked_add(fee)
            .ok_or(LendingError::MathOverflow)?;

        Ok((total_repay, fee))
    }
    ///
    pub fn repay(&mut self, settle: RepaySettle) -> ProgramResult {
        if !self.enable {
            return Err(LendingError::MarketReserveDisabled.into());
        }

        self.available = self.available
            .checked_add(settle.amount)
            .ok_or(LendingError::MathOverflow)?;
        self.borrowed_amount_wads = self.borrowed_amount_wads.try_sub(settle.amount_decimal)?;

        Ok(())
    }
    ///
    pub fn flash_loan_repay(&mut self, amount: u64, fee: u64) -> ProgramResult {
        self.available = self.available
            .checked_add(amount)
            .ok_or(LendingError::MarketReserveInsufficentLiquidity)?;
        self.borrowed_amount_wads = self.borrowed_amount_wads.try_sub(Decimal::from(amount))?;
        self.flash_loan_fee = self.flash_loan_fee
            .checked_add(fee)
            .ok_or(LendingError::MathOverflow)?;

        Ok(())
    }
    ///
    pub fn reduce_insurance(&mut self, amount: u64) -> ProgramResult {
        if amount <= self.flash_loan_fee {
            self.flash_loan_fee -= amount;
        } else {
            let amount = amount - self.flash_loan_fee;
            self.flash_loan_fee = 0;
            self.insurance_wads = self.insurance_wads.try_sub(Decimal::from(amount))?;
        }
        
        Ok(())
    }
}

/// Lending market reserve state
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MarketReserve {
    /// Version of the struct
    pub version: u8,
    ///
    pub last_update: LastUpdate,
    /// 
    pub manager: Pubkey,
    ///
    pub market_price: Decimal,
    ///
    pub token_info: TokenInfo,
    ///
    pub collateral_info: CollateralInfo,
    ///
    pub liquidity_info: LiquidityInfo,
}

impl MarketReserve {
    ///
    pub fn exchange_liquidity_to_collateral(&self, amount: u64) -> Result<u64, ProgramError> {
        let total_supply = self.liquidity_info.total_supply()?;
        let exchange_rate = if total_supply == Decimal::zero() {
            Rate::one()
        } else {
            Decimal::from(self.collateral_info.total_mint)
                .try_div(total_supply)?
                .try_into()?
        };

        Decimal::from(amount)
            .try_mul(exchange_rate)?
            .try_floor_u64()
    }
    ///
    pub fn exchange_collateral_to_liquidity(&self, amount: u64) -> Result<u64, ProgramError> {
        let exchange_rate: Rate = self.liquidity_info
            .total_supply()?
            .try_sub(self.liquidity_info.insurance_wads)?
            .try_div(Decimal::from(self.collateral_info.total_mint))?
            .try_into()?;

        Decimal::from(amount)
            .try_mul(exchange_rate)?
            .try_floor_u64()
    }
    /// 
    // compounded_interest_rate: c
    // borrowed_amount_wads: m
    // fee rate: k
    // -----------------------------------------------------------------
    // d_m = m * (c-1)
    // d_fee = k * d_m = [k(c-1)] * m
    // m = m + d_m
    // fee = fee + d_fee
    // -----------------------------------------------------------------
    pub fn accrue_interest(&mut self, borrow_rate: Rate, slot: Slot) -> ProgramResult {
        let elapsed = self.last_update.slots_elapsed(slot)?;
        if elapsed > 0 {
            let compounded_interest_rate = Rate::one()
                .try_add(borrow_rate)?
                .try_pow(elapsed)?;
            let fee_interest_rate = compounded_interest_rate
                .try_sub(Rate::one())?
                .try_mul(Rate::from_percent(self.liquidity_info.config.borrow_tax_rate))?;
            let insurance_wads = self.liquidity_info.borrowed_amount_wads.try_mul(fee_interest_rate)?;

            self.liquidity_info.insurance_wads = self.liquidity_info.insurance_wads.try_add(insurance_wads)?;
            self.liquidity_info.acc_borrow_rate_wads = self.liquidity_info.acc_borrow_rate_wads.try_mul(compounded_interest_rate)?;
            self.liquidity_info.borrowed_amount_wads = self.liquidity_info.borrowed_amount_wads.try_mul(compounded_interest_rate)?;
        }

        Ok(())
    }
    ///
    pub fn deposit(&mut self, amount: u64) -> Result<u64, ProgramError> {
        let mint_amount = self.exchange_liquidity_to_collateral(amount)?;
        self.collateral_info.mint(mint_amount)?;
        self.liquidity_info.deposit(amount)?;

        Ok(mint_amount)
    }
    ///
    pub fn withdraw(&mut self, burn_amount: u64) -> Result<u64, ProgramError> {
        let amount = self.exchange_collateral_to_liquidity(burn_amount)?;
        self.collateral_info.burn(burn_amount)?;
        self.liquidity_info.withdraw(amount)?;

        Ok(amount)
    }
}

impl Sealed for MarketReserve {}
impl IsInitialized for MarketReserve {
    fn is_initialized(&self) -> bool {
        self.version != UNINITIALIZED_VERSION
    }
}

const MARKET_RESERVE_PADDING_LEN: usize = 128;
const MARKET_RESERVE_LEN: usize = 449;

impl Pack for MarketReserve {
    const LEN: usize = MARKET_RESERVE_LEN;

    fn pack_into_slice(&self, output: &mut [u8]) {
        let output = array_mut_ref![output, 0, MARKET_RESERVE_LEN];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            version,
            last_update,
            manager,
            market_price,
            mint_pubkey,
            account,
            price_oracle,
            decimal,
            sotoken_mint_pubkey,
            total_mint,
            borrow_value_ratio,
            liquidation_value_ratio,
            liquidation_bonus_ratio,
            enable,
            rate_oracle,
            available,
            flash_loan_fee,
            acc_borrow_rate_wads,
            borrowed_amount_wads,
            insurance_wads,
            close_factor,
            borrow_tax_rate,
            flash_loan_fee_rate,
            max_deposit,
            max_acc_deposit,
            _padding,
        ) = mut_array_refs![
            output,
            1,
            LAST_UPDATE_LEN,
            PUBKEY_BYTES,
            16,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            1,
            PUBKEY_BYTES,
            8,
            1,
            1,
            1,
            1,
            PUBKEY_BYTES,
            8,
            8,
            16,
            16,
            16,
            1,
            1,
            8,
            8,
            8,
            MARKET_RESERVE_PADDING_LEN
        ];

        *version = self.version.to_le_bytes();
        self.last_update.pack_into_slice(&mut last_update[..]);
        manager.copy_from_slice(self.manager.as_ref());
        pack_decimal(self.market_price, market_price);

        mint_pubkey.copy_from_slice(self.token_info.mint_pubkey.as_ref());
        account.copy_from_slice(self.token_info.account.as_ref());
        price_oracle.copy_from_slice(self.token_info.price_oracle.as_ref());
        *decimal = self.token_info.decimal.to_le_bytes();

        sotoken_mint_pubkey.copy_from_slice(self.collateral_info.sotoken_mint_pubkey.as_ref());
        *total_mint = self.collateral_info.total_mint.to_le_bytes();

        *borrow_value_ratio = self.collateral_info.config.borrow_value_ratio.to_le_bytes();
        *liquidation_value_ratio = self.collateral_info.config.liquidation_value_ratio.to_le_bytes();
        *liquidation_bonus_ratio = self.collateral_info.config.liquidation_bonus_ratio.to_le_bytes();

        pack_bool(self.liquidity_info.enable, enable);
        rate_oracle.copy_from_slice(self.liquidity_info.rate_oracle.as_ref());
        *available = self.liquidity_info.available.to_le_bytes();
        *flash_loan_fee = self.liquidity_info.flash_loan_fee.to_le_bytes();
        pack_decimal(self.liquidity_info.acc_borrow_rate_wads, acc_borrow_rate_wads);
        pack_decimal(self.liquidity_info.borrowed_amount_wads, borrowed_amount_wads);
        pack_decimal(self.liquidity_info.insurance_wads, insurance_wads);

        *close_factor = self.liquidity_info.config.close_factor.to_le_bytes();
        *borrow_tax_rate = self.liquidity_info.config.borrow_tax_rate.to_le_bytes();
        *flash_loan_fee_rate = self.liquidity_info.config.flash_loan_fee_rate.to_le_bytes();
        *max_deposit = self.liquidity_info.config.max_deposit.to_le_bytes();
        *max_acc_deposit = self.liquidity_info.config.max_acc_deposit.to_le_bytes();
    }

    fn unpack_from_slice(input: &[u8]) -> Result<Self, ProgramError> {
        let input = array_ref![input, 0, MARKET_RESERVE_LEN];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            version,
            last_update,
            manager,
            market_price,
            mint_pubkey,
            account,
            price_oracle,
            decimal,
            sotoken_mint_pubkey,
            total_mint,
            borrow_value_ratio,
            liquidation_value_ratio,
            liquidation_bonus_ratio,
            enable,
            rate_oracle,
            available,
            flash_loan_fee,
            acc_borrow_rate_wads,
            borrowed_amount_wads,
            insurance_wads,
            close_factor,
            borrow_tax_rate,
            flash_loan_fee_rate,
            max_deposit,
            max_acc_deposit,
            _padding,
        ) = array_refs![
            input,
            1,
            LAST_UPDATE_LEN,
            PUBKEY_BYTES,
            16,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            1,
            PUBKEY_BYTES,
            8,
            1,
            1,
            1,
            1,
            PUBKEY_BYTES,
            8,
            8,
            16,
            16,
            16,
            1,
            1,
            8,
            8,
            8,
            MARKET_RESERVE_PADDING_LEN
        ];

        let version = u8::from_le_bytes(*version);
        if version > PROGRAM_VERSION {
            msg!("MarketReserve version does not match lending program version");
            return Err(ProgramError::InvalidAccountData);
        }

        Ok(Self {
            version,
            last_update: LastUpdate::unpack_from_slice(&last_update[..])?,
            manager: Pubkey::new_from_array(*manager),
            market_price: unpack_decimal(market_price),
            token_info: TokenInfo {
                mint_pubkey: Pubkey::new_from_array(*mint_pubkey),
                account: Pubkey::new_from_array(*account),
                price_oracle: Pubkey::new_from_array(*price_oracle),
                decimal: u8::from_le_bytes(*decimal),
            },
            collateral_info: CollateralInfo {
                sotoken_mint_pubkey: Pubkey::new_from_array(*sotoken_mint_pubkey),
                total_mint: u64::from_le_bytes(*total_mint),
                config: CollateralConfig {
                    borrow_value_ratio: u8::from_le_bytes(*borrow_value_ratio),
                    liquidation_value_ratio: u8::from_le_bytes(*liquidation_value_ratio),
                    liquidation_bonus_ratio: u8::from_le_bytes(*liquidation_bonus_ratio),
                },
            },
            liquidity_info: LiquidityInfo {
                enable: unpack_bool(enable)?,
                rate_oracle: Pubkey::new_from_array(*rate_oracle),
                available: u64::from_le_bytes(*available),
                flash_loan_fee: u64::from_le_bytes(*flash_loan_fee),
                acc_borrow_rate_wads: unpack_decimal(acc_borrow_rate_wads),
                borrowed_amount_wads: unpack_decimal(borrowed_amount_wads),
                insurance_wads: unpack_decimal(insurance_wads),
                config: LiquidityConfig {
                    close_factor: u8::from_le_bytes(*close_factor),
                    borrow_tax_rate: u8::from_le_bytes(*borrow_tax_rate),
                    flash_loan_fee_rate: u64::from_le_bytes(*flash_loan_fee_rate),
                    max_deposit: u64::from_le_bytes(*max_deposit),
                    max_acc_deposit: u64::from_le_bytes(*max_acc_deposit),
                },
            },
        })
    }
}

/// All Operations due MarketReserve
impl<P: Any + Param + Copy> Operator<P> for MarketReserve {
    fn operate_unchecked(&mut self, param: P) -> ProgramResult {
        if let Some(control) = <dyn Any>::downcast_ref::<LiquidityControl>(&param) {
            self.liquidity_info.enable = control.0;
            return Ok(())
        }

        if let Some(config) = <dyn Any>::downcast_ref::<CollateralConfig>(&param) {
            self.collateral_info.config = *config;
            return Ok(());
        }

        if let Some(config) = <dyn Any>::downcast_ref::<LiquidityConfig>(&param) {
            self.liquidity_info.config = *config;
            return Ok(());
        }

        if let Some(oracle) = <dyn Any>::downcast_ref::<ReservePriceOracle>(&param) {
            self.token_info.price_oracle = oracle.0;
            return Ok(());
        }

        if let Some(oracle) = <dyn Any>::downcast_ref::<ReserveRateOracle>(&param) {
            self.liquidity_info.rate_oracle = oracle.0;
            return Ok(());
        }

        panic!("unexpected param type");
    }
}

///
#[derive(Clone, Debug, Copy)]
pub struct LiquidityControl(pub bool);

impl Param for LiquidityControl {
    fn assert_valid(&self) -> ProgramResult {
        Ok(())
    }
}

///
#[derive(Clone, Debug, Copy)]
pub struct ReservePriceOracle(pub Pubkey);

impl Param for ReservePriceOracle {
    fn assert_valid(&self) -> ProgramResult {
        Ok(())
    }
}

///
#[derive(Clone, Debug, Copy)]
pub struct ReserveRateOracle(pub Pubkey);

impl Param for ReserveRateOracle {
    fn assert_valid(&self) -> ProgramResult {
        Ok(())
    }
}