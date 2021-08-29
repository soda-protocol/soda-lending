use num_derive::FromPrimitive;
use thiserror::Error;

use solana_sdk::program_error::ProgramError;

#[derive(Clone, Debug, Eq, Error, FromPrimitive, PartialEq)]
pub enum SodaError {
    ///
    #[error("soda: invalid account data")]
    InvalidAccountData,
    #[error("soda: debug error 1")]
    DebugError1,
    #[error("soda: debug error 2")]
    DebugError2,
    #[error("soda: debug error 3")]
    DebugError3,
    #[error("soda: debug error 4")]
    DebugError4,
    #[error("soda: debug error 5")]
    DebugError5,
    #[error("soda: debug error 6")]
    DebugError6,
    #[error("soda: debug error 7")]
    DebugError7,
    #[error("soda: debug error 8")]
    DebugError8,
    #[error("soda: debug error 9")]
    DebugError9,
    #[error("soda: debug error 10")]
    DebugError10,
    #[error("soda: debug error 11")]
    DebugError11,
}

impl From<SodaError> for ProgramError {
    fn from(e: SodaError) -> Self {
        ProgramError::Custom(e as u32)
    }
}