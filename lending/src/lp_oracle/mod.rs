#![allow(missing_docs)]

mod standard;

pub use standard::*;

use borsh::{BorshSerialize, BorshDeserialize};
use solana_program::{
    msg,
    clock::{Clock, Slot},
    entrypoint::ProgramResult,
    program_error::ProgramError,
    program_pack::{IsInitialized, Sealed, Pack},
    pubkey::Pubkey,
};
use spl_token_swap::state::SwapVersion;
use crate::{math::Decimal, oracle::OracleConfig, state::{PROGRAM_VERSION, UNINITIALIZED_VERSION}};

#[derive(Clone, Debug, PartialEq)]
pub struct TokenPrice {
    pub price: Decimal,
    pub decimal: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, BorshSerialize, BorshDeserialize)]
pub enum LPType {
    Standard,
    Raydium,
}

#[derive(Clone, Copy, Debug, PartialEq, BorshSerialize, BorshDeserialize)]
pub struct TokenInfo {
    pub mint: Pubkey,
    pub decimal: u8,
    pub oracle_config: OracleConfig,
}

#[derive(Clone, Copy, Debug, PartialEq, BorshSerialize, BorshDeserialize)]
pub struct LPAggregatorConfig {
    pub lp_type: LPType,
    pub lp_mint: Pubkey,
    pub lp_decimal: u8,
    pub token_a: TokenInfo,
    pub token_b: TokenInfo,
}

#[derive(Clone, Copy, Debug, PartialEq, BorshSerialize, BorshDeserialize)]
pub struct LPAggregator {
    pub version: u8,
    pub enable: bool,
    pub last_update: Slot,
    pub owner: Pubkey,
    pub config: LPAggregatorConfig,
    pub price: u128,
}

impl LPAggregator {
    pub fn new(owner: Pubkey, config: LPAggregatorConfig) -> Self {
        Self {
            version: PROGRAM_VERSION,
            enable: true,
            last_update: 0,
            owner,
            config,
            price: 0,
        }
    }

    pub fn control(&mut self, enable: bool) {
        self.enable = enable;
    }

    pub fn feed_standard_lp_price(
        &mut self,
        token_swap: &SwapVersion,
        oracle_a_data: &[u8],
        oracle_b_data: &[u8],
        clock: &Clock,
        lp_supply: u64,
        balance_a: u64,
        balance_b: u64,
        max_lp_amount: u64,
    ) -> ProgramResult {
        if !self.enable {
            return Err();
        }

        let price_a = self.config.token_a.oracle_config.oracle_type.parse_price(oracle_a_data, clock)?;
        let price_b = self.config.token_b.oracle_config.oracle_type.parse_price(oracle_b_data, clock)?;

        let price = estimate_standard_lp_price(
            token_swap,
            lp_supply,
            balance_a,
            balance_b,
            max_lp_amount,
            self.config.lp_decimal,
            TokenPrice {
                price: price_a,
                decimal: self.config.token_a.decimal,
            },
            TokenPrice {
                price: price_b,
                decimal: self.config.token_b.decimal,
            },
        )?;
        self.price = price.to_scaled_val()?;
        self.last_update = clock.slot;

        Ok(())
    }
}

impl Sealed for LPAggregator {}
impl IsInitialized for LPAggregator {
    fn is_initialized(&self) -> bool {
        self.version != UNINITIALIZED_VERSION
    }
}

const LP_AGGREGATOR_LEN: usize = 1024;

impl Pack for LPAggregator {
    const LEN: usize = LP_AGGREGATOR_LEN;

    fn pack_into_slice(&self, output: &mut [u8]) {
        self.serialize(&mut output.as_mut()).expect("LPAggregator borsh serialize error");
    }

    fn unpack_from_slice(input: &[u8]) -> Result<Self, ProgramError> {
        let lp_agg: Self = BorshDeserialize::deserialize(&mut input.as_ref()).expect("LPAggregator borsh deserialize error");
        if lp_agg.version > PROGRAM_VERSION {
            msg!("LPAggregator version does not match lending program version");
            return Err(ProgramError::InvalidAccountData);
        }

        Ok(lp_agg)
    }
}