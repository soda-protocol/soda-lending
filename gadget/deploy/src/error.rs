use num_derive::FromPrimitive;
use thiserror::Error;

use solana_sdk::program_error::ProgramError;

#[derive(Clone, Debug, Eq, Error, FromPrimitive, PartialEq)]
pub enum SodaError {
    ///
    #[error("")]
    InvalidAccountData,
}

impl From<SodaError> for ProgramError {
    fn from(e: SodaError) -> Self {
        ProgramError::Custom(e as u32)
    }
}