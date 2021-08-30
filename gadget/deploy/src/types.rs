use std::{str::FromStr, time::Duration, error::Error, thread, convert::{TryInto, TryFrom}, collections::HashMap};

use solana_client::{
    blockhash_query::BlockhashQuery, 
    rpc_client::RpcClient, 
    rpc_request::TokenAccountsFilter,
};
use solana_sdk::{
    clock::{Clock, Slot}, 
    commitment_config::CommitmentConfig, hash::Hash, instruction::Instruction, program_pack::Pack, pubkey::Pubkey, signer::{Signer, keypair::Keypair}, system_instruction::create_account, transaction::Transaction};
use spl_token::{
    instruction::{initialize_mint, initialize_account, mint_to},
    state::{Mint, Account},
};
use soda_lending_contract::{math::{Decimal, Rate, TryAdd, TryDiv, TryMul, WAD}, pyth::{self, Product}, state::{
        Manager, MarketReserve, RateOracle, UserObligation, 
        CollateralConfig, LiquidityConfig
    }};
use bincode;

use crate::error::SodaError;

#[derive(Debug, Clone, Copy)]
pub struct LoanInfo {
    ///
    pub reserve: Pubkey,
    ///
    pub acc_borrow_rate_wads: Decimal,
    ///
    pub borrowed_amount_wads: Decimal,
    ///
    pub loan_value: Decimal,
}

#[derive(Debug, Clone, Copy)]
pub struct CollateralInfo {
    ///
    pub reserve: Pubkey,
    ///
    pub so_amount: u64,
    ///
    pub amount: u64,
    ///
    pub borrow_equivalent_value: Decimal,
    ///
    pub liquidation_equivalent_value: Decimal,
    ///
    pub max_value: Decimal,
}

#[derive(Debug, Clone)]
pub struct UserObligationInfo {
    ///
    pub manager: Pubkey,
    ///
    pub owner: Pubkey,
    ///
    pub collaterals: Vec<CollateralInfo>,
    ///
    pub collaterals_borrow_value: Decimal,
    ///
    pub collaterals_liquidation_value: Decimal,
    ///
    pub collaterals_max_value: Decimal,
    ///
    pub loans: Vec<LoanInfo>,
    ///
    pub loans_value: Decimal,
}

