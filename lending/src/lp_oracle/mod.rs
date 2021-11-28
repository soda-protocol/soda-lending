#![allow(missing_docs)]

mod standard;

pub use standard::*;

use solana_program::{clock::{Clock, Slot}, entrypoint::ProgramResult, program_option::COption, pubkey::Pubkey};
use spl_token_swap::{curve::calculator::RoundDirection, state::{SwapState, SwapVersion}};
use crate::{math::Decimal, oracle::OracleConfig};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LPType {
    Standard,
    Raydium,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LPAggregatorConfig {
    pub lp_type: LPType,
    pub lp_mint: Pubkey,
    pub lp_decimal: u8,
    pub token_a: (Pubkey, u8, OracleConfig),
    pub token_b: (Pubkey, u8, OracleConfig),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LPAggregator {
    pub is_initialized: bool,
    pub config: LPAggregatorConfig,
    pub last_update: Slot,
    pub price: COption<Decimal>,
}

impl LPAggregator {
    pub fn refresh_lp_price(
        &mut self,
        oracle_a_data: &[u8],
        oracle_b_data: &[u8],
        pool_data: &[u8],
        clock: &Clock,
    ) -> ProgramResult {
        let price_a = self.config.token_a.2.oracle_type.parse_price(oracle_a_data, clock)?;
        let price_b = self.config.token_b.2.oracle_type.parse_price(oracle_b_data, clock)?;


        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PoolBalance {
    pub lp_supply: u64,
    pub balance_a: u64,
    pub balance_b: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TokenPrice {
    pub price: Decimal,
    pub decimal: u8,
}