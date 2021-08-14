#![allow(missing_docs)]
///
use std::convert::TryInto;
use super::*;
use crate::{
    error::LendingError,
    math::{Rate, TryDiv, WAD}
};
use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use solana_program::{
    clock::Slot, 
    entrypoint::ProgramResult, 
    msg, 
    program_error::ProgramError, 
    program_option::COption, 
    program_pack::{IsInitialized, Pack, Sealed}, 
    pubkey::{Pubkey, PUBKEY_BYTES}
};

///
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Liquidity {
    ///
    pub available: u64,
    ///
    pub borrowed: u64,
    ///
    pub interest: i128,
    ///
    pub fee: u64,
}

impl Liquidity {
    ///
    pub fn utilization_rate(&self) -> Result<Rate, ProgramError> {
        let total = self.available
            .checked_add(self.borrowed)
            .ok_or(LendingError::MathOverflow)?;

        Rate::one()
            .try_mul(self.borrowed)?
            .try_div(total)
    }
    ///
    pub fn deposit(&mut self, amount: u64) -> ProgramResult {
        self.available = self.available
            .checked_add(amount)
            .ok_or(LendingError::MathOverflow)?;

        Ok(())
    }
    ///
    pub fn borrow_out(&mut self, amount: u64) -> ProgramResult {
        self.available = self.available
            .checked_sub(amount)
            .ok_or(LendingError::MarketReserveLiquidityAvailableInsufficent)?;
        
        self.borrowed = self.borrowed
            .checked_add(amount)
            .ok_or(LendingError::MathOverflow)?;

        Ok(())
    }
    ///
    pub fn repay(&mut self, settle: &Settle) -> ProgramResult {
        self.available = self.available
            .checked_add(settle.total)
            .ok_or(LendingError::MathOverflow)?;
    
        self.interest = self.interest
            .checked_add(settle.interest as i128)
            .ok_or(LendingError::MathOverflow)?;

        Ok(())
    }
    ///
    pub fn liquidate(&mut self, settle: &Settle, fee: u64) -> ProgramResult {
        self.available = self.available
            .checked_add(settle.total)
            .ok_or(LendingError::MathOverflow)?;
    
        self.interest = self.interest
            .checked_add(settle.interest as i128)
            .ok_or(LendingError::MathOverflow)?;

        self.fee = self.fee
            .checked_add(fee)
            .ok_or(LendingError::MathOverflow)?;
        
        Ok(())
    }
    ///
    pub fn withdraw(&mut self, settle: &Settle, fee: u64) -> ProgramResult {
        self.interest = self.interest
            .checked_sub(settle.interest as i128)
            .ok_or(LendingError::MathOverflow)?;

        self.available = self.available
            .checked_sub(settle.total)
            .ok_or(LendingError::MarketReserveLiquidityAvailableInsufficent)?;

        self.fee = self.fee
            .checked_add(fee)
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

///
#[derive(Clone, Debug, Copy, Default, PartialEq)]
pub struct LiquidityConfig {
    ///
    pub interest_fee_rate: u64, 
    ///
    pub max_borrow_utilization_rate: u8,
}

impl LiquidityConfig {
    pub fn check_valid(&self) -> ProgramResult {
        if self.interest_fee_rate < WAD && self.max_borrow_utilization_rate < 100 {
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
    pub rate_oracle: Pubkey,
    ///
    pub liquidity: Liquidity,
    ///
    pub config: LiquidityConfig,
}

impl Sealed for LiquidityInfo {}

pub const MARKET_RESERVE_LIQUIDITY_INFO_LEN: usize = 81;

impl Pack for LiquidityInfo {
    const LEN: usize = MARKET_RESERVE_LIQUIDITY_INFO_LEN;

    fn pack_into_slice(&self, output: &mut [u8]) {
        let output = array_mut_ref![output, 0, MARKET_RESERVE_LIQUIDITY_INFO_LEN];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            rate_oracle,
            available,
            borrowed,
            interest,
            fee,
            interest_fee_rate,
            max_borrow_utilization_rate,
        ) = mut_array_refs![
            output,
            PUBKEY_BYTES,
            8,
            8,
            16,
            8,
            8,
            1
        ];

        rate_oracle.copy_from_slice(self.rate_oracle.as_ref());
        *available = self.liquidity.available.to_le_bytes();
        *borrowed = self.liquidity.borrowed.to_le_bytes();
        *interest = self.liquidity.interest.to_le_bytes();
        *fee = self.liquidity.fee.to_le_bytes();
        *interest_fee_rate = self.config.interest_fee_rate.to_le_bytes();
        *max_borrow_utilization_rate = self.config.max_borrow_utilization_rate.to_le_bytes();
    }

    fn unpack_from_slice(input: &[u8]) -> Result<Self, ProgramError> {
        let input = array_ref![input, 0, MARKET_RESERVE_LIQUIDITY_INFO_LEN];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            rate_oracle,
            available,
            borrowed,
            interest,
            fee,
            interest_fee_rate,
            max_borrow_utilization_rate,
        ) = array_refs![
            input,
            PUBKEY_BYTES,
            8,
            8,
            16,
            8,
            8,
            1
        ];

        Ok(Self{
            rate_oracle: Pubkey::new_from_array(*rate_oracle),
            liquidity: Liquidity{
                available: u64::from_le_bytes(*available),
                borrowed: u64::from_le_bytes(*borrowed),
                interest: i128::from_le_bytes(*interest),
                fee: u64::from_le_bytes(*fee),
            },
            config: LiquidityConfig {
                interest_fee_rate: u64::from_le_bytes(*interest_fee_rate),
                max_borrow_utilization_rate: u8::from_le_bytes(*max_borrow_utilization_rate),
            },
        })
    }
}

impl LiquidityInfo {
    pub fn borrow_out(&mut self, amount: u64) -> ProgramResult {
        self.check_utilization()?;
        self.liquidity.borrow_out(amount)?;
        self.check_utilization()
    }

    fn check_utilization(&self) -> ProgramResult {
        let utilization_rate = self.liquidity.utilization_rate()?;
        if utilization_rate >= Rate::from_percent(self.config.max_borrow_utilization_rate) {
            Err(LendingError::MarketReserveLiquidityUtilizationTooLarge.into())
        } else {
            Ok(())
        }
    }
}

///
#[derive(Clone, Debug, Copy, Default, PartialEq)]
pub struct CollateralConfig {
    ///
    pub liquidate_fee_rate: u64,
    ///
    pub arbitrary_liquidate_rate: u64,
    ///
    pub liquidate_limit: u8,
    ///
    pub effective_value_rate: u8,
    ///
    pub close_factor: u8,
}

impl CollateralConfig {
    pub fn check_valid(&self) -> ProgramResult {
        if self.liquidate_fee_rate < WAD &&
            self.arbitrary_liquidate_rate < WAD &&
            self.liquidate_limit < 100 &&
            self.effective_value_rate < self.liquidate_limit &&
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
    pub amount: u64,
    ///
    pub config: CollateralConfig,
}

impl Sealed for CollateralInfo {}

const MARKET_RESERVE_COLLATERAL_INFO_LEN: usize = 27;

impl Pack for CollateralInfo {
    const LEN: usize = MARKET_RESERVE_COLLATERAL_INFO_LEN;

    fn pack_into_slice(&self, output: &mut [u8]) {
        let output = array_mut_ref![output, 0, MARKET_RESERVE_COLLATERAL_INFO_LEN];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            amount,
            liquidate_fee_rate,
            arbitrary_liquidate_rate,
            liquidate_limit,
            effective_value_rate,
            close_factor,
        ) = mut_array_refs![
            output,
            8,
            8,
            8,
            1,
            1,
            1
        ];

        *amount = self.amount.to_le_bytes();
        *liquidate_fee_rate = self.config.liquidate_fee_rate.to_le_bytes();
        *arbitrary_liquidate_rate = self.config.arbitrary_liquidate_rate.to_le_bytes();
        *liquidate_limit = self.config.liquidate_limit.to_le_bytes();
        *effective_value_rate = self.config.effective_value_rate.to_le_bytes();
        *close_factor = self.config.close_factor.to_le_bytes();
    }

    fn unpack_from_slice(input: &[u8]) -> Result<Self, ProgramError> {
        let input = array_ref![input, 0, MARKET_RESERVE_COLLATERAL_INFO_LEN];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            amount,
            liquidate_fee_rate,
            arbitrary_liquidate_rate,
            liquidate_limit,
            effective_value_rate,
            close_factor,
        ) = array_refs![
            input,
            8,
            8,
            8,
            1,
            1,
            1
        ];

        Ok(Self{
            amount: u64::from_le_bytes(*amount),
            config: CollateralConfig {
                liquidate_fee_rate: u64::from_le_bytes(*liquidate_fee_rate),
                arbitrary_liquidate_rate: u64::from_le_bytes(*arbitrary_liquidate_rate),
                liquidate_limit: u8::from_le_bytes(*liquidate_limit),
                effective_value_rate: u8::from_le_bytes(*effective_value_rate),
                close_factor: u8::from_le_bytes(*close_factor),
            },
        })
    }
}

impl CollateralInfo {
    ///
    pub fn deposit(&mut self, amount: u64) -> ProgramResult {
        self.amount = self.amount
            .checked_add(amount)
            .ok_or(LendingError::MathOverflow)?;
        
        Ok(())
    }
    ///
    pub fn redeem(&mut self, amount: u64) -> ProgramResult {
        self.amount = self.amount
            .checked_sub(amount)
            .ok_or(LendingError::MarketReserveCollateralInsufficent)?;
    
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
    pub token_info: TokenInfo,
    ///
    pub liquidity_info: COption<LiquidityInfo>,
    ///
    pub collateral_info: CollateralInfo,
}

impl Sealed for MarketReserve {}
impl IsInitialized for MarketReserve {
    fn is_initialized(&self) -> bool {
        self.version != UNINITIALIZED_VERSION
    }
}

const MARKET_RESERVE_LEN: usize = 219;

impl Pack for MarketReserve {
    const LEN: usize = MARKET_RESERVE_LEN;

