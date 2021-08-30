use num_derive::FromPrimitive;
use solana_client::client_error::ClientError;
use thiserror::Error;

use solana_sdk::program_error::ProgramError;

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum SodaError {
    ///
    #[error("Program error: {0:#x}")]
    Custom(u64),
    ///
    #[error("Other error")]
    Other,
    ///
    #[error("Client error")]
    Client,
}

impl From<ProgramError> for SodaError {
    fn from(e: ProgramError) -> Self {
        Self::Custom(u64::from(e))
    }
}

impl From<ClientError> for SodaError {
    fn from(_: ClientError) -> Self {
        Self::Client
    }
}