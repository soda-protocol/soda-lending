#![allow(missing_docs)]

//! Storage:
//! ----
//! 4kb aggregator state
//! ----
//! u64 current_pos
//! ... round data

use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    msg,
    borsh::try_from_slice_unchecked,
    clock::{Clock, UnixTimestamp},
    program_pack::IsInitialized,
    pubkey::Pubkey,
    program_error::ProgramError,
};
use crate::{error::LendingError, math::{Decimal, TryDiv}};

pub const MAX_ORACLES: usize = 8;

pub type Timestamp = UnixTimestamp;
pub type Value = u128;

#[derive(Clone, Copy, Eq, PartialEq, BorshSerialize, BorshDeserialize, Default, Debug)]
#[repr(C)]
pub struct Submission(pub Timestamp, pub Value);

unsafe impl bytemuck::Zeroable for Submission {
    fn zeroed() -> Self {
        Self::default()
    }
}
unsafe impl bytemuck::Pod for Submission {}

/// Define the type of state stored in accounts
#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct Aggregator {
    /// Set to true after initialization.
    pub is_initialized: bool,

    /// Version of the state
    pub version: u32,

    /// The configuration for this aggregator
    pub config: Config,

    /// When the config was last updated.
    pub updated_at: Timestamp,

    /// The aggregator owner is allowed to modify it's config.
    pub owner: Pubkey,

    /// A set of current submissions, one per oracle. Array index corresponds to oracle index.
    pub submissions: [Submission; MAX_ORACLES],

    /// The current median answer.
    pub answer: Option<Value>,
}

impl IsInitialized for Aggregator {
    fn is_initialized(&self) -> bool {
        self.is_initialized
    }
}

#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Config {
    /// A list of oracles allowed to submit answers.
    pub oracles: Vec<Pubkey>,

    /// Number of submissions required to produce an answer. Must be larger than 0.
    pub min_answer_threshold: u8,

    /// Offset in number of seconds before a submission is considered stale.
    pub staleness_threshold: u8,

    /// Decimal places for value representations
    pub decimals: u8,
}

pub fn get_chainlink_price(data: &[u8], clock: &Clock) -> Result<Decimal, ProgramError> {
    #[cfg(not(feature = "devnet"))]
    const STALE_AFTER_SECS_ELAPSED: i64 = 4;
    #[cfg(feature = "devnet")]
    const STALE_AFTER_SECS_ELAPSED: i64 = 8;

    let aggregator = try_from_slice_unchecked::<Aggregator>(&data[..4096])?;
    if !aggregator.is_initialized() {
        return Err(ProgramError::UninitializedAccount);
    }

    let price = aggregator.answer.ok_or_else(|| {
        msg!("Chainlink oracle price is not available");
        LendingError::InvalidPriceOracle
    })?;

    let agg_lastupdate = aggregator
        .submissions
        .iter()
        .map(|submission| submission.0)
        .max()
        .ok_or_else(|| {
            msg!("Chainlink oracle has no submissions");
            LendingError::InvalidPriceOracle
        })?;

    let secs_eplased = clock.unix_timestamp
        .checked_sub(agg_lastupdate)
        .ok_or(LendingError::MathOverflow)?;
    if secs_eplased >= STALE_AFTER_SECS_ELAPSED {
        msg!("Chainlink oracle price is stale");
        return Err(LendingError::InvalidPriceOracle.into());
    }

    let decimals = 10u64
        .checked_pow(aggregator.config.decimals as u32)
        .ok_or(LendingError::MathOverflow)?;

    Decimal::from(price).try_div(decimals)
}