use std::convert::{Into, TryInto};
use super::*;
use crate::{error::LendingError, math::{Rate, TryDiv}};
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
    pub interest: u64,
    ///
    pub fee: u64,
    ///
    pub loss: u64,
}

impl Liquidity {
    ///
    pub fn utilization_rate(&self) -> Result<Rate, ProgramError> {
        let total = self.available
            .checked_add(self.borrowed)
            .ok_or(LendingError::MathOverflow)?;

        Decimal::from(self.borrowed)
            .try_div(total)?
            .try_into()
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
    pub fn repay(&mut self, fund: &Fund) -> ProgramResult {
        self.available = self.available
            .checked_add(fund.principal)
            .ok_or(LendingError::MathOverflow)?
            .checked_add(fund.interest)
            .ok_or(LendingError::MathOverflow)?;
        
        self.interest = self.interest
            .checked_add(fund.interest)
            .ok_or(LendingError::MathOverflow)?;

        Ok(())
    }
    ///
    pub fn liquidate(&mut self, fund: &Fund, fee: u64) -> ProgramResult {
        self.available = self.available
            .checked_add(fund.principal)
            .ok_or(LendingError::MathOverflow)?
            .checked_add(fund.interest)
            .ok_or(LendingError::MathOverflow)?;
    
        self.interest = self.interest
            .checked_add(fund.interest)
            .ok_or(LendingError::MathOverflow)?;

        self.fee = self.fee
            .checked_add(fee)
            .ok_or(LendingError::MathOverflow)?;
        
        Ok(())
    }
    ///
    pub fn withdraw(&mut self, fund: &Fund, fee: u64) -> ProgramResult {
        if fund.interest > self.interest {
            msg!("insufficient interest reserve: {}", fund.interest);
            self.loss = self.loss
                .checked_add(fund.interest - self.interest)
                .ok_or(LendingError::MathOverflow)?;
            self.interest = 0;
        } else {
            self.interest -= fund.interest;
        }

        self.available = self.available
            .checked_sub(fund.principal)
            .ok_or(LendingError::MarketReserveLiquidityAvailableInsufficent)?
            .checked_sub(fund.interest)
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
    ///
    pub fn fill_loss(&mut self, loss: u64) -> ProgramResult {
        if loss > self.loss {
            self.loss = 0;
        } else {
            self.loss -= loss;
        }
        
        self.available = self.available
            .checked_add(loss)
            .ok_or(LendingError::MathOverflow)?;

        Ok(())
    }
    ///
    pub fn add_loss(&mut self, loss: u64) -> ProgramResult {
        self.loss = self.loss
            .checked_add(loss)
            .ok_or(LendingError::MathOverflow)?;

        Ok(())
    }
}

///
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LiquidityInfo {
    ///
    pub interest_rate_oracle: Pubkey,
    ///
    pub borrow_rate_oracle: Pubkey,
    ///
    pub min_borrow_utilization_rate: u64,
    ///
    pub max_borrow_utilization_rate: u64,
    ///
    pub interest_fee_rate: u64,
    ///
    pub liquidity: Liquidity,
}

impl Sealed for LiquidityInfo {}

const MARKET_RESERVE_LIQUIDITY_INFO_LEN: usize = 128;

impl Pack for LiquidityInfo {
    const LEN: usize = MARKET_RESERVE_LIQUIDITY_INFO_LEN;

    fn pack_into_slice(&self, output: &mut [u8]) {
        let output = array_mut_ref![output, 0, MARKET_RESERVE_LIQUIDITY_INFO_LEN];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            interest_rate_oracle,
            borrow_rate_oracle,
            min_borrow_utilization_rate,
            max_borrow_utilization_rate,
            interest_fee_rate,
            available,
            borrowed,
            interest,
            fee,
            loss,
        ) = mut_array_refs![
            output,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            8,
            8,
            8,
            8,
            8,
            8,
            8,
            8
        ];

        interest_rate_oracle.copy_from_slice(self.interest_rate_oracle.as_ref());
        borrow_rate_oracle.copy_from_slice(self.borrow_rate_oracle.as_ref());
        *min_borrow_utilization_rate = self.min_borrow_utilization_rate.to_le_bytes();
        *max_borrow_utilization_rate = self.max_borrow_utilization_rate.to_le_bytes();
        *interest_fee_rate = self.interest_fee_rate.to_le_bytes();
        *available = self.liquidity.available.to_le_bytes();
        *borrowed = self.liquidity.borrowed.to_le_bytes();
        *interest = self.liquidity.interest.to_le_bytes();
        *fee = self.liquidity.fee.to_le_bytes();
        *loss = self.liquidity.loss.to_le_bytes();
    }

    fn unpack_from_slice(input: &[u8]) -> Result<Self, ProgramError> {
        let input = array_ref![input, 0, MARKET_RESERVE_LIQUIDITY_INFO_LEN];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            interest_rate_oracle,
            borrow_rate_oracle,
            min_borrow_utilization_rate,
            max_borrow_utilization_rate,
            interest_fee_rate,
            available,
            borrowed,
            interest,
            fee,
            loss,
        ) = array_refs![
            input,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            8,
            8,
            8,
            8,
            8,
            8,
            8,
            8
        ];

        Ok(Self{
            interest_rate_oracle: Pubkey::new_from_array(*interest_rate_oracle),
            borrow_rate_oracle: Pubkey::new_from_array(*borrow_rate_oracle),
            min_borrow_utilization_rate: u64::from_le_bytes(*min_borrow_utilization_rate),
            max_borrow_utilization_rate: u64::from_le_bytes(*max_borrow_utilization_rate),
            interest_fee_rate: u64::from_le_bytes(*interest_fee_rate),
            liquidity: Liquidity{
                available: u64::from_le_bytes(*available),
                borrowed: u64::from_le_bytes(*borrowed),
                interest: u64::from_le_bytes(*interest),
                fee: u64::from_le_bytes(*fee),
                loss: u64::from_le_bytes(*loss),
            }
        })
    }
}

