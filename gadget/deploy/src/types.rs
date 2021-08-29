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
use soda_lending_contract::{
    math::{WAD, Rate, Decimal, TryMul, TryDiv},
    state::{
        Manager, MarketReserve, RateOracle, UserObligation, 
        CollateralConfig, LiquidityConfig
    },
    pyth::{self, Product},
};
use bincode;

use crate::error::SodaError;

#[derive(Debug, Clone, Copy)]
pub struct LoanInfo {
    ///
    pub reserve: Pubkey,
    ///
    pub acc_borrow_rate_wads: f64,
    ///
    pub borrowed_amount_wads: f64,
    ///
    pub loan_value: f64,
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
    pub borrow_equivalent_value: f64,
    ///
    pub liquidation_equivalent_value: f64,
    ///
    pub max_value: f64,
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
    pub collaterals_borrow_value: f64,
    ///
    pub collaterals_liquidation_value: f64,
    ///
    pub collaterals_max_value: f64,
    ///
    pub loans: Vec<LoanInfo>,
    ///
    pub loans_value: f64,
}

impl UserObligationInfo {
    pub fn from_raw_data(
        clock_data: &[u8],
        obligation_data: &[u8],
        market_and_price_map: &HashMap<Pubkey, (Vec<u8>, Vec<u8>, Vec<u8>)>,
    ) -> Result<Self, SodaError> {
        let clock: &Clock = &bincode::deserialize(&clock_data).map_err(|_| SodaError::InvalidAccountData)?;

        let obligation = UserObligation::unpack(obligation_data).map_err(|_| SodaError::InvalidAccountData)?;
    
        let collaterals = obligation.collaterals
            .iter()
            .map(|collateral| {
                let (market_reserve_data, price_data, rate_oracle_data) = market_and_price_map
                    .get(&collateral.reserve)
                    .ok_or(SodaError::InvalidAccountData)?;

                let mut market_reserve = MarketReserve::unpack(market_reserve_data)
                    .map_err(|_| SodaError::InvalidAccountData)?;
                let price = get_pyth_price(price_data, clock)?;
                let rate_oracle = RateOracle::unpack(rate_oracle_data).map_err(|_| SodaError::InvalidAccountData)?;
                
                let utilization_rate = market_reserve.liquidity_info
                    .utilization_rate()
                    .map_err(|_| SodaError::InvalidAccountData)?;
                let borrow_rate = rate_oracle
                    .calculate_borrow_rate(clock.slot, utilization_rate)
                    .map_err(|_| SodaError::InvalidAccountData)?;
                market_reserve  
                    .accrue_interest(borrow_rate, clock.slot)
                    .map_err(|_| SodaError::InvalidAccountData)?;

                let collateral_amount = market_reserve
                    .calculate_collateral_to_liquidity(collateral.amount)
                    .map_err(|_| SodaError::InvalidAccountData)?;

                let decimals = 10u64
                    .checked_pow(market_reserve.token_info.decimal as u32)
                    .ok_or(SodaError::InvalidAccountData)?;

                let max_value = price * (collateral_amount as f64 / decimals as f64);
                let borrow_equivalent_value = max_value * (collateral.borrow_value_ratio as f64 / WAD as f64);
                let liquidation_equivalent_value = max_value * (collateral.liquidation_value_ratio as f64 / WAD as f64);

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
                    .ok_or(SodaError::InvalidAccountData)?;

                    let mut market_reserve = MarketReserve::unpack(market_reserve_data)
                    .map_err(|_| SodaError::InvalidAccountData)?;
                let price = get_pyth_price(price_data, clock)?;
                let rate_oracle = RateOracle::unpack(rate_oracle_data).map_err(|_| SodaError::InvalidAccountData)?;
                
                let utilization_rate = market_reserve.liquidity_info
                    .utilization_rate()
                    .map_err(|_| SodaError::InvalidAccountData)?;
                let borrow_rate = rate_oracle
                    .calculate_borrow_rate(clock.slot, utilization_rate)
                    .map_err(|_| SodaError::InvalidAccountData)?;
                market_reserve  
                    .accrue_interest(borrow_rate, clock.slot)
                    .map_err(|_| SodaError::InvalidAccountData)?;

                let decimals = 10u64
                    .checked_pow(market_reserve.token_info.decimal as u32)
                    .ok_or(SodaError::InvalidAccountData)?;

                let ref_acc_borrow_rate_wads = u128::try_from(market_reserve.liquidity_info.acc_borrow_rate_wads.0)
                    .map_err(|_| SodaError::InvalidAccountData)? as f64 / WAD as f64;

                let mut acc_borrow_rate_wads = u128::try_from(loan.acc_borrow_rate_wads.0)
                    .map_err(|_| SodaError::InvalidAccountData)? as f64 / WAD as f64;
                let mut borrowed_amount_wads = u128::try_from(loan.borrowed_amount_wads.0)
                    .map_err(|_| SodaError::InvalidAccountData)? as f64 / WAD as f64;

                if ref_acc_borrow_rate_wads > acc_borrow_rate_wads {
                    let compounded_interest_rate = ref_acc_borrow_rate_wads / acc_borrow_rate_wads;
                    acc_borrow_rate_wads = ref_acc_borrow_rate_wads;
                    borrowed_amount_wads = borrowed_amount_wads * compounded_interest_rate;
                }

                Ok(LoanInfo {
                    reserve: loan.reserve,
                    acc_borrow_rate_wads,
                    borrowed_amount_wads,
                    loan_value: borrowed_amount_wads * price / decimals as f64,
                })
            }).collect::<Result<Vec<_>, SodaError>>()?;

        let (collaterals_borrow_value, collaterals_liquidation_value, collaterals_max_value) = collaterals
            .iter()
            .fold((0f64, 0f64, 0f64), |acc, collateral|
                (
                    acc.0 + collateral.borrow_equivalent_value,
                    acc.1 + collateral.liquidation_equivalent_value,
                    acc.2 + collateral.max_value,
                )
            );

        let loans_value = loans
            .iter()
            .fold(0f64, |acc, loan| acc + loan.borrowed_amount_wads);

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

pub fn get_pyth_price(pyth_price_data: &[u8], clock: &Clock) -> Result<f64, SodaError> {
    const STALE_AFTER_SLOTS_ELAPSED: u64 = 100;

    let pyth_price = pyth::load::<pyth::Price>(&pyth_price_data)
        .map_err(|_| SodaError::InvalidAccountData)?;

    if pyth_price.ptype != pyth::PriceType::Price {
        return Err(SodaError::InvalidAccountData);
    }

    let slots_diff = if clock.slot > pyth_price.valid_slot {
        clock.slot - pyth_price.valid_slot
    } else {
        pyth_price.valid_slot - clock.slot
    };
    if slots_diff >= STALE_AFTER_SLOTS_ELAPSED {
        return Err(SodaError::InvalidAccountData);
    }

    let price: u64 = pyth_price.agg.price.try_into().map_err(|_| SodaError::InvalidAccountData)?;

    if pyth_price.expo >= 0 {
        let exponent = pyth_price.expo
            .try_into()
            .map_err(|_| SodaError::InvalidAccountData)?;
        let zeros = 10u64
            .checked_pow(exponent)
            .ok_or(SodaError::InvalidAccountData)?;

        Ok((price as f64) * (zeros as f64))
    } else {
        let exponent = pyth_price.expo
            .checked_abs()
            .ok_or(SodaError::InvalidAccountData)?
            .try_into()
            .map_err(|_| SodaError::InvalidAccountData)?;
        let decimals = 10u64
            .checked_pow(exponent)
            .ok_or(SodaError::InvalidAccountData)?;
        
        Ok((price as f64) / (decimals as f64))
    }
}