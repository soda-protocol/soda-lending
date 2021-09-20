#![allow(missing_docs)]
mod pyth;
mod chainlink;

pub use pyth::*;
pub use chainlink::*;

use crate::{math::Decimal, state::Param};
use num_derive::{FromPrimitive, ToPrimitive};
use num_traits::{FromPrimitive, ToPrimitive};
use solana_program::{
    clock::Clock,
    pubkey::Pubkey,
    entrypoint::ProgramResult,
};

#[derive(Clone, Copy, Debug, FromPrimitive, ToPrimitive, PartialEq)]
pub enum OracleType {
    ///
    Pyth,
    ///
    ChainLink,
}

impl From<u8> for OracleType {
    fn from(val: u8) -> Self {
        Self::from_u8(val).expect("Oracle type cannot be derived from u8")
    }
}

impl Into<u8> for OracleType {
    fn into(self) -> u8 {
        self.to_u8().expect("Oracle type cannot be convert into u8")
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OracleConfig {
    ///
    pub oracle: Pubkey,
    ///
    pub oracle_type: OracleType,
}

impl Param for OracleConfig {
    fn assert_valid(&self) -> ProgramResult {
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct OracleInfo {
    ///
    pub price: Decimal,
    ///
    pub config: OracleConfig,
}

impl OracleInfo {
    ///
    pub fn update_price(&mut self, data: &[u8], clock: &Clock) -> ProgramResult {
        self.price = match self.config.oracle_type {
            OracleType::Pyth => get_pyth_price(data, clock)?,
            OracleType::ChainLink => get_chainlink_price(data, clock)?,
        };

        Ok(())
    }
}