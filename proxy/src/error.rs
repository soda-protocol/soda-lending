//! Error types

use num_derive::FromPrimitive;
use solana_program::{
    decode_error::DecodeError,
    program_error::ProgramError,
};
use thiserror::Error;

/// Errors that may be returned by the TokenLending program.
#[derive(Clone, Debug, Eq, Error, FromPrimitive, PartialEq)]
pub enum ProxyError {
    /// Invalid instruction data passed in.
    #[error("Failed to unpack instruction data")]
    InstructionUnpackError,
    ///
    #[error("Raydium invalid fee")]
    RaydiumInvalidFee,
    ///
    #[error("Raydium invalid status")]
    RaydiumInvalidStatus,
    ///
    #[error("Token initialize account failed")]
    TokenInitializeAccountFailed,
    ///
    #[error("Token transfer failed")]
    TokenTransferFailed,
    ///
    #[error("Token account close failed")]
    TokenAccountCloseFailed,
    ///
    #[error("Token account sync Native failed")]
    TokenAccountSyncNativeFailed,
    ///
    #[error("Operation overflowed")]
    MathOverflow,
}

impl From<ProxyError> for ProgramError {
    fn from(e: ProxyError) -> Self {
        ProgramError::Custom(e as u32)
    }
}

impl<T> DecodeError<T> for ProxyError {
    fn type_of() -> &'static str {
        "Soda Flash Liquidation Error"
    }
}