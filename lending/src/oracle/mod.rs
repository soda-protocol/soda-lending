#![allow(missing_docs)]
mod pyth;
mod chainlink;
mod switchboard;

pub use pyth::*;
pub use chainlink::*;
pub use switchboard::*;

use borsh::{BorshSerialize, BorshDeserialize};
use num_derive::{FromPrimitive, ToPrimitive};
use num_traits::{FromPrimitive, ToPrimitive};
use solana_program::{clock::Clock, entrypoint::ProgramResult, program_error::ProgramError, pubkey::Pubkey};
use crate::{math::Decimal, state::Param};

#[derive(Clone, Copy, BorshSerialize, BorshDeserialize, Debug, FromPrimitive, ToPrimitive, PartialEq)]
pub enum OracleType {
    ///
    Pyth,
    ///
    ChainLink,
    ///
    SwitchboardV1,
    ///
    SwitchboardV2,
}

impl OracleType {
    pub fn parse_price(&self, data: &[u8], clock: &Clock) -> Result<Decimal, ProgramError> {
        match self {
            OracleType::Pyth => get_pyth_price(data, clock),
            OracleType::ChainLink => get_chainlink_price(data, clock),
            OracleType::SwitchboardV1 => get_switchboard_price_v1(data, clock),
            OracleType::SwitchboardV2 => get_switchboard_price_v2(data, clock),
        }
    }
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

#[derive(Clone, Copy, Debug, PartialEq, BorshSerialize, BorshDeserialize)]
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
        self.price = self.config.oracle_type.parse_price(data, clock)?;

        Ok(())
    }
}
