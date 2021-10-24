//! Error types

use num_derive::FromPrimitive;
use solana_program::{
    decode_error::DecodeError,
    program_error::ProgramError,
};
use thiserror::Error;

/// Errors that may be returned by the TokenLending program.
#[derive(Clone, Debug, Eq, Error, FromPrimitive, PartialEq)]
pub enum LendingError {
    ///
    #[cfg(feature = "friend")]
    #[error("Obligation is already in binding")]
    AlreadyBindFriend,
    /// The account cannot be initialized because it is already in use.
    #[error("Account is already initialized")]
    AlreadyInitialized,
    ///
    #[error("Borrow amount is too small")]
    BorrowTooSmall,
    ///
    #[error("COption unpack error")]
    COptionUnpackError,
    ///
    #[error("Flash loan repay insufficient")]
    FlashLoanRepayInsufficient,
    /// The owner of the input isn't set to the program address generated by the program.
    #[error("Input account owner is not the program address")]
    InvalidAccountOwner,
    ///
    #[error("Input token account owner is invalid")]
    InvalidTokenAccountOwner,
    ///
    #[error("Authority is invalid")]
    InvalidAuthority,
    ///
    #[error("Manager is invalid")]
    InvalidManager,
    ///
    #[error("Manager authority is invalid")]
    InvalidManagerAuthority,
    ///
    #[error("Invalid supply token account")]
    InvalidSupplyTokenAccount,
    ///
    #[error("Market reserve is invalid")]
    InvalidMarketReserve,
    ///
    #[error("Unique Credit is invalid")]
    InvalidUniqueCredit,
    ///
    #[error("Price oracle is invalid")]
    InvalidPriceOracle,
    ///
    #[error("Rate model is invalid")]
    InvalidRateModel,
    ///
    #[error("Liquidity config is invalid")]
    InvalidLiquidityConfig,
    ///
    #[error("Collateral config is invalid")]
    InvalidCollateralConfig,
    ///
    #[error("Indexed collateral config is invalid")]
    InvalidIndexedCollateralConfig,
    ///
    #[error("Indexed loan config is invalid")]
    InvalidIndexedLoanConfig,
    // 10
    /// Invalid amount, must be greater than zero
    #[error("Input amount is invalid")]
    InvalidAmount,
    ///
    #[error("Input flash loan program is invalid")]
    InvalidFlashLoanProgram,
    ///
    #[error("Invalid soToken mint info")]
    InvalidSoTokenMint,
    /// Invalid instruction data passed in.
    #[error("Failed to unpack instruction data")]
    InstructionUnpackError,
    ///
    #[error("Insufficient unique credit limit")]
    InsufficientUniqueCreditLimit,
    ///
    #[error("Liquidation is not available")]
    LiquidationNotAvailable,
    /// Liquidation repay amount too small
    #[error("Liquidation repaying liquidity amount is too small")]
    LiquidationRepayTooSmall,
    /// Liquidation repay amount too small
    #[error("Liquidation repaying liquidity amount is too much")]
    LiquidationRepayTooMuch,
    /// Math operation overflow
    #[error("Math operation overflow")]
    MathOverflow,
    ///
    #[error("Market reserve is disabled")]
    MarketReserveDisabled,
    ///
    #[error("Market reserve deposit too much")]
    MarketReserveDepositTooMuch,
    ///
    #[error("Market reserve available liquidity is insufficient")]
    MarketReserveInsufficentLiquidity,
    ///
    #[error("Market reserve needs to be refreshed")]
    MarketReserveStale,
    /// Negative interest rate
    #[error("Interest rate is negative")]
    NegativeInterestRate,
    /// Lamport balance below rent-exempt threshold.
    #[error("Lamport balance below rent-exempt threshold")]
    NotRentExempt,
    /// Obligation state stale
    #[error("Obligation state needs to be refreshed")]
    ObligationStale,
    ///
    #[error("Obligation collaterals are not healthy")]
    ObligationNotHealthy,
    ///
    #[error("Obligation has dept")]
    ObligationHasDept,
    ///
    #[error("Obligation reserves are full")]
    ObligationReservesFull,
    ///
    #[error("Obligation collateral not found")]
    ObligationCollateralNotFound,
    ///
    #[error("Obligation collateral insufficient")]
    ObligationCollateralInsufficient,
    ///
    #[error("Obligation loan not found")]
    ObligationLoanNotFound,
    ///
    #[error("Obligation collateral index is invalid")]
    ObligationInvalidCollateralIndex,
    ///
    #[error("Obligation loan index is invalid")]
    ObligationInvalidLoanIndex,
    ///
    #[error("Obligation replace collateral already exists`")]
    ObligationReplaceCollateralExists,
    ///
    #[error("User Obligation friend is invalid")]
    ObligationInvalidFriend,
    ///
    #[error("Repaying liquidity amount is too much")]
    RepayTooMuch,
    /// Token approve failed
    #[error("Token approve failed")]
    TokenApproveFailed,
    /// Token burn failed
    #[error("Token burn failed")]
    TokenBurnFailed,
    /// Token initialize mint failed
    #[error("Token initialize mint failed")]
    TokenInitializeMintFailed,
    /// Token initialize account failed
    #[error("Token initialize account failed")]
    TokenInitializeAccountFailed,
    /// Token transfer failed
    #[error("Token transfer failed")]
    TokenTransferFailed,
    /// Token mint to failed
    #[error("Token mint to failed")]
    TokenMintToFailed,
    /// Token approve failed
    #[error("Token revoke failed")]
    TokenRevokeFailed,
}

impl From<LendingError> for ProgramError {
    fn from(e: LendingError) -> Self {
        ProgramError::Custom(e as u32)
    }
}

impl<T> DecodeError<T> for LendingError {
    fn type_of() -> &'static str {
        "Lending Error"
    }
}