impl UserObligationInfo {
    pub fn from_raw_data(
        clock_data: &[u8],
        obligation_data: &[u8],
        market_and_price_map: &HashMap<Pubkey, (Vec<u8>, Vec<u8>, Vec<u8>)>,
    ) -> Result<Self, SodaError> {
        let clock: &Clock = &bincode::deserialize(&clock_data).map_err(|_| SodaError::Other)?;

        let obligation = UserObligation::unpack(obligation_data)?;
    
        let collaterals = obligation.collaterals
            .iter()
            .map(|collateral| {
                let (market_reserve_data, price_data, rate_oracle_data) = market_and_price_map
                    .get(&collateral.reserve)
                    .ok_or(SodaError::Other)?;

                let mut market_reserve = MarketReserve::unpack(market_reserve_data)?;
                let price = get_pyth_price(price_data, clock)?;
                let rate_oracle = RateOracle::unpack(rate_oracle_data)?;
                
                let utilization_rate = market_reserve.liquidity_info.utilization_rate()?;
                let borrow_rate = rate_oracle.calculate_borrow_rate(clock.slot, utilization_rate)?;
                market_reserve.accrue_interest(borrow_rate, clock.slot)?;

                let collateral_amount = market_reserve.exchange_collateral_to_liquidity(collateral.amount)?;

                let decimals = 10u64
                    .checked_pow(market_reserve.token_info.decimal as u32)
                    .ok_or(SodaError::Other)?;

                let max_value = price
                    .try_mul(collateral_amount)?
                    .try_div(decimals)?;
                let borrow_equivalent_value = max_value.try_mul(Rate::from_scaled_val(collateral.borrow_value_ratio))?;
                let liquidation_equivalent_value = max_value.try_mul(Rate::from_scaled_val(collateral.liquidation_value_ratio))?;

                Ok(CollateralInfo {
                    reserve: collateral.reserve,
                    so_amount: collateral.amount,
                    amount: collateral_amount,
                    borrow_equivalent_value,
                    liquidation_equivalent_value,
                    max_value,
                })
            }).collect::<Result<Vec<_>, SodaError>>()?;
            
        let loans = obligation.loans
            .iter()
            .map(|loan| {
                let (market_reserve_data, price_data, rate_oracle_data) = market_and_price_map
                    .get(&loan.reserve)
                    .ok_or(SodaError::Other)?;

                let mut market_reserve = MarketReserve::unpack(market_reserve_data)?;
                let price = get_pyth_price(price_data, clock)?;
                let rate_oracle = RateOracle::unpack(rate_oracle_data)?;
                
                let utilization_rate = market_reserve.liquidity_info.utilization_rate()?;
                let borrow_rate = rate_oracle.calculate_borrow_rate(clock.slot, utilization_rate)?;
                market_reserve.accrue_interest(borrow_rate, clock.slot)?;

                let (acc_borrow_rate_wads, borrowed_amount_wads) = 
                if market_reserve.liquidity_info.acc_borrow_rate_wads > loan.acc_borrow_rate_wads {
                    let compounded_interest_rate: Rate = market_reserve.liquidity_info.acc_borrow_rate_wads
                        .try_div(loan.acc_borrow_rate_wads)?
                        .try_into()?;

                    (
                        market_reserve.liquidity_info.acc_borrow_rate_wads,
                        loan.borrowed_amount_wads.try_mul(compounded_interest_rate)?
                    )
                } else {
                    (loan.acc_borrow_rate_wads, loan.borrowed_amount_wads)
                };

                let decimals = 10u64
                    .checked_pow(market_reserve.token_info.decimal as u32)
                    .ok_or(SodaError::Other)?;

                let loan_value = price
                    .try_mul(borrowed_amount_wads.try_ceil_u64()?)?
                    .try_div(decimals)?;

                Ok(LoanInfo {
                    reserve: loan.reserve,
                    acc_borrow_rate_wads,
                    borrowed_amount_wads,
                    loan_value,
                })
            }).collect::<Result<Vec<_>, SodaError>>()?;

        let (collaterals_borrow_value, collaterals_liquidation_value, collaterals_max_value) = collaterals
            .iter()
            .try_fold((Decimal::zero(), Decimal::zero(), Decimal::zero()),
            |acc, collateral| -> Result<_, SodaError> {
                Ok((
                    acc.0.try_add(collateral.borrow_equivalent_value)?,
                    acc.1.try_add(collateral.liquidation_equivalent_value)?,
                    acc.2.try_add(collateral.max_value)?,
                ))
            })?;

        let loans_value = loans
            .iter()
            .try_fold(Decimal::zero(), |acc, loan| acc.try_add(loan.borrowed_amount_wads))?;

        Ok(Self {
            manager: obligation.manager,
            owner: obligation.owner,
            collaterals,
            collaterals_borrow_value,
            collaterals_liquidation_value,
            collaterals_max_value,
            loans,
            loans_value,
        })
    }
}

pub fn get_pyth_price(pyth_price_data: &[u8], clock: &Clock) -> Result<Decimal, SodaError> {
    const STALE_AFTER_SLOTS_ELAPSED: u64 = 100;

    let pyth_price = pyth::load::<pyth::Price>(&pyth_price_data)
        .map_err(|_| SodaError::Other)?;

    if pyth_price.ptype != pyth::PriceType::Price {
        return Err(SodaError::Other);
    }

    let slots_diff = if clock.slot > pyth_price.valid_slot {
        clock.slot - pyth_price.valid_slot
    } else {
        pyth_price.valid_slot - clock.slot
    };
    if slots_diff >= STALE_AFTER_SLOTS_ELAPSED {
        return Err(SodaError::Other);
    }

    let price: u64 = pyth_price.agg.price.try_into().map_err(|_| SodaError::Other)?;

    if pyth_price.expo >= 0 {
        let exponent = pyth_price.expo
            .try_into()
            .map_err(|_| SodaError::Other)?;
        let zeros = 10u64
            .checked_pow(exponent)
            .ok_or(SodaError::Other)?;

        Ok(Decimal::from(price).try_mul(zeros)?)
    } else {
        let exponent = pyth_price.expo
            .checked_abs()
            .ok_or(SodaError::Other)?
            .try_into()
            .map_err(|_| SodaError::Other)?;
        let decimals = 10u64
            .checked_pow(exponent)
            .ok_or(SodaError::Other)?;
        
        Ok(Decimal::from(price).try_div(decimals)?)
    }
}