    fn pack_into_slice(&self, output: &mut [u8]) {
        let output = array_mut_ref![output, 0, MARKET_RESERVE_LEN];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            version,
            last_update,
            manager,
            token_info,
            liquidity_info,
            collateral_info,
        ) = mut_array_refs![
            output,
            1,
            LAST_UPDATE_LEN,
            PUBKEY_BYTES,
            TOKEN_INFO_LEN,
            COPTION_LEN + MARKET_RESERVE_LIQUIDITY_INFO_LEN,
            MARKET_RESERVE_COLLATERAL_INFO_LEN
        ];

        *version = self.version.to_le_bytes();
        self.last_update.pack_into_slice(&mut last_update[..]);
        manager.copy_from_slice(self.manager.as_ref());
        self.token_info.pack_into_slice(&mut token_info[..]);
        pack_coption_struct(&self.liquidity_info, &mut liquidity_info[..]);
        self.collateral_info.pack_into_slice(&mut collateral_info[..]);
    }

    fn unpack_from_slice(input: &[u8]) -> Result<Self, ProgramError> {
        let input = array_ref![input, 0, MARKET_RESERVE_LEN];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            version,
            last_update,
            manager,
            token_info,
            liquidity_info,
            collateral_info,
        ) = array_refs![
            input,
            1,
            LAST_UPDATE_LEN,
            PUBKEY_BYTES,
            TOKEN_INFO_LEN,
            COPTION_LEN + MARKET_RESERVE_LIQUIDITY_INFO_LEN,
            MARKET_RESERVE_COLLATERAL_INFO_LEN
        ];

        let version = u8::from_le_bytes(*version);
        if version > PROGRAM_VERSION {
            msg!("MarketReserve version does not match lending program version");
            return Err(ProgramError::InvalidAccountData);
        }

        Ok(Self{
            version,
            last_update: LastUpdate::unpack_from_slice(&last_update[..])?,
            manager: Pubkey::new_from_array(*manager),
            token_info: TokenInfo::unpack_from_slice(&token_info[..])?,
            liquidity_info: unpack_coption_struct::<LiquidityInfo>(&liquidity_info[..])?,
            collateral_info: CollateralInfo::unpack_from_slice(&collateral_info[..])?,
        })
    }
}

impl MarketReserve {
    pub fn check_valid(&mut self, slot: Slot) -> ProgramResult {
        if self.last_update.is_stale(slot)? {
            Err(LendingError::MarketReserveStale.into())
        } else {
            self.last_update.mark_stale();
            Ok(())
        }
    }
}