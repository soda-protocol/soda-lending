use std::{str::FromStr, time::Duration, error::Error, thread, convert::TryInto, collections::HashMap};

use solana_client::{
    blockhash_query::BlockhashQuery, 
    rpc_client::RpcClient, 
    rpc_request::TokenAccountsFilter,
};
use solana_sdk::{
    clock::{Clock, Slot}, 
    commitment_config::CommitmentConfig, hash::Hash, instruction::Instruction, program_error::ProgramError, program_pack::Pack, pubkey::Pubkey, signer::{Signer, keypair::Keypair}, system_instruction::create_account, transaction::Transaction};
use spl_token::{
    instruction::{initialize_mint, initialize_account, mint_to},
    state::{Mint, Account},
};
use soda_lending_contract::{
    math::{Rate, Decimal, TryMul, TryDiv},
    state::{
        Manager, MarketReserve, RateOracle, UserAsset, UserObligation, 
        CollateralConfig, LiquidityConfig
    },
    pyth::{self, Product},
};
use bincode;

#[derive(Debug, Clone, Copy)]
pub struct CollateralInfo {
    ///
    pub price_oracle: Pubkey,
    ///
    pub borrow_value_ratio: f64,
    ///
    pub liquidation_value_ratio: f64,
    ///
    pub amount: f64,
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
    pub version: u8,
    ///
    pub reserve: Pubkey,
    ///
    pub owner: Pubkey,
    ///
    pub collaterals: Vec<CollateralInfo>,
    /// 
    pub borrowed_amount: f64,
    ///
    pub dept_amount: f64,
    ///
    pub loan_value: f64,
}

impl UserObligationInfo {
    pub fn from_raw_data(
        clock_data: &[u8],
        market_reserve_data: &[u8],
        obligation_data: &[u8],
        rate_oracle_data: &[u8],
        liquidity_price_oracle_data: &[u8],
        collaterals_price_oracle_map: &HashMap<Pubkey, Vec<u8>>,
    ) -> Result<Self, ProgramError> {
        let clock: &Clock = &bincode::deserialize(&clock_data).map_err(|_| ProgramError::InvalidArgument)?;

        let rate = RateOracle::unpack(rate_oracle_data)?;
        rate.check_valid(clock.slot)?;

        let mut obligation = UserObligation::unpack(obligation_data)?;
        obligation.update_borrow_interest(clock.slot, Rate::from_scaled_val(rate.borrow_rate))?;

        let collaterals = obligation.collaterals
            .iter()
            .map(|collateral| {
                let price_data = collaterals_price_oracle_map
                    .get(&collateral.price_oracle)
                    .ok_or(ProgramError::InvalidAccountData)?;
                let price = get_pyth_price(price_data, clock)?;
                
                let decimals = 10u64
                    .checked_pow(collateral.decimal as u32)
                    .ok_or(ProgramError::InvalidAccountData)?;
                let amount = collateral.amount as f64 / decimals as f64;

                let borrow_value_ratio = collateral.borrow_value_ratio as f64 / 100f64;
                let liquidation_value_ratio = collateral.liquidation_value_ratio as f64 / 100f64;
                let max_value = price * amount;
                let borrow_equivalent_value = max_value * borrow_value_ratio;
                let liquidation_equivalent_value = max_value * liquidation_value_ratio;

                Ok(CollateralInfo {
                    price_oracle: collateral.price_oracle,
                    borrow_value_ratio,
                    liquidation_value_ratio,
                    amount,
                    borrow_equivalent_value,
                    liquidation_equivalent_value,
                    max_value,
                })
            }).collect::<Result<Vec<_>, ProgramError>>()?;
            
        let market_reserve = MarketReserve::unpack(market_reserve_data)?;
        let decimals = 10u64
            .checked_pow(market_reserve.token_info.decimal as u32)
            .ok_or(ProgramError::InvalidAccountData)?;
        let borrowed_amount = obligation.borrowed_amount as f64 / decimals as f64;
        let dept_amount = obligation.dept_amount as f64 / decimals as f64;
        let price = get_pyth_price(liquidity_price_oracle_data, clock)?;

        Ok(Self {
            version: obligation.version,
            reserve: obligation.reserve,
            owner: obligation.owner,
            collaterals,
            borrowed_amount,
            dept_amount,
            loan_value: dept_amount * price,
        })
    }

    pub fn get_effective_value(&self) -> (f64, f64, f64) {
        self.collaterals
            .iter()
            .fold((0f64, 0f64, 0f64), |acc, &c|
                (
                    acc.0 + c.borrow_equivalent_value,
                    acc.1 + c.liquidation_equivalent_value,
                    acc.2 + c.max_value,
                )
            )
    }
}

pub fn get_pyth_price(pyth_price_data: &[u8], clock: &Clock) -> Result<f64, ProgramError> {
    const STALE_AFTER_SLOTS_ELAPSED: u64 = 100;

    let pyth_price = pyth::load::<pyth::Price>(&pyth_price_data)
        .map_err(|_| ProgramError::InvalidAccountData)?;

    if pyth_price.ptype != pyth::PriceType::Price {
        return Err(ProgramError::InvalidAccountData);
    }

    let slots_diff = if clock.slot > pyth_price.valid_slot {
        clock.slot - pyth_price.valid_slot
    } else {
        pyth_price.valid_slot - clock.slot
    };
    if slots_diff >= STALE_AFTER_SLOTS_ELAPSED {
        return Err(ProgramError::InvalidAccountData);
    }

    let price: u64 = pyth_price.agg.price.try_into().map_err(|_| ProgramError::InvalidAccountData)?;

    if pyth_price.expo >= 0 {
        let exponent = pyth_price.expo
            .try_into()
            .map_err(|_| ProgramError::InvalidAccountData)?;
        let zeros = 10u64
            .checked_pow(exponent)
            .ok_or(ProgramError::InvalidAccountData)?;

        Ok((price as f64) * (zeros as f64))
    } else {
        let exponent = pyth_price.expo
            .checked_abs()
            .ok_or(ProgramError::InvalidAccountData)?
            .try_into()
            .map_err(|_| ProgramError::InvalidAccountData)?;
        let decimals = 10u64
            .checked_pow(exponent)
            .ok_or(ProgramError::InvalidAccountData)?;
        
        Ok((price as f64) / (decimals as f64))
    }
}