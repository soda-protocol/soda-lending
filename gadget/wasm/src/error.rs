use thiserror::Error;

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum SodaError {
    ///
    #[error("Program error: {0:#x}")]
    Custom(u64),
    ///
    #[error("Math operation overflow")]
    MathOverflow,
    ///
    #[error("Pack error")]
    PackError,
    ///
    #[error("Unpack error")]
    UnpackError,
    ///
    #[error("Invalid pubkey")]
    InvalidPubkey,
}