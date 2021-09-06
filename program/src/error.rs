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
    /// Invalid instruction data passed in.
    #[error("Failed to unpack instruction data")]
    InstructionUnpackError,
    /// The account cannot be initialized because it is already in use.
    #[error("Account is already initialized")]
    AlreadyInitialized,
    /// Lamport balance below rent-exempt threshold.
    #[error("Lamport balance below rent-exempt threshold")]
    NotRentExempt,
    /// The owner of the input isn't set to the program address generated by the program.
    #[error("Input account owner is not the program address")]
    InvalidAccountOwner,
    /// Expected a different SPL Token program
    #[error("Input token program account is not valid")]
    InvalidTokenProgram,
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
    #[error("Price oracle is invalid")]
    InvalidPriceOracle,
    ///
    #[error("Rate oracle is invalid")]
    InvalidRateOracle,
    ///
    #[error("Invalid liquidity config")]
    InvalidLiquidityConfig,
    ///
    #[error("Invalid collateral config")]
    InvalidCollateralConfig,
    ///
    #[error("Invalid indexed collateral config")]
    InvalidIndexedCollateralConfig,
    ///
    #[error("Invalid indexed loan config")]
    InvalidIndexedLoanConfig,
    ///
    #[error("Invalid rate oracle config")]
    InvalidRateOracleConfig,
    /// Oracle config is invalid
    #[error("Price oracle config is invalid")]
    InvalidPriceOracleConfig,
    // 10
    /// Invalid amount, must be greater than zero
    #[error("Input amount is invalid")]
    InvalidAmount,
    /// Invalid config value
    #[error("Input account must be a signer")]
    InvalidSigner,
    ///
    #[error("Input flash loan program is invalid")]
    InvalidFlashLoanProgram,
    /// Math operation overflow
    #[error("Math operation overflow")]
    MathOverflow,

    // 15
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
    /// Token burn failed
    #[error("Token burn failed")]
    TokenBurnFailed,

    ///
    #[error("COption unpack error")]
    COptionUnpackError,

    // 25
    /// Liquidation repay amount too small
    #[error("Liquidation repaying liquidity amount is too small")]
    LiquidationRepayTooSmall,
    /// Liquidation repay amount too small
    #[error("Liquidation repaying liquidity amount is too much")]
    LiquidationRepayTooMuch,
    // 30
    ///
    #[error("Obligation is already in binding")]
    ObligationAlreadyBindFriend,
    /// Obligation state stale
    #[error("Obligation state needs to be refreshed")]
    ObligationStale,

    // 40
    /// Negative interest rate
    #[error("Interest rate is negative")]
    NegativeInterestRate,
    ///
    #[error("Obligation collaterals not matched")]
    ObligationCollateralsNotMatched,
    ///
    #[error("Obligation loans not matched")]
    ObligationLoansNotMatched,
    ///
    #[error("Obligation collaterals are not healthy")]
    ObligationNotHealthy,
    ///
    #[error("Obligation has dept")]
    ObligationHasDept,
    ///
    #[error("Obligation liquidation is not available")]
    ObligationCanNotLiquidate,
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
    #[error("Obligation invalid collateral index")]
    ObligationInvalidCollateralIndex,
    ///
    #[error("Obligation invalid loan index")]
    ObligationInvalidLoanIndex,

    ///
    #[error("Market Reserve is disabled")]
    MarketReserveDisabled,
    ///
    #[error("Market Reserve deposit too much")]
    MarketReserveDepositTooMuch,
    ///
    #[error("Market Reserve accumulative deposit too much")]
    MarketReserveAccDepositTooMuch,
    ///
    #[error("Market Reserve liquidity available insufficient")]
    MarketReserveInsufficentLiquidity,
    ///
    #[error("Market Reserve is stale")]
    MarketReserveStale,
    ///
    #[error("Flash loan repay insufficient")]
    FlashLoanRepayInsufficient,
    ///
    #[error("Rate Oracle is not available")]
    RateOracleNotAvailable,
    ///
    #[error("Invalid soToken mint info")]
    InvalidSoTokenMint,
    ///
    #[error("User Obligation friend is invalid")]
    ObligationInvalidFriend,
    ///
    #[cfg(feature = "case-injection")]
    #[error("Undefined case injection")]
    UndefinedCaseInjection,
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