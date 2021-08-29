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
    pub close_factor: u8,
}

impl Param for CollateralConfig {
    fn is_valid(&self) -> ProgramResult {
        if self.borrow_value_ratio < self.liquidation_value_ratio &&
            self.liquidation_value_ratio < 100 &&
            self.close_factor < 100 {
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
    pub borrow_fee_rate: u64,
    ///
    pub liquidation_fee_rate: u64,
    ///
    pub flash_loan_fee_rate: u64,
}

impl Param for LiquidityConfig {
    fn is_valid(&self) -> ProgramResult {
        if self.borrow_fee_rate < WAD &&
            self.liquidation_fee_rate < WAD &&
            self.flash_loan_fee_rate < WAD {
            Ok(())
        } else {
            Err(LendingError::InvalidLiquidityConfig.into())
        }
    }
}

///
#[derive(Clone, Copy, Debug)]
pub struct EnableBorrow();

impl Param for EnableBorrow {
    fn is_valid(&self) -> ProgramResult {
        Ok(())
    }
}

///
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LiquidityInfo {
    ///
    pub enable_borrow: bool,
    ///
    pub rate_oracle: Pubkey,
    ///
    pub available: u64,
    ///
    pub borrowed_amount_wads: Decimal,
    ///
    pub acc_borrow_rate_wads: Decimal,
    ///
    pub fee: u64,
    ///
    pub config: LiquidityConfig,
}

impl LiquidityInfo {
    ///
    pub fn total_amount(&self) -> Result<Decimal, ProgramError> {
        Decimal::from(self.available).try_add(self.borrowed_amount_wads)
    }
    ///
    pub fn utilization_rate(&self) -> Result<Rate, ProgramError> {
        let total_amount = self.total_amount()?;
        if total_amount == Decimal::zero() {
            Ok(Rate::zero())
        } else {
            self.borrowed_amount_wads
                .try_div(self.total_amount()?)?
                .try_into()
        }
    }
    ///
    pub fn deposit(&mut self, amount: u64) -> ProgramResult {
        self.available = self.available
            .checked_add(amount)
            .ok_or(LendingError::MathOverflow)?;

        Ok(())
    }
    ///
    pub fn withdraw(&mut self, amount: u64) -> ProgramResult {
        self.available = self.available
            .checked_sub(amount)
            .ok_or(LendingError::MarketReserveLiquidityAvailableInsufficent)?;

        Ok(())
    }
    ///
    pub fn borrow_out(&mut self, asset: BorrowWithFee) -> ProgramResult {
        if !self.enable_borrow {
            return Err(LendingError::MarketReserveBorrowDisabled.into());
        }

        self.available = self.available
            .checked_sub(asset.amount)
            .ok_or(LendingError::MarketReserveLiquidityAvailableInsufficent)?;
        self.borrowed_amount_wads = self.borrowed_amount_wads.try_add(Decimal::from(asset.amount))?;

        self.fee = self.fee
            .checked_add(asset.fee)
            .ok_or(LendingError::MathOverflow)?;

        Ok(())
    }
    ///
    pub fn repay(&mut self, amount: u64) -> ProgramResult {
        self.available = self.available
            .checked_add(amount)
            .ok_or(LendingError::MathOverflow)?;
        self.borrowed_amount_wads = self.borrowed_amount_wads.try_sub(Decimal::from(amount))?;

        Ok(())
    }
    ///
    pub fn liquidate(&mut self, asset: LiquidationWithFee) -> ProgramResult {
        self.available = self.available
            .checked_add(asset.repay)
            .ok_or(LendingError::MathOverflow)?;
        self.borrowed_amount_wads = self.borrowed_amount_wads.try_sub(Decimal::from(asset.repay))?;
    
        self.fee = self.fee
            .checked_add(asset.fee)
            .ok_or(LendingError::MathOverflow)?;
        
        Ok(())
    }
    ///
    pub fn withdraw_fee(&mut self, fee: u64) -> ProgramResult {
        self.fee = self.fee
            .checked_sub(fee)
            .ok_or(LendingError::MarketReserveLiquidityFeeInsufficent)?;
        
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

impl<P: Any + Param + Copy> Operator<P> for MarketReserve {
    fn operate(&mut self, param: P) -> ProgramResult {
        if let Some(_enable) = <dyn Any>::downcast_ref::<EnableBorrow>(&param) {
            self.liquidity_info.enable_borrow = true;
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

        panic!("unexpected param type");
    }
}

impl MarketReserve {
    ///
    pub fn calculate_liquidity_to_collateral(&self, amount: u64) -> Result<u64, ProgramError> {
        let total_amount = self.liquidity_info.total_amount()?;
        if total_amount == Decimal::zero() {
            Ok(amount)
        } else {
            let total_mint = Decimal::from(self.collateral_info.total_mint);
            total_amount
                .try_add(Decimal::from(amount))?
                .try_mul(total_mint)?
                .try_div(total_amount)?
                .try_sub(total_mint)?
                .try_floor_u64()
        }
    }
    ///
    pub fn calculate_collateral_to_liquidity(&self, amount: u64) -> Result<u64, ProgramError> {
        let total_amount = self.liquidity_info.total_amount()?;
        if total_amount == Decimal::zero() {
            Err(LendingError::MarketReserveLiquidityEmpty.into())
        } else {
            let total_mint = Decimal::from(self.collateral_info.total_mint);
            let left_amount = total_mint
                .try_sub(Decimal::from(amount))?
                .try_mul(total_amount)?
                .try_div(total_mint)?;

            total_amount
                .try_sub(left_amount)?
                .try_floor_u64()
        }
    }
    ///
    pub fn accrue_interest(&mut self, borrow_rate: Rate, slot: Slot) -> ProgramResult {
        let elapsed = self.last_update.slots_elapsed(slot)?;
        if elapsed > 0 {
            let compounded_interest_rate = Rate::one()
                .try_add(borrow_rate)?
                .try_pow(elapsed)?;

            self.liquidity_info.acc_borrow_rate_wads = self.liquidity_info.acc_borrow_rate_wads.try_mul(compounded_interest_rate)?;
            self.liquidity_info.borrowed_amount_wads = self.liquidity_info.borrowed_amount_wads.try_mul(compounded_interest_rate)?;
        }

        Ok(())
    }
    ///
    pub fn exchange_liquidity_to_collateral(&mut self, amount: u64) -> Result<u64, ProgramError> {
        let mint_amount = self.calculate_liquidity_to_collateral(amount)?;
        self.collateral_info.mint(mint_amount)?;
        self.liquidity_info.deposit(amount)?;

        Ok(mint_amount)
    }
    ///
    pub fn exchange_collateral_to_liquidity(&mut self, amount: u64) -> Result<u64, ProgramError> {
        let mint_amount = self.calculate_collateral_to_liquidity(amount)?;
        self.collateral_info.burn(mint_amount)?;
        self.liquidity_info.withdraw(amount)?;

        Ok(mint_amount)
    }
}

impl Sealed for MarketReserve {}
impl IsInitialized for MarketReserve {
    fn is_initialized(&self) -> bool {
        self.version != UNINITIALIZED_VERSION
    }
}

const MARKET_RESERVE_RESERVE_LEN: usize = 128;
const MARKET_RESERVE_LEN: usize = 399;

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
            account,
            price_oracle,
            decimal,
            sotoken_mint_pubkey,
            total_mint,
            borrow_value_ratio,
            liquidation_value_ratio,
            close_factor,
            enable_borrow,
            rate_oracle,
            available,
            borrowed_amount_wads,
            acc_borrow_rate_wads,
            fee,
            borrow_fee_rate,
            liquidation_fee_rate,
            flash_loan_fee_rate,
            _rest,
        ) = mut_array_refs![
            output,
            1,
            LAST_UPDATE_LEN,
            PUBKEY_BYTES,
            16,
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
            16,
            16,
            8,
            8,
            8,
            8,
            MARKET_RESERVE_RESERVE_LEN
        ];

        *version = self.version.to_le_bytes();
        self.last_update.pack_into_slice(&mut last_update[..]);
        manager.copy_from_slice(self.manager.as_ref());
        pack_decimal(self.market_price, market_price);

        account.copy_from_slice(self.token_info.account.as_ref());
        price_oracle.copy_from_slice(self.token_info.price_oracle.as_ref());
        *decimal = self.token_info.decimal.to_le_bytes();

        pack_bool(self.liquidity_info.enable_borrow, enable_borrow);
        rate_oracle.copy_from_slice(self.liquidity_info.rate_oracle.as_ref());
        *available = self.liquidity_info.available.to_le_bytes();
        pack_decimal(self.liquidity_info.borrowed_amount_wads, borrowed_amount_wads);
        pack_decimal(self.liquidity_info.acc_borrow_rate_wads, acc_borrow_rate_wads);
        *fee = self.liquidity_info.fee.to_le_bytes();

        *borrow_fee_rate = self.liquidity_info.config.borrow_fee_rate.to_le_bytes();
        *liquidation_fee_rate = self.liquidity_info.config.liquidation_fee_rate.to_le_bytes();
        *flash_loan_fee_rate = self.liquidity_info.config.flash_loan_fee_rate.to_le_bytes();

        sotoken_mint_pubkey.copy_from_slice(self.collateral_info.sotoken_mint_pubkey.as_ref());
        *total_mint = self.collateral_info.total_mint.to_le_bytes();

        *borrow_value_ratio = self.collateral_info.config.borrow_value_ratio.to_le_bytes();
        *liquidation_value_ratio = self.collateral_info.config.liquidation_value_ratio.to_le_bytes();
        *close_factor = self.collateral_info.config.close_factor.to_le_bytes();
    }

    fn unpack_from_slice(input: &[u8]) -> Result<Self, ProgramError> {
        let input = array_ref![input, 0, MARKET_RESERVE_LEN];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            version,
            last_update,
            manager,
            market_price,
            account,
            price_oracle,
            decimal,
            sotoken_mint_pubkey,
            total_mint,
            borrow_value_ratio,
            liquidation_value_ratio,
            close_factor,
            enable_borrow,
            rate_oracle,
            available,
            borrowed_amount_wads,
            acc_borrow_rate_wads,
            fee,
            borrow_fee_rate,
            liquidation_fee_rate,
            flash_loan_fee_rate,
            _rest,
        ) = array_refs![
            input,
            1,
            LAST_UPDATE_LEN,
            PUBKEY_BYTES,
            16,
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
            16,
            16,
            8,
            8,
            8,
            8,
            MARKET_RESERVE_RESERVE_LEN
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
                    close_factor: u8::from_le_bytes(*close_factor),
                },
            },
            liquidity_info: LiquidityInfo {
                enable_borrow: unpack_bool(enable_borrow)?,
                rate_oracle: Pubkey::new_from_array(*rate_oracle),
                available: u64::from_le_bytes(*available),
                borrowed_amount_wads: unpack_decimal(borrowed_amount_wads),
                acc_borrow_rate_wads: unpack_decimal(acc_borrow_rate_wads),
                fee: u64::from_le_bytes(*fee),
                config: LiquidityConfig {
                    borrow_fee_rate: u64::from_le_bytes(*borrow_fee_rate),
                    liquidation_fee_rate: u64::from_le_bytes(*liquidation_fee_rate),
                    flash_loan_fee_rate: u64::from_le_bytes(*flash_loan_fee_rate)
                },
            },
        })
    }
}