///
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CollateralInfo {
    ///
    pub liquidate_fee_rate: u64,
    ///
    pub liquidate_limit_rate: u64,
    ///
    pub amount: u64,
}

impl CollateralInfo {
    ///
    pub fn add(&mut self, amount: u64) -> ProgramResult {
        self.amount = self.amount
            .checked_add(amount)
            .ok_or(LendingError::MathOverflow)?;
        
        Ok(())
    }
    ///
    pub fn reduce(&mut self, amount: u64) -> ProgramResult {
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
    pub timestamp: Slot,
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

const MARKET_RESERVE_LEN: usize = 258;

impl Pack for MarketReserve {
    const LEN: usize = MARKET_RESERVE_LEN;

    fn pack_into_slice(&self, output: &mut [u8]) {
        let output = array_mut_ref![output, 0, MARKET_RESERVE_LEN];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            version,
            timestamp,
            manager,
            token_info,
            liquidity_info,
            liquidate_fee_rate,
            liquidate_limit_rate,
            amount,
        ) = mut_array_refs![
            output,
            1,
            8,
            PUBKEY_BYTES,
            TOKEN_INFO_LEN,
            1 + MARKET_RESERVE_LIQUIDITY_INFO_LEN,
            8,
            8,
            8
        ];

        *version = self.version.to_le_bytes();
        *timestamp = self.timestamp.to_le_bytes();
        manager.copy_from_slice(self.manager.as_ref());
        self.token_info.pack_into_slice(token_info);
        pack_coption_struct(&self.liquidity_info, liquidity_info);
        *liquidate_fee_rate = self.collateral_info.liquidate_fee_rate.to_le_bytes();
        *liquidate_limit_rate = self.collateral_info.liquidate_limit_rate.to_le_bytes();
        *amount = self.collateral_info.amount.to_le_bytes();
    }

    fn unpack_from_slice(input: &[u8]) -> Result<Self, ProgramError> {
        let input = array_ref![input, 0, MARKET_RESERVE_LEN];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            version,
            timestamp,
            manager,
            token_info,
            liquidity_info,
            liquidate_fee_rate,
            liquidate_limit_rate,
            amount,
        ) = array_refs![
            input,
            1,
            8,
            PUBKEY_BYTES,
            TOKEN_INFO_LEN,
            1 + MARKET_RESERVE_LIQUIDITY_INFO_LEN,
            8,
            8,
            8
        ];

        let version = u8::from_le_bytes(*version);
        if version > PROGRAM_VERSION {
            msg!("MarketReserve version does not match lending program version");
            return Err(ProgramError::InvalidAccountData);
        }

        let token_info = TokenInfo::unpack_from_slice(token_info)?;
        let liquidity_info = unpack_coption_struct::<LiquidityInfo>(liquidity_info)?;

        Ok(Self{
            version,
            timestamp: Slot::from_le_bytes(*timestamp),
            manager: Pubkey::new_from_array(*manager),
            token_info,
            liquidity_info,
            collateral_info: CollateralInfo{
                liquidate_fee_rate: u64::from_le_bytes(*liquidate_fee_rate),
                liquidate_limit_rate: u64::from_le_bytes(*liquidate_limit_rate),
                amount: u64::from_le_bytes(*amount),
            },
        })
    }
}

// impl MarketReserve {
//     pub fn deposit(&mut self, amount: u64) -> ProgramResult {
//         self.liquidity_info
//             .as_mut()
//             .ok_or(LendingError::MarketReserveLiquidityNotAvailable)?
//             .liquidity.deposit(amount)
//     }

//     pub fn withdraw(&mut self, fund: &Fund, fee: u64) -> ProgramResult {
//         self.liquidity_info
//             .as_mut()
//             .ok_or(LendingError::MarketReserveLiquidityNotAvailable)?
//             .liquidity.withdraw(fund, fee)
//     }

//     pub fn borrow_out(&mut self, amount: u64) -> ProgramResult {
//         let liquidity_info = self.liquidity_info
//             .as_mut()
//             .ok_or(LendingError::MarketReserveLiquidityNotAvailable)?;

//         let utilization_rate = liquidity_info.liquidity.utilization_rate()?;
//         if utilization_rate >= Rate::from_scaled_val(liquidity_info.max_borrow_utilization_rate) {
//             Err(LendingError::MarketReserveLiquidityUtilizationTooLarge.into())
//         } else if utilization_rate <= Rate::from_scaled_val(liquidity_info.min_borrow_utilization_rate) {
//             Err(LendingError::MarketReserveLiquidityUtilizationTooSmall.into())
//         } else {
//             liquidity_info.liquidity.borrow_out(amount)
//         }
//     }

//     pub fn repay(&mut self, fund: &Fund) -> ProgramResult {
//         self.liquidity_info
//             .as_mut()
//             .ok_or(LendingError::MarketReserveLiquidityNotAvailable)?
//             .liquidity.repay(fund)
//     }

//     pub fn liquidate(&mut self, fund: &Fund, fee: u64) -> ProgramResult {
//         self.liquidity_info
//             .as_mut()
//             .ok_or(LendingError::MarketReserveLiquidityNotAvailable)?
//             .liquidity.liquidate(fund, fee)
//     }
// }