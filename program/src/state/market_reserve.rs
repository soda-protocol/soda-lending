#![allow(missing_docs)]
use super::*;
use crate::{
    error::LendingError,
    math::{Rate, TryDiv, TrySub, WAD},
    oracle::{OracleInfo, OracleConfig, OracleType},
};
use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use solana_program::{
    clock::Slot, 
    entrypoint::ProgramResult, 
    msg, 
    program_error::ProgramError,
    program_pack::{IsInitialized, Pack, Sealed}, 
    pubkey::{Pubkey, PUBKEY_BYTES}
};
use std::{convert::TryInto, any::Any};

///
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TokenConfig {
    ///
    pub mint_pubkey: Pubkey,
    ///
    pub supply_account: Pubkey,
    ///
    pub decimal: u8,
}

impl Param for TokenConfig {
    fn assert_valid(&self) -> ProgramResult {
        Ok(())
    }
}

///
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CollateralConfig {
    ///
    pub borrow_value_ratio: u8,
    ///
    pub liquidation_value_ratio: u8,
    ///
    pub liquidation_penalty_ratio: u8,
}

impl Param for CollateralConfig {
    fn assert_valid(&self) -> ProgramResult {
        if self.borrow_value_ratio > 0 &&
            self.liquidation_penalty_ratio > 0 &&
            self.borrow_value_ratio < self.liquidation_value_ratio &&
            self.liquidation_value_ratio < 100 && 
            self.liquidation_penalty_ratio < 100 {
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
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LiquidityConfig {
    ///
    pub close_ratio: u8,
    ///
    pub borrow_tax_rate: u8,
    ///
    pub flash_loan_fee_rate: u64,
    ///
    pub max_deposit: u64,
}

impl Param for LiquidityConfig {
    fn assert_valid(&self) -> ProgramResult {
        if self.close_ratio > 0 &&
            self.borrow_tax_rate > 0 &&
            self.flash_loan_fee_rate > 0 &&
            self.max_deposit > 0 &&
            self.close_ratio < 100 &&
            self.borrow_tax_rate < 100 &&
            self.flash_loan_fee_rate < WAD {
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

        self.available = self.available
            .checked_add(amount)
            .ok_or(LendingError::MathOverflow)?;

        if self.total_supply()? <= Decimal::from(self.config.max_deposit) {
            Ok(())
        } else {
            Err(LendingError::MarketReserveDepositTooMuch.into())
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
    pub fn repay(&mut self, settle: &RepaySettle) -> ProgramResult {
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
            self.flash_loan_fee = self.flash_loan_fee
                .checked_sub(amount)
                .ok_or(LendingError::MathOverflow)?;
        } else {
            let amount = amount - self.flash_loan_fee;
            self.flash_loan_fee = 0;
            self.insurance_wads = self.insurance_wads.try_sub(Decimal::from(amount))?;
        }
        
        Ok(())
    }
}

/// Lending market reserve state
#[derive(Clone, Debug, PartialEq)]
pub struct MarketReserve {
    /// Version of the struct
    pub version: u8,
    ///
    pub last_update: LastUpdate,
    /// 
    pub manager: Pubkey,
    ///
    pub token_config: TokenConfig,
    ///
    pub oracle_info: OracleInfo,
    ///
    pub collateral_info: CollateralInfo,
    ///
    pub liquidity_info: LiquidityInfo,
    ///
    pub rate_model: RateModel,
}

impl MarketReserve {
    ///
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        slot: Slot,
        manager: Pubkey,
        token_config: TokenConfig,
        oracle_config: OracleConfig,
        liquidity_config: LiquidityConfig,
        sotoken_mint_pubkey: Pubkey,
        collateral_config: CollateralConfig,
        rate_model: RateModel,
    ) -> Self {
        Self {
            version: PROGRAM_VERSION,
            last_update: LastUpdate::new(slot),
            manager,
            token_config,
            oracle_info: OracleInfo {
                price: Decimal::default(),
                config: oracle_config,
            },
            liquidity_info: LiquidityInfo {
                enable: true,
                available: 0,
                flash_loan_fee: 0,
                acc_borrow_rate_wads: Decimal::one(),
                borrowed_amount_wads: Decimal::zero(),
                insurance_wads: Decimal::zero(),
                config: liquidity_config,
            },
            collateral_info: CollateralInfo {
                sotoken_mint_pubkey,
                total_mint: 0,
                config: collateral_config,
            },
            rate_model,
        }
    }
    ///
    pub fn liquidity_to_collateral_rate(&self) -> Result<Rate, ProgramError> {
        let total_supply = self.liquidity_info
            .total_supply()?
            .try_sub(self.liquidity_info.insurance_wads)?;
        if total_supply == Decimal::zero() {
            Ok(Rate::one())
        } else {
            Decimal::from(self.collateral_info.total_mint)
                .try_div(total_supply)?
                .try_into()
        }
    }
    ///
    pub fn collateral_to_liquidity_rate(&self) -> Result<Rate, ProgramError> {
        self.liquidity_info
            .total_supply()?
            .try_sub(self.liquidity_info.insurance_wads)?
            .try_div(Decimal::from(self.collateral_info.total_mint))?
            .try_into()
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
    pub fn accrue_interest(&mut self, slot: Slot) -> ProgramResult {
        let elapsed = self.last_update.slots_elapsed(slot)?;
        if elapsed > 0 {
            let compounded_interest_rate = Rate::one()
                .try_add(self.rate_model.calculate_borrow_rate(self.liquidity_info.utilization_rate()?)?)?
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
        let mint_amount = amount_mul_rate(amount, self.liquidity_to_collateral_rate()?)?;
        self.collateral_info.mint(mint_amount)?;
        self.liquidity_info.deposit(amount)?;

        Ok(mint_amount)
    }
    ///
    pub fn withdraw(&mut self, amount: u64) -> Result<u64, ProgramError> {
        let withdraw_amount = amount_mul_rate(amount, self.collateral_to_liquidity_rate()?)?;
        self.collateral_info.burn(amount)?;
        self.liquidity_info.withdraw(withdraw_amount)?;

        Ok(withdraw_amount)
    }
}

impl Sealed for MarketReserve {}
impl IsInitialized for MarketReserve {
    fn is_initialized(&self) -> bool {
        self.version != UNINITIALIZED_VERSION
    }
}

const MARKET_RESERVE_PADDING_LEN: usize = 256;
const MARKET_RESERVE_LEN: usize = 571;

impl Pack for MarketReserve {
    const LEN: usize = MARKET_RESERVE_LEN;

    fn pack_into_slice(&self, output: &mut [u8]) {
        let output = array_mut_ref![output, 0, MARKET_RESERVE_LEN];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            version,
            last_update,
            manager,
            mint_pubkey,
            supply_account,
            decimal,
            price,
            oracle,
            oracle_type,
            sotoken_mint_pubkey,
            total_mint,
            borrow_value_ratio,
            liquidation_value_ratio,
            liquidation_penalty_ratio,
            enable,
            available,
            flash_loan_fee,
            acc_borrow_rate_wads,
            borrowed_amount_wads,
            insurance_wads,
            close_ratio,
            borrow_tax_rate,
            flash_loan_fee_rate,
            max_deposit,
            offset,
            optimal,
            kink,
            max,
            _padding,
        ) = mut_array_refs![
            output,
            1,
            LAST_UPDATE_LEN,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            1,
            16,
            PUBKEY_BYTES,
            1,
            PUBKEY_BYTES,
            8,
            1,
            1,
            1,
            1,
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
            8,
            1,
            16,
            MARKET_RESERVE_PADDING_LEN
        ];

        *version = self.version.to_le_bytes();
        self.last_update.pack_into_slice(last_update);
        manager.copy_from_slice(self.manager.as_ref());

        mint_pubkey.copy_from_slice(self.token_config.mint_pubkey.as_ref());
        supply_account.copy_from_slice(self.token_config.supply_account.as_ref());
        *decimal = self.token_config.decimal.to_le_bytes();

        pack_decimal(self.oracle_info.price, price);
        oracle.copy_from_slice(self.oracle_info.config.oracle.as_ref());
        let oracle_type_u8: u8 = self.oracle_info.config.oracle_type.into();
        *oracle_type = oracle_type_u8.to_le_bytes();

        sotoken_mint_pubkey.copy_from_slice(self.collateral_info.sotoken_mint_pubkey.as_ref());
        *total_mint = self.collateral_info.total_mint.to_le_bytes();

        *borrow_value_ratio = self.collateral_info.config.borrow_value_ratio.to_le_bytes();
        *liquidation_value_ratio = self.collateral_info.config.liquidation_value_ratio.to_le_bytes();
        *liquidation_penalty_ratio = self.collateral_info.config.liquidation_penalty_ratio.to_le_bytes();

        pack_bool(self.liquidity_info.enable, enable);
        *available = self.liquidity_info.available.to_le_bytes();
        *flash_loan_fee = self.liquidity_info.flash_loan_fee.to_le_bytes();
        pack_decimal(self.liquidity_info.acc_borrow_rate_wads, acc_borrow_rate_wads);
        pack_decimal(self.liquidity_info.borrowed_amount_wads, borrowed_amount_wads);
        pack_decimal(self.liquidity_info.insurance_wads, insurance_wads);

        *close_ratio = self.liquidity_info.config.close_ratio.to_le_bytes();
        *borrow_tax_rate = self.liquidity_info.config.borrow_tax_rate.to_le_bytes();
        *flash_loan_fee_rate = self.liquidity_info.config.flash_loan_fee_rate.to_le_bytes();
        *max_deposit = self.liquidity_info.config.max_deposit.to_le_bytes();

        *offset = self.rate_model.offset.to_le_bytes();
        *optimal = self.rate_model.optimal.to_le_bytes();
        *kink = self.rate_model.kink.to_le_bytes();
        *max = self.rate_model.max.to_le_bytes();
    }

    fn unpack_from_slice(input: &[u8]) -> Result<Self, ProgramError> {
        let input = array_ref![input, 0, MARKET_RESERVE_LEN];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            version,
            last_update,
            manager,
            mint_pubkey,
            supply_account,
            decimal,
            price,
            oracle,
            oracle_type,
            sotoken_mint_pubkey,
            total_mint,
            borrow_value_ratio,
            liquidation_value_ratio,
            liquidation_penalty_ratio,
            enable,
            available,
            flash_loan_fee,
            acc_borrow_rate_wads,
            borrowed_amount_wads,
            insurance_wads,
            close_ratio,
            borrow_tax_rate,
            flash_loan_fee_rate,
            max_deposit,
            offset,
            optimal,
            kink,
            max,
            _padding,
        ) = array_refs![
            input,
            1,
            LAST_UPDATE_LEN,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            1,
            16,
            PUBKEY_BYTES,
            1,
            PUBKEY_BYTES,
            8,
            1,
            1,
            1,
            1,
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
            8,
            1,
            16,
            MARKET_RESERVE_PADDING_LEN
        ];

        let version = u8::from_le_bytes(*version);
        if version > PROGRAM_VERSION {
            msg!("MarketReserve version does not match lending program version");
            return Err(ProgramError::InvalidAccountData);
        }

        Ok(Self {
            version,
            last_update: LastUpdate::unpack_from_slice(last_update)?,
            manager: Pubkey::new_from_array(*manager),
            token_config: TokenConfig {
                mint_pubkey: Pubkey::new_from_array(*mint_pubkey),
                supply_account: Pubkey::new_from_array(*supply_account),
                decimal: u8::from_le_bytes(*decimal),
            },
            oracle_info: OracleInfo {
                price: unpack_decimal(price),
                config: OracleConfig {
                    oracle: Pubkey::new_from_array(*oracle),
                    oracle_type: OracleType::from(u8::from_le_bytes(*oracle_type)),
                },
            },
            collateral_info: CollateralInfo {
                sotoken_mint_pubkey: Pubkey::new_from_array(*sotoken_mint_pubkey),
                total_mint: u64::from_le_bytes(*total_mint),
                config: CollateralConfig {
                    borrow_value_ratio: u8::from_le_bytes(*borrow_value_ratio),
                    liquidation_value_ratio: u8::from_le_bytes(*liquidation_value_ratio),
                    liquidation_penalty_ratio: u8::from_le_bytes(*liquidation_penalty_ratio),
                },
            },
            liquidity_info: LiquidityInfo {
                enable: unpack_bool(enable)?,
                available: u64::from_le_bytes(*available),
                flash_loan_fee: u64::from_le_bytes(*flash_loan_fee),
                acc_borrow_rate_wads: unpack_decimal(acc_borrow_rate_wads),
                borrowed_amount_wads: unpack_decimal(borrowed_amount_wads),
                insurance_wads: unpack_decimal(insurance_wads),
                config: LiquidityConfig {
                    close_ratio: u8::from_le_bytes(*close_ratio),
                    borrow_tax_rate: u8::from_le_bytes(*borrow_tax_rate),
                    flash_loan_fee_rate: u64::from_le_bytes(*flash_loan_fee_rate),
                    max_deposit: u64::from_le_bytes(*max_deposit),
                },
            },
            rate_model: RateModel {
                offset: u64::from_le_bytes(*offset),
                optimal: u64::from_le_bytes(*optimal),
                kink: u8::from_le_bytes(*kink),
                max: u128::from_le_bytes(*max),
            }
        })
    }
}

/// All Operations due MarketReserve
impl<P: Any + Param> Operator<P> for MarketReserve {
    fn operate_unchecked(&mut self, param: P) -> ProgramResult {
        if let Some(control) = <dyn Any>::downcast_ref::<LiquidityControl>(&param) {
            self.liquidity_info.enable = control.0;
            return Ok(())
        }

        if let Some(config) = <dyn Any>::downcast_ref::<TokenConfig>(&param) {
            self.token_config = *config;
            return Ok(());
        }

        if let Some(config) = <dyn Any>::downcast_ref::<CollateralConfig>(&param) {
            self.collateral_info.config = *config;
            return Ok(());
        }

        if let Some(config) = <dyn Any>::downcast_ref::<LiquidityConfig>(&param) {
            self.liquidity_info.config = *config;
            return Ok(());
        }

        if let Some(model) = <dyn Any>::downcast_ref::<RateModel>(&param) {
            self.rate_model = *model;
            return Ok(());
        }

        if let Some(config) = <dyn Any>::downcast_ref::<OracleConfig>(&param) {
            self.oracle_info.config = *config;
            return Ok(());
        }

        panic!("unexpected param type {}", std::any::type_name::<P>());
    }
}

///
#[derive(Clone, Debug)]
pub struct LiquidityControl(pub bool);

impl Param for LiquidityControl {
    fn assert_valid(&self) -> ProgramResult {
        Ok(())
    }
}
