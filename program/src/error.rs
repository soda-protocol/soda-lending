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
    // 0
    /// Invalid instruction data passed in.
    #[error("Failed to unpack instruction data")]
    InstructionUnpackError,
    /// The account cannot be initialized because it is already in use.
    #[error("Account is already initialized")]
    AlreadyInitialized,
    /// Lamport balance below rent-exempt threshold.
    #[error("Lamport balance below rent-exempt threshold")]
    NotRentExempt,
    /// The program address provided doesn't match the value generated by the program.
    #[error("Market authority is invalid")]
    InvalidMarketAuthority,
    /// Expected a different market owner
    #[error("Market owner is invalid")]
    InvalidMarketOwner,

    // 5
    /// The owner of the input isn't set to the program address generated by the program.
    #[error("Input account owner is not the program address")]
    InvalidAccountOwner,
    /// The owner of the account input isn't set to the correct token program id.
    #[error("Input token account is not owned by the correct token program id")]
    InvalidTokenOwner,
    /// Expected an SPL Token account
    #[error("Input token account is not valid")]
    InvalidTokenAccount,
    /// Expected an SPL Token mint
    #[error("Input token mint account is not valid")]
    InvalidTokenMint,
    /// Expected a different SPL Token program
    #[error("Input token program account is not valid")]
    InvalidTokenProgram,
    ///
    #[error("Manager is invalid")]
    InvalidManager,
    ///
    #[error("Manager owner is invalid")]
    ///
    InvalidManagerOwner,
    ///
    #[error("Manager authority is invalid")]
    InvalidManagerAuthority,
    ///
    #[error("Invalid manager token account")]
    InvalidManagerTokenAccount,
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
    #[error("User asset owner is invalid")]
    InvalidUserAssetOwner,
    ///
    #[error("Invalid user token account")]
    InvalidUserTokenAccount,
    ///
    #[error("User authority is invalid")]
    InvalidUserAuthority,
    ///
    #[error("Invalid user obligation")]
    InvalidUserObligation,
    ///
    #[error("Rate oracle authority is invalid")]
    InvalidOracleAuthority,
    ///
    #[error("Invalid liquidity config")]
    InvalidLiquidityConfig,
    ///
    #[error("Invalid collateral config")]
    InvalidCollateralConfig,
    // 10
    /// Invalid amount, must be greater than zero
    #[error("Input amount is invalid")]
    InvalidAmount,
    /// Invalid config value
    #[error("Input config value is invalid")]
    InvalidConfig,
    /// Invalid config value
    #[error("Input account must be a signer")]
    InvalidSigner,
    /// Invalid account input
    #[error("Invalid account input")]
    InvalidAccountInput,
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

    // 20
    /// Insufficient liquidity available
    #[error("Insufficient liquidity available")]
    InsufficientLiquidity,
    /// This reserve's collateral cannot be used for borrows
    #[error("Input reserve has collateral disabled")]
    ReserveCollateralDisabled,
    /// Reserve state stale
    #[error("Reserve state needs to be refreshed")]
    ReserveStale,
    /// Withdraw amount too small
    #[error("Withdraw amount too small")]
    WithdrawTooSmall,
    /// Withdraw amount too large
    #[error("Withdraw amount too large")]
    WithdrawTooLarge,

    // 25
    /// Borrow amount too small
    #[error("Borrow amount too small to receive liquidity after fees")]
    BorrowTooSmall,
    /// Borrow amount too large
    #[error("Borrow amount too large for deposited collateral")]
    BorrowTooLarge,
    /// Repay amount too small
    #[error("Repay amount too small to transfer liquidity")]
    RepayTooSmall,
    /// Liquidation amount too small
    #[error("Liquidation amount too small to receive collateral")]
    LiquidationTooSmall,
    /// Cannot liquidate healthy obligations
    #[error("Cannot liquidate healthy obligations")]
    ObligationHealthy,

    // 30
    /// Obligation state stale
    #[error("Obligation state needs to be refreshed")]
    ObligationStale,
    /// Obligation reserve limit exceeded
    #[error("Obligation reserve limit exceeded")]
    ObligationReserveLimit,
    /// Expected a different obligation owner
    #[error("Obligation owner is invalid")]
    InvalidObligationOwner,
    /// Obligation deposits are empty
    #[error("Obligation deposits are empty")]
    ObligationDepositsEmpty,
    /// Obligation borrows are empty
    #[error("Obligation borrows are empty")]
    ObligationBorrowsEmpty,

    // 35
    /// Obligation deposits have zero value
    #[error("Obligation deposits have zero value")]
    ObligationDepositsZero,
    /// Obligation borrows have zero value
    #[error("Obligation borrows have zero value")]
    ObligationBorrowsZero,
    /// Invalid obligation collateral
    #[error("Invalid obligation collateral")]
    InvalidObligationCollateral,
    /// Invalid obligation liquidity
    #[error("Invalid obligation liquidity")]
    InvalidObligationLiquidity,
    /// Obligation collateral is empty
    #[error("Obligation collateral is empty")]
    ObligationCollateralEmpty,

    // 40
    /// Obligation liquidity is empty
    #[error("Obligation liquidity is empty")]
    ObligationLiquidityEmpty,
    /// Negative interest rate
    #[error("Interest rate is negative")]
    NegativeInterestRate,
    /// Oracle config is invalid
    #[error("Input oracle config is invalid")]
    InvalidOracleConfig,
    /// Not enough liquidity after flash loan
    #[error("Not enough liquidity after flash loan")]
    NotEnoughLiquidityAfterFlashLoan,
    ///
    #[error("Obligation collaterals not matched")]
    ObligationCollateralsNotMatched,
    ///
    #[error("Obligation collaterals are not empty")]
    ObligationCollateralsNotEmpty,
    ///
    #[error("Obligation collaterals are not healthy")]
    ObligationNotHealthy,
    ///
    #[error("Obligation liquidation is not available")]
    ObligationLiquidationNotAvailable,
    ///
    #[error("Obligation collaterals limit exceeded")]
    ObligationCollateralsLimitExceed,
    ///
    #[error("Obligation collateral not found")]
    ObligationCollateralNotFound,
    ///
    #[error("Obligation collateral index exceeded")]
    ObligationCollateralIndexExceed,
    ///
    #[error("Obligation collateral amount insufficient")]
    ObligationCollateralAmountInsufficient,
    ///
    #[error("Obligation dept amount insufficient")]
    ObligationRepayTooMuch,
    ///
    #[error("Market Reserve liquidity does not exist")]
    MarketReserveLiquidityNotExist,
    ///
    #[error("Market Reserve liquidity utilization rate exceeds limit")]
    MarketReserveLiquidityUtilizationTooLarge,
    ///
    #[error("Market Reserve liquidity available insufficient")]
    MarketReserveLiquidityAvailableInsufficent,
    ///
    #[error("Market Reserve liquidity fee insufficient")]
    MarketReserveLiquidityFeeInsufficent,
    ///
    #[error("Market Reserve collateral insufficient")]
    MarketReserveCollateralInsufficent,
    ///
    #[error("Market Reserve is stale")]
    MarketReserveStale,
    ///
    #[error("Market Reserve already has liquidity property")]
    MarketReserveLiquidityAlreadyExist,
    ///
    #[error("User Asset insufficient")]
    UserAssetInsufficient,
    ///
    #[error("Rate Oracle is not available")]
    RateOracleNotAvailable,
    ///
    #[error("Liquidation too much")]
    LiquidationTooMuch,
    ///
    #[error("Price oracle not matched")]
    PriceOracleNotMatched,


    ///
    #[error("Debug error 1")]
    DebugError1,
    /// 
    #[error("Debug error 2")]
    DebugError2,
    ///
    #[error("Debug error 3")]
    DebugError3,
    ///
    #[error("Debug error 4")]
    DebugError4,
    ///
    #[error("Debug error 5")]
    DebugError5,
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