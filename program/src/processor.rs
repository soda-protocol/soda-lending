//! Program state processor
use std::convert::TryInto;
use crate::{
    error::LendingError,
    instruction::LendingInstruction,
    math::{Rate, Decimal, TryDiv, TryMul},
    pyth,
    state::{
        CollateralConfig, CollateralInfo, Collateral, LastUpdate, Liquidity, LiquidityConfig, RateOracle,
        LiquidityInfo, Manager, MarketReserve, PROGRAM_VERSION, TokenInfo, UserAsset, UserObligation,
        PriceInfo, calculate_decimals, calculate_interest_fee, calculate_liquidation_fee,
    },
};
use num_traits::FromPrimitive;
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    decode_error::DecodeError,
    entrypoint::ProgramResult,
    msg,
    program::{invoke, invoke_signed},
    program_error::{PrintProgramError, ProgramError},
    program_option::COption,
    program_pack::{IsInitialized, Pack},
    pubkey::Pubkey,
    sysvar::{clock::Clock, rent::Rent, Sysvar},
};
use spl_token::{check_program_account, state::{Account, Mint}};

/// Processes an instruction
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    input: &[u8],
) -> ProgramResult {
    let instruction = LendingInstruction::unpack(input)?;
    match instruction {
        LendingInstruction::InitManager { quote_currency } => {
            msg!("Instruction: Init Lending Manager");
            process_init_manager(program_id, accounts, quote_currency)
        }
        LendingInstruction::InitRateOracle => {
            msg!("Instruction: Init Rate Oracle");
            process_init_rate_oracle(program_id, accounts)
        }
        LendingInstruction::InitMarketReserveWithoutLiquidity { collateral_config } => {
            msg!("Instruction: Init Market Reserve Without Liquidity");
            process_init_market_reserve(program_id, accounts, collateral_config, None)
        }
        LendingInstruction::InitMarketReserveWithLiquidity {
            collateral_config,
            liquidity_config,
        } => {
            msg!("Instruction: Init Market Reserve With Liquidity");
            process_init_market_reserve(program_id, accounts, collateral_config, Some(liquidity_config))
        }
        LendingInstruction::InitUserObligation => {
            msg!("Instruction: Init User Obligation");
            process_init_user_obligation(program_id, accounts)
        }
        LendingInstruction::InitUserAsset => {
            msg!("Instruction: Init User Asset");
            process_init_user_asset(program_id, accounts)
        }
        LendingInstruction::DepositLiquidity { amount } => {
            msg!("Instruction: Deposit Liquidity: {}", amount);
            process_deposit_liquidity(program_id, accounts, amount)
        }
        LendingInstruction::WithdrawLiquidity { amount } => {
            msg!("Instruction: Withdraw Liquidity: {}", amount);
            process_withdraw_liquidity(program_id, accounts, amount)
        }
        LendingInstruction::DepositCollateral { amount } => {
            msg!("Instruction: Deposit Collateral: {}", amount);
            process_deposit_collateral(program_id, accounts, amount)
        }
        LendingInstruction::UpdateUserObligation => {
            msg!("Instruction: Update User Obligation");
            process_update_user_obligation(program_id, accounts)
        }
        LendingInstruction::BorrowLiquidity { amount } => {
            msg!("Instruction: Borrow Collateral: {}", amount);
            process_borrow_liquidity(program_id, accounts, amount)
        }
        LendingInstruction::RepayLoan { amount } => {
            msg!("Instruction: Repay Loan: {}", amount);
            process_repay_loan(program_id, accounts, amount)
        }
        LendingInstruction::RedeemCollateral { amount } => {
            msg!("Instruction: Redeem Collateral: {}", amount);
            process_redeem_collateral(program_id, accounts, amount)
        }
        LendingInstruction::Liquidate { is_arbitrary, amount } => {
            msg!("Instruction: Liquidation: amount = {}, is arbitrary = {}", amount, is_arbitrary);
            process_liquidate(program_id, accounts, is_arbitrary, amount)
        }
        LendingInstruction::FeedRateOracle { interest_rate, borrow_rate } => {
            msg!("Instruction: Feed Rate Oracle: interest rate = {}, borrow rate = {}", interest_rate, borrow_rate);
            process_feed_rate_oracle(program_id, accounts, interest_rate, borrow_rate)
        }
        LendingInstruction::PauseRateOracle => {
            msg!("Instruction: Pause Rate Oracle");
            process_pause_rate_oracle(program_id, accounts)
        }
        LendingInstruction::AddLiquidityToReserve { liquidity_config } => {
            msg!("Instruction: Add Liquidity Property To Market Reserve");
            process_add_liquidity_to_market_reserve(program_id, accounts, liquidity_config)
        }
        LendingInstruction::WithdrawFee { amount } => {
            msg!("Instruction: Withdraw Fee: {}", amount);
            process_withdraw_fee(program_id, accounts, amount)
        }
    }
}

fn process_init_manager(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    quote_currency: [u8; 32],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let rent = &Rent::from_account_info(next_account_info(account_info_iter)?)?;
    let manager_info = next_account_info(account_info_iter)?;
    let owner_info = next_account_info(account_info_iter)?;
    let oracle_program_id = next_account_info(account_info_iter)?;
    let token_program_id = next_account_info(account_info_iter)?;

    if manager_info.owner != program_id {
        msg!("manager provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    assert_rent_exempt(rent, manager_info)?;
    assert_uninitialized::<Manager>(manager_info)?;

    check_program_account(&token_program_id.key)?;
    
    let manager = Manager{
        version: PROGRAM_VERSION,
        bump_seed: Pubkey::find_program_address(&[manager_info.key.as_ref()], program_id).1,
        owner: *owner_info.key,
        quote_currency,
        token_program_id: *token_program_id.key,
        pyth_program_id: *oracle_program_id.key,
    };
    Manager::pack(manager, &mut manager_info.try_borrow_mut_data()?)
}

fn process_init_rate_oracle(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let rent = &Rent::from_account_info(next_account_info(account_info_iter)?)?;
    let rate_oracle_info = next_account_info(account_info_iter)?;
    let owner_info = next_account_info(account_info_iter)?;

    if rate_oracle_info.owner != program_id {
        msg!("Rate oracle owner provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    assert_rent_exempt(rent, rate_oracle_info)?;
    assert_uninitialized::<RateOracle>(rate_oracle_info)?;

    let rate_oracle = RateOracle {
        version: PROGRAM_VERSION,
        owner: *owner_info.key,
        status: false,
        timestamp: 0,
        interest_rate: 0,
        borrow_rate: 0,
    };
    RateOracle::pack(rate_oracle, &mut rate_oracle_info.try_borrow_mut_data()?)
}

fn process_init_market_reserve(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    collateral_config: CollateralConfig,
    liquidity_config: Option<LiquidityConfig>,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let rent = &Rent::from_account_info(next_account_info(account_info_iter)?)?;
    let clock = &Clock::from_account_info(next_account_info(account_info_iter)?)?;
    let manager_info = next_account_info(account_info_iter)?;
    let market_reserve_info = next_account_info(account_info_iter)?;
    let pyth_product_info = next_account_info(account_info_iter)?;
    let pyth_price_info = next_account_info(account_info_iter)?;
    let token_mint_info = next_account_info(account_info_iter)?;
    let token_account_info = next_account_info(account_info_iter)?;
    let authority_info = next_account_info(account_info_iter)?;

    if manager_info.owner != program_id {
        msg!("Manager ower provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let manager = Manager::unpack(&manager_info.try_borrow_data()?)?;
    let manager_authority = &Pubkey::create_program_address(&[manager_info.key.as_ref(),
        &[manager.bump_seed]], program_id)?;

    if market_reserve_info.owner != program_id {
        msg!("MarketReserve owner provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    assert_rent_exempt(rent, market_reserve_info)?;
    assert_uninitialized::<MarketReserve>(market_reserve_info)?;

    if pyth_product_info.owner != &manager.pyth_program_id {
        msg!("Pyth product account provided is not owned by the pyth program");
        return Err(LendingError::InvalidOracleConfig.into());
    }
    let pyth_product_data = pyth_product_info.try_borrow_data()?;
    let pyth_product = pyth::load::<pyth::Product>(&pyth_product_data)
        .map_err(|_| ProgramError::InvalidAccountData)?;
    if pyth_product.magic != pyth::MAGIC {
        msg!("Pyth product account provided is not a valid Pyth account");
        return Err(LendingError::InvalidOracleConfig.into());
    }
    if pyth_product.ver != pyth::VERSION_2 {
        msg!("Pyth product account provided has a different version than expected");
        return Err(LendingError::InvalidOracleConfig.into());
    }
    if pyth_product.atype != pyth::AccountType::Product as u32 {
        msg!("Pyth product account provided is not a valid Pyth product account");
        return Err(LendingError::InvalidOracleConfig.into());
    }
    let quote_currency = get_pyth_product_quote_currency(pyth_product)?;
    if manager.quote_currency != quote_currency {
        msg!("Lending market quote currency does not match the oracle quote currency");
        return Err(LendingError::InvalidOracleConfig.into());
    }

    if pyth_price_info.owner != &manager.pyth_program_id {
        msg!("Pyth price account provided is not owned by the lending market oracle program");
        return Err(LendingError::InvalidOracleConfig.into());
    }
    let pyth_price_pubkey_bytes: &[u8; 32] = pyth_price_info.key
        .as_ref()
        .try_into()
        .map_err(|_| LendingError::InvalidAccountInput)?;
    if pyth_price_pubkey_bytes != &pyth_product.px_acc.val {
        msg!("Pyth product price account does not match the Pyth price provided");
        return Err(LendingError::InvalidOracleConfig.into());
    }

    if token_mint_info.owner != &manager.token_program_id {
        msg!("Token mint info owner provided is not owned by the token program in manager");
        return Err(LendingError::InvalidTokenProgram.into()); 
    }
    let token_mint = Mint::unpack(&token_mint_info.try_borrow_data()?)?;

    if token_account_info.owner != &manager.token_program_id {
        msg!("Token account info owner provided is not owned by the token program in manager");
        return Err(LendingError::InvalidTokenProgram.into());
    }
    let token_account = Account::unpack(&token_account_info.try_borrow_data()?)?;
    if &token_account.owner != manager_authority {
        msg!("Token account owner is not matched with manager authority");
        return Err(LendingError::InvalidTokenAccount.into());
    }

    if authority_info.key != &manager.owner {
        msg!("Only manager owner can create reserve");
        return Err(LendingError::InvalidManagerOwner.into());
    }
    if !authority_info.is_signer {
        msg!("authority is not a signer");
        return Err(LendingError::InvalidSigner.into());
    }

    let liquidity_info = if let Some(liquidity_config) = liquidity_config {
        let rate_oracle_info = next_account_info(account_info_iter)?;
        if rate_oracle_info.owner != program_id {
            return Err(LendingError::InvalidAccountOwner.into());
        }
        RateOracle::unpack(&rate_oracle_info.try_borrow_data()?)?;

        COption::Some(LiquidityInfo{
            rate_oracle: *rate_oracle_info.key,
            liquidity: Liquidity::default(),
            config: liquidity_config,
        })
    } else {
        COption::None
    };

    let market_reserve = MarketReserve{
        version: PROGRAM_VERSION,
        timestamp: clock.slot,
        manager: *manager_info.key,
        token_info: TokenInfo{
            account: *token_account_info.key,
            price_oracle: *pyth_price_info.key,
            decimal: token_mint.decimals,
        },
        liquidity_info,
        collateral_info: CollateralInfo{
            amount: 0,
            config: collateral_config,
        },
    };
    MarketReserve::pack(market_reserve, &mut market_reserve_info.try_borrow_mut_data()?)
}

fn process_init_user_obligation(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let rent = &Rent::from_account_info(next_account_info(account_info_iter)?)?;
    let clock = &Clock::from_account_info(next_account_info(account_info_iter)?)?;
    let market_reserve_info = next_account_info(account_info_iter)?;
    let user_obligation_info = next_account_info(account_info_iter)?;
    let owner_info = next_account_info(account_info_iter)?;

    if market_reserve_info.owner != program_id {
        msg!("MarketReserve owner provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let market_reserve = MarketReserve::unpack(&market_reserve_info.try_borrow_data()?)?;
    if market_reserve.liquidity_info.is_none() {
        msg!("MarketReserve liquidity is not available");
        return Err(LendingError::MarketReserveLiquidityNotExist.into());
    }

    if user_obligation_info.owner != program_id {
        msg!("UserObligation owner provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    assert_rent_exempt(rent, user_obligation_info)?;
    assert_uninitialized::<UserObligation>(user_obligation_info)?;

    let user_obligation = UserObligation{
        version: PROGRAM_VERSION,
        reserve: *market_reserve_info.key,
        owner: *owner_info.key,
        last_update: LastUpdate::new(clock.slot),
        collaterals: Vec::new(),
        collaterals_value: (Decimal::default(), Decimal::default(), Decimal::default()),
        loan_market_price: Decimal::default(),
        borrowed_amount: 0,
        dept_amount: 0,
    };
    UserObligation::pack(user_obligation, &mut user_obligation_info.try_borrow_mut_data()?)
}

fn process_init_user_asset(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let rent = &Rent::from_account_info(next_account_info(account_info_iter)?)?;
    let clock = &Clock::from_account_info(next_account_info(account_info_iter)?)?;
    let market_reserve_info = next_account_info(account_info_iter)?;
    let user_asset_info = next_account_info(account_info_iter)?;
    let owner_info = next_account_info(account_info_iter)?;

    if market_reserve_info.owner != program_id {
        msg!("MarketReserve owner provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let market_reserve = MarketReserve::unpack(&market_reserve_info.try_borrow_data()?)?;
    if market_reserve.liquidity_info.is_none() {
        msg!("MarketReserve liquidity is not available");
        return Err(LendingError::MarketReserveLiquidityNotExist.into());
    }

    if user_asset_info.owner != program_id {
        msg!("UserAsset owner provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    assert_rent_exempt(rent, user_asset_info)?;
    assert_uninitialized::<UserAsset>(user_asset_info)?;

    let user_asset = UserAsset{
        version: PROGRAM_VERSION,
        reserve: *market_reserve_info.key,
        owner: *owner_info.key,
        timestamp: clock.slot,
        principle_amount: 0,
        total_amount: 0,
    };
    UserAsset::pack(user_asset, &mut user_asset_info.try_borrow_mut_data()?)
}

fn process_deposit_liquidity(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    amount: u64,
) -> ProgramResult {
    if amount == 0 {
        msg!("Liquidity amount provided cannot be zero");
        return Err(LendingError::InvalidAmount.into());
    }

    let account_info_iter = &mut accounts.iter();
    let clock = &Clock::from_account_info(next_account_info(account_info_iter)?)?;
    let market_reserve_info = next_account_info(account_info_iter)?;
    let manager_token_account_info = next_account_info(account_info_iter)?;
    let rate_oracle_info = next_account_info(account_info_iter)?;
    let user_asset_info = next_account_info(account_info_iter)?;
    let user_authority_info = next_account_info(account_info_iter)?;
    let user_token_account_info = next_account_info(account_info_iter)?;
    let token_program_id = next_account_info(account_info_iter)?;

    if market_reserve_info.owner != program_id {
        msg!("MarketReserve owner provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut market_reserve = MarketReserve::unpack(&market_reserve_info.try_borrow_data()?)?;
    let liquidity_info = market_reserve.liquidity_info
        .as_mut()
        .ok_or(LendingError::MarketReserveLiquidityNotExist)?;

    if manager_token_account_info.key != &market_reserve.token_info.account {
        return Err(LendingError::InvalidManagerTokenAccount.into()); 
    }

    if rate_oracle_info.key != &liquidity_info.rate_oracle {
        msg!("MarketReserve liquidity rate oracle is not matched with provided");
        return Err(LendingError::InvalidRateOracle.into());
    }
    let rate_oracle = RateOracle::unpack(&rate_oracle_info.try_borrow_data()?)?;
    rate_oracle.check_valid(clock.slot)?;

    if user_asset_info.owner != program_id {
        msg!("UserAsset owner provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut user_asset = UserAsset::unpack(&user_asset_info.try_borrow_data()?)?;
    if &user_asset.reserve != market_reserve_info.key {
        msg!("UserAsset market reserve is not matched with accounts provided");
        return Err(LendingError::InvalidMarketReserve.into());
    }

    if user_authority_info.key != &user_asset.owner {
        return Err(LendingError::InvalidUserAuthority.into());
    }

    // 1. update
    user_asset.update_interest(clock.slot, Rate::from_scaled_val(rate_oracle.interest_rate))?;
    // 2. deposit in obligation
    user_asset.deposit(amount)?;
    // 3. deposit in market reserve
    liquidity_info.liquidity.deposit(amount)?;
    // 4. update timestamp
    market_reserve.timestamp = clock.slot;
    user_asset.timestamp = clock.slot;
    // 5. pack data
    MarketReserve::pack(market_reserve, &mut market_reserve_info.try_borrow_mut_data()?)?;
    UserAsset::pack(user_asset, &mut user_asset_info.try_borrow_mut_data()?)?;
    // 6. transfer from user to manager
    spl_token_transfer(TokenTransferParams {
        source: user_token_account_info.clone(),
        destination: manager_token_account_info.clone(),
        amount,
        authority: user_authority_info.clone(),
        authority_signer_seeds: &[],
        token_program: token_program_id.clone(),
    })
}

fn process_withdraw_liquidity(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    amount: u64,
) -> ProgramResult {
    if amount == 0 {
        msg!("Liquidity amount provided cannot be zero");
        return Err(LendingError::InvalidAmount.into());
    }

    let account_info_iter = &mut accounts.iter();
    let clock = &Clock::from_account_info(next_account_info(account_info_iter)?)?;
    let manager_info = next_account_info(account_info_iter)?;
    let manager_authority_info = next_account_info(account_info_iter)?;
    let market_reserve_info = next_account_info(account_info_iter)?;
    let manager_token_account_info = next_account_info(account_info_iter)?;
    let rate_oracle_info = next_account_info(account_info_iter)?;
    let user_asset_info = next_account_info(account_info_iter)?;
    let user_authority_info = next_account_info(account_info_iter)?;
    let user_token_account_info = next_account_info(account_info_iter)?;
    let token_program_id = next_account_info(account_info_iter)?;

    if manager_info.owner != program_id {
        msg!("Manager ower provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let manager = Manager::unpack(&manager_info.try_borrow_data()?)?;
    let authority_signer_seeds = &[
        manager_info.key.as_ref(),
        &[manager.bump_seed]
    ];
    let manager_authority = Pubkey::create_program_address(authority_signer_seeds, program_id)?;

    if manager_authority_info.key != &manager_authority {
        msg!("Manager authority is not matched with program address derived from manager info");
        return Err(LendingError::InvalidManagerAuthority.into());
    }

    if market_reserve_info.owner != program_id {
        msg!("MarketReserve owner provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut market_reserve = MarketReserve::unpack(&market_reserve_info.try_borrow_data()?)?;
    if &market_reserve.manager != manager_info.key {
        msg!("MarketReserve manager provided is not matched with manager info");
        return Err(LendingError::InvalidMarketReserve.into());
    }
    let liquidity_info = market_reserve.liquidity_info
        .as_mut()
        .ok_or(LendingError::MarketReserveLiquidityNotExist)?;

    if manager_token_account_info.key != &market_reserve.token_info.account {
        return Err(LendingError::InvalidManagerTokenAccount.into()); 
    }

    if rate_oracle_info.key != &liquidity_info.rate_oracle {
        msg!("MarketReserve liquidity rate oracle is not matched with provided");
        return Err(LendingError::InvalidRateOracle.into());
    }
    let rate_oracle = RateOracle::unpack(&rate_oracle_info.try_borrow_data()?)?;
    rate_oracle.check_valid(clock.slot)?;

    if user_asset_info.owner != program_id {
        msg!("UserAsset owner provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut user_asset = UserAsset::unpack(&user_asset_info.try_borrow_data()?)?;
    if &user_asset.reserve != market_reserve_info.key {
        msg!("UserAsset market reserve is not matched with accounts provided");
        return Err(LendingError::InvalidMarketReserve.into());
    }

    if user_authority_info.key != &user_asset.owner {
        return Err(LendingError::InvalidUserAuthority.into());
    }
    if !user_authority_info.is_signer {
        msg!("authority is not a signer");
        return Err(LendingError::InvalidSigner.into());
    }

    // 1. update
    user_asset.update_interest(clock.slot, Rate::from_scaled_val(rate_oracle.interest_rate))?;
    // 2. withdraw
    let fund = user_asset.withdraw(amount)?;
    // 3. withdraw in market reserve
    let fee = calculate_interest_fee(fund.interest, Rate::from_scaled_val(liquidity_info.config.interest_fee_rate))?;
    liquidity_info.liquidity.withdraw(&fund, fee)?;
    // 4. update timestamp
    market_reserve.timestamp = clock.slot;
    user_asset.timestamp = clock.slot;
    // 4. pack data
    MarketReserve::pack(market_reserve, &mut market_reserve_info.try_borrow_mut_data()?)?;
    UserAsset::pack(user_asset, &mut user_asset_info.try_borrow_mut_data()?)?;
    // 5. transfer from manager to user
    spl_token_transfer(TokenTransferParams {
        source: manager_token_account_info.clone(),
        destination: user_token_account_info.clone(),
        amount,
        authority: manager_authority_info.clone(),
        authority_signer_seeds,
        token_program: token_program_id.clone(),
    })
}

fn process_deposit_collateral(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    amount: u64,
) -> ProgramResult {
    if amount == 0 {
        msg!("Collateral amount provided cannot be zero");
        return Err(LendingError::InvalidAmount.into());
    }

    let account_info_iter = &mut accounts.iter();
    let clock = &Clock::from_account_info(next_account_info(account_info_iter)?)?;
    let market_reserve_info = next_account_info(account_info_iter)?;
    let manager_token_account_info = next_account_info(account_info_iter)?;
    let user_obligatiton_info = next_account_info(account_info_iter)?;
    let user_authority_info = next_account_info(account_info_iter)?;
    let user_token_account_info = next_account_info(account_info_iter)?;
    let token_program_id = next_account_info(account_info_iter)?;

    if market_reserve_info.owner != program_id {
        msg!("Market reserve owner provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut market_reserve = MarketReserve::unpack(&market_reserve_info.try_borrow_data()?)?;

    if manager_token_account_info.key != &market_reserve.token_info.account {
        return Err(LendingError::InvalidManagerTokenAccount.into()); 
    }

    if user_obligatiton_info.owner != program_id {
        msg!("User Obligation owner provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut user_obligation = UserObligation::unpack(&user_obligatiton_info.try_borrow_data()?)?;

    if user_authority_info.key != &user_obligation.owner {
        return Err(LendingError::InvalidUserAuthority.into());
    }

    // handle obligation
    let price_oracle = &market_reserve.token_info.price_oracle;
    if let Ok(index) = user_obligation.find_collateral(price_oracle) {
        user_obligation.deposit(index, amount)?;
    } else {
        user_obligation.new_deposit(Collateral{
            price_oracle: price_oracle.clone(),
            decimal: market_reserve.token_info.decimal,
            borrow_value_ratio: market_reserve.collateral_info.config.borrow_value_ratio,
            liquidation_value_ratio: market_reserve.collateral_info.config.liquidation_value_ratio,
            amount,
        })?;
    }
    user_obligation.last_update.update_slot(clock.slot, true);
    UserObligation::pack(user_obligation, &mut user_obligatiton_info.try_borrow_mut_data()?)?;
    
    // handle market reserve
    market_reserve.collateral_info.deposit(amount)?;
    market_reserve.timestamp = clock.slot;
    MarketReserve::pack(market_reserve, &mut market_reserve_info.try_borrow_mut_data()?)?;
    
    // transfer from user to manager
    spl_token_transfer(TokenTransferParams {
        source: user_token_account_info.clone(),
        destination: manager_token_account_info.clone(),
        amount,
        authority: user_authority_info.clone(),
        authority_signer_seeds: &[],
        token_program: token_program_id.clone(),
    })
}

fn process_update_user_obligation(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let clock = &Clock::from_account_info(next_account_info(account_info_iter)?)?;
    let market_reserve_info = next_account_info(account_info_iter)?;
    let liquidity_price_oracle_info = next_account_info(account_info_iter)?;
    let rate_oracle_info = next_account_info(account_info_iter)?;
    let user_obligatiton_info = next_account_info(account_info_iter)?;

    if market_reserve_info.owner != program_id {
        msg!("Market reserve owner provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let market_reserve = MarketReserve::unpack(&market_reserve_info.try_borrow_data()?)?;
    let liquidity_info = market_reserve.liquidity_info
        .as_ref()
        .ok_or(LendingError::MarketReserveLiquidityNotExist)?;

    if liquidity_price_oracle_info.key != &market_reserve.token_info.price_oracle {
        return Err(LendingError::InvalidPriceOracle.into());
    }
    let liquidity_price = get_pyth_price(liquidity_price_oracle_info, clock)?;

    if rate_oracle_info.key != &liquidity_info.rate_oracle {
        return Err(LendingError::InvalidRateOracle.into());
    }
    let rate_oracle = RateOracle::unpack(&rate_oracle_info.try_borrow_data()?)?;
    rate_oracle.check_valid(clock.slot)?;

    if user_obligatiton_info.owner != program_id {
        msg!("User Obligation owner provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut user_obligation = UserObligation::unpack(&user_obligatiton_info.try_borrow_data()?)?;
    if &user_obligation.reserve != market_reserve_info.key {
        return Err(LendingError::InvalidUserObligation.into());
    }

    // handle obligation
    user_obligation.update_borrow_interest(clock.slot, Rate::from_scaled_val(rate_oracle.borrow_rate))?;
    let collateral_prices = account_info_iter.map(|price_oracle_info| Ok(
        PriceInfo{
            price_oracle: *price_oracle_info.key,
            price: get_pyth_price(price_oracle_info, clock)?,
        }
    )).collect::<Result<Vec<_>, ProgramError>>()?;
    user_obligation.update_temp_data(&collateral_prices, liquidity_price)?;
    drop(collateral_prices);
    user_obligation.last_update.update_slot(clock.slot, false);
    UserObligation::pack(user_obligation, &mut user_obligatiton_info.try_borrow_mut_data()?)
}

// must after update obligation
fn process_borrow_liquidity(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    amount: u64,
) -> ProgramResult {
    if amount == 0 {
        msg!("Liquidity amount provided cannot be zero");
        return Err(LendingError::InvalidAmount.into());
    }

    let account_info_iter = &mut accounts.iter();
    let clock = &Clock::from_account_info(next_account_info(account_info_iter)?)?;
    let manager_info = next_account_info(account_info_iter)?;
    let manager_authority_info = next_account_info(account_info_iter)?;
    let market_reserve_info = next_account_info(account_info_iter)?;
    let manager_token_account_info = next_account_info(account_info_iter)?;
    let user_obligatiton_info = next_account_info(account_info_iter)?;
    let user_authority_info = next_account_info(account_info_iter)?;
    let user_token_account_info = next_account_info(account_info_iter)?;
    let token_program_id = next_account_info(account_info_iter)?;

    if manager_info.owner != program_id {
        msg!("Manager ower provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let manager = Manager::unpack(&manager_info.try_borrow_data()?)?;
    let authority_signer_seeds = &[
        manager_info.key.as_ref(),
        &[manager.bump_seed]
    ];
    let manager_authority = Pubkey::create_program_address(authority_signer_seeds, program_id)?;

    if manager_authority_info.key != &manager_authority {
        msg!("Manager authority is not matched with program address derived from manager info");
        return Err(LendingError::InvalidManagerAuthority.into());
    }

    if market_reserve_info.owner != program_id {
        msg!("Market reserve owner provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut market_reserve = MarketReserve::unpack(&market_reserve_info.try_borrow_data()?)?;
    if &market_reserve.manager != manager_info.key {
        msg!("MarketReserve manager provided is not matched with manager info");
        return Err(LendingError::InvalidMarketReserve.into());
    }
    let liquidity_info = market_reserve.liquidity_info
        .as_mut()
        .ok_or(LendingError::MarketReserveLiquidityNotExist)?;

    if manager_token_account_info.key != &market_reserve.token_info.account {
        return Err(LendingError::InvalidManagerTokenAccount.into()); 
    }

    if user_obligatiton_info.owner != program_id {
        msg!("User Obligation owner provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut user_obligation = UserObligation::unpack(&user_obligatiton_info.try_borrow_data()?)?;
    if &user_obligation.reserve != market_reserve_info.key {
        return Err(LendingError::InvalidUserObligation.into());
    }
    if user_obligation.last_update.is_stale(clock.slot)? {
        return Err(LendingError::ObligationStale.into());
    }

    if user_authority_info.key != &user_obligation.owner {
        return Err(LendingError::InvalidUserAuthority.into());
    }
    if !user_authority_info.is_signer {
        msg!("authority is not a signer");
        return Err(LendingError::InvalidSigner.into());
    }

    // handle user obligation
    user_obligation.borrow_out(amount)?;
    user_obligation.validate_borrow(calculate_decimals(market_reserve.token_info.decimal)?)?;
    user_obligation.last_update.update_slot(clock.slot, true);
    UserObligation::pack(user_obligation, &mut user_obligatiton_info.try_borrow_mut_data()?)?;
    
    // handle market reserve
    liquidity_info.liquidity.borrow_out(amount)?;
    market_reserve.timestamp = clock.slot;
    MarketReserve::pack(market_reserve, &mut market_reserve_info.try_borrow_mut_data()?)?;
    // 7. transfer
    spl_token_transfer(TokenTransferParams {
        source: manager_token_account_info.clone(),
        destination: user_token_account_info.clone(),
        amount,
        authority: manager_authority_info.clone(),
        authority_signer_seeds,
        token_program: token_program_id.clone(),
    })
}

fn process_repay_loan(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    amount: u64,
) -> ProgramResult {
    if amount == 0 {
        msg!("Loan amount provided cannot be zero");
        return Err(LendingError::InvalidAmount.into());
    }

    let account_info_iter = &mut accounts.iter();
    let clock = &Clock::from_account_info(next_account_info(account_info_iter)?)?;
    let market_reserve_info = next_account_info(account_info_iter)?;
    let manager_token_account_info = next_account_info(account_info_iter)?;
    let rate_oracle_info = next_account_info(account_info_iter)?;
    let user_obligatiton_info = next_account_info(account_info_iter)?;
    let user_authority_info = next_account_info(account_info_iter)?;
    let user_token_account_info = next_account_info(account_info_iter)?;
    let token_program_id = next_account_info(account_info_iter)?;

    if market_reserve_info.owner != program_id {
        msg!("Market reserve owner provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut market_reserve = MarketReserve::unpack(&market_reserve_info.try_borrow_data()?)?;

    let liquidity_info = market_reserve.liquidity_info
        .as_mut()
        .ok_or(LendingError::MarketReserveLiquidityNotExist)?;

    if manager_token_account_info.key != &market_reserve.token_info.account {
        return Err(LendingError::InvalidManagerTokenAccount.into())
    }

    if rate_oracle_info.key != &liquidity_info.rate_oracle {
        return Err(LendingError::InvalidRateOracle.into());
    }
    let rate_oracle = RateOracle::unpack(&rate_oracle_info.try_borrow_data()?)?;
    rate_oracle.check_valid(clock.slot)?;

    if user_obligatiton_info.owner != program_id {
        msg!("User Obligation owner provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }

    let mut user_obligation = UserObligation::unpack(&user_obligatiton_info.try_borrow_data()?)?;
    if &user_obligation.reserve != market_reserve_info.key {
        return Err(LendingError::InvalidUserObligation.into());
    }

    if user_authority_info.key != &user_obligation.owner {
        return Err(LendingError::InvalidUserAuthority.into());
    }
    if !user_authority_info.is_signer {
        msg!("authority is not a signer");
        return Err(LendingError::InvalidSigner.into());
    }

    // handle obligation
    user_obligation.update_borrow_interest(clock.slot, Rate::from_scaled_val(rate_oracle.borrow_rate))?;
    let fund = user_obligation.repay(amount)?;
    user_obligation.last_update.update_slot(clock.slot, true);
    UserObligation::pack(user_obligation, &mut user_obligatiton_info.try_borrow_mut_data()?)?;
    // handle market reserve
    liquidity_info.liquidity.repay(&fund)?;
    market_reserve.timestamp = clock.slot;
    MarketReserve::pack(market_reserve, &mut market_reserve_info.try_borrow_mut_data()?)?;
    // 6. transfer
    spl_token_transfer(TokenTransferParams {
        source: user_token_account_info.clone(),
        destination: manager_token_account_info.clone(),
        amount,
        authority: user_authority_info.clone(),
        authority_signer_seeds: &[],
        token_program: token_program_id.clone(),
    })
}

// must after update obligation
fn process_redeem_collateral(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    amount: u64,
) -> ProgramResult {
    if amount == 0 {
        msg!("Liquidity amount provided cannot be zero");
        return Err(LendingError::InvalidAmount.into());
    }

    let account_info_iter = &mut accounts.iter();
    let clock = &Clock::from_account_info(next_account_info(account_info_iter)?)?;
    let manager_info = next_account_info(account_info_iter)?;
    let manager_authority_info = next_account_info(account_info_iter)?;
    let liquidity_market_reserve_info = next_account_info(account_info_iter)?;
    let colleteral_market_reserve_info = next_account_info(account_info_iter)?;
    let manager_token_account_info = next_account_info(account_info_iter)?;
    let user_obligatiton_info = next_account_info(account_info_iter)?;
    let user_authority_info = next_account_info(account_info_iter)?;
    let user_token_account_info = next_account_info(account_info_iter)?;
    let token_program_id = next_account_info(account_info_iter)?;

    if manager_info.owner != program_id {
        msg!("Manager ower provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let manager = Manager::unpack(&manager_info.try_borrow_data()?)?;
    let authority_signer_seeds = &[
        manager_info.key.as_ref(),
        &[manager.bump_seed]
    ];
    let manager_authority = Pubkey::create_program_address(authority_signer_seeds, program_id)?;

    if manager_authority_info.key != &manager_authority {
        msg!("Manager authority is not matched with program address derived from manager info");
        return Err(LendingError::InvalidManagerAuthority.into());
    }

    if liquidity_market_reserve_info.owner != program_id {
        msg!("Liquidity market reserve owner provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let market_reserve = MarketReserve::unpack(&liquidity_market_reserve_info.try_borrow_data()?)?;
    if &market_reserve.manager != manager_info.key {
        msg!("Liquidity market reserve manager provided is not matched with manager info");
        return Err(LendingError::InvalidMarketReserve.into());
    }
    let decimals = calculate_decimals(market_reserve.token_info.decimal)?;

    if colleteral_market_reserve_info.owner != program_id {
        msg!("Collateral market reserve owner provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut market_reserve = MarketReserve::unpack(&colleteral_market_reserve_info.try_borrow_data()?)?;
    if &market_reserve.manager != manager_info.key {
        msg!("MarketReserve manager provided is not matched with manager info");
        return Err(LendingError::InvalidMarketReserve.into());
    }

    if manager_token_account_info.key != &market_reserve.token_info.account {
        return Err(LendingError::InvalidManagerTokenAccount.into()); 
    }

    if user_obligatiton_info.owner != program_id {
        msg!("User Obligation owner provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut user_obligation = UserObligation::unpack(&user_obligatiton_info.try_borrow_data()?)?;
    if &user_obligation.reserve != liquidity_market_reserve_info.key {
        return Err(LendingError::InvalidUserObligation.into());
    }
    if user_obligation.last_update.is_stale(clock.slot)? {
        return Err(LendingError::ObligationStale.into());
    }

    if user_authority_info.key != &user_obligation.owner {
        return Err(LendingError::InvalidUserAuthority.into());
    }
    if !user_authority_info.is_signer {
        msg!("authority is not a signer");
        return Err(LendingError::InvalidSigner.into());
    }

    // handle obligation
    let index = user_obligation.find_collateral(&market_reserve.token_info.price_oracle)?;
    user_obligation.redeem(index, amount)?;
    user_obligation.validate_borrow(decimals)?;
    user_obligation.last_update.update_slot(clock.slot, true);
    UserObligation::pack(user_obligation, &mut user_obligatiton_info.try_borrow_mut_data()?)?;
    // handle market reserve
    market_reserve.collateral_info.redeem(amount)?;
    market_reserve.timestamp = clock.slot;
    MarketReserve::pack(market_reserve, &mut colleteral_market_reserve_info.try_borrow_mut_data()?)?;
    // transfer
    spl_token_transfer(TokenTransferParams {
        source: manager_token_account_info.clone(),
        destination: user_token_account_info.clone(),
        amount,
        authority: manager_authority_info.clone(),
        authority_signer_seeds,
        token_program: token_program_id.clone(),
    })
}

fn process_liquidate(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    is_arbitrary: bool,
    amount: u64,
) -> ProgramResult {
    if amount == 0 {
        msg!("Collateral amount provided cannot be zero");
        return Err(LendingError::InvalidAmount.into());
    }

    let account_info_iter = &mut accounts.iter();
    let clock = &Clock::from_account_info(next_account_info(account_info_iter)?)?;
    let manager_info = next_account_info(account_info_iter)?;
    let manager_authority_info = next_account_info(account_info_iter)?;
    let liquidity_market_reserve_info = next_account_info(account_info_iter)?;
    let manager_liquidity_token_account_info = next_account_info(account_info_iter)?;
    let colleteral_market_reserve_info = next_account_info(account_info_iter)?;
    let collateral_price_oracle_info = next_account_info(account_info_iter)?;
    let manager_collateral_token_account_info = next_account_info(account_info_iter)?;
    let user_obligatiton_info = next_account_info(account_info_iter)?;
    let liquidator_authority_info = next_account_info(account_info_iter)?;
    let liquidator_liquidity_account_info = next_account_info(account_info_iter)?;
    let liquidator_collateral_account_info = next_account_info(account_info_iter)?;
    let token_program_id = next_account_info(account_info_iter)?;

    if manager_info.owner != program_id {
        msg!("Manager ower provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let manager = Manager::unpack(&manager_info.try_borrow_data()?)?;
    let authority_signer_seeds = &[
        manager_info.key.as_ref(),
        &[manager.bump_seed]
    ];
    let manager_authority = Pubkey::create_program_address(authority_signer_seeds, program_id)?;

    if manager_authority_info.key != &manager_authority {
        msg!("Manager authority is not matched with program address derived from manager info");
        return Err(LendingError::InvalidManagerAuthority.into());
    }

    if liquidity_market_reserve_info.owner != program_id {
        msg!("Liquidity market reserve owner provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut liquidity_market_reserve = MarketReserve::unpack(&liquidity_market_reserve_info.try_borrow_data()?)?;
    if &liquidity_market_reserve.manager != manager_info.key {
        msg!("Liquidity market reserve manager provided is not matched with manager info");
        return Err(LendingError::InvalidMarketReserve.into());
    }
    let liquidity_info = liquidity_market_reserve.liquidity_info
        .as_mut()
        .ok_or(LendingError::MarketReserveLiquidityNotExist)?;

    if manager_liquidity_token_account_info.key != &liquidity_market_reserve.token_info.account {
        return Err(LendingError::InvalidManagerTokenAccount.into()); 
    }

    if colleteral_market_reserve_info.owner != program_id {
        msg!("Collateral market reserve owner provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut collateral_market_reserve = MarketReserve::unpack(&colleteral_market_reserve_info.try_borrow_data()?)?;
    if &collateral_market_reserve.manager != manager_info.key {
        msg!("Collateral market reserve manager provided is not matched with manager info");
        return Err(LendingError::InvalidMarketReserve.into());
    }

    if collateral_price_oracle_info.key != &collateral_market_reserve.token_info.price_oracle {
        return Err(LendingError::InvalidPriceOracle.into());
    }
    let collateral_price = get_pyth_price(collateral_price_oracle_info, clock)?;

    if manager_collateral_token_account_info.key != &collateral_market_reserve.token_info.account {
        return Err(LendingError::InvalidManagerTokenAccount.into()); 
    }

    if user_obligatiton_info.owner != program_id {
        msg!("User Obligation owner provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut user_obligation = UserObligation::unpack(&user_obligatiton_info.try_borrow_data()?)?;
    if &user_obligation.reserve != liquidity_market_reserve_info.key {
        return Err(LendingError::InvalidUserObligation.into());
    }
    if user_obligation.last_update.is_stale(clock.slot)? {
        return Err(LendingError::ObligationStale.into());
    }

    if !liquidator_authority_info.is_signer {
        msg!("authority is not a signer");
        return Err(LendingError::InvalidSigner.into());
    }

    // handle obligation
    let liquidity_decimals = calculate_decimals(liquidity_market_reserve.token_info.decimal)?;
    let index = user_obligation.find_collateral(&collateral_market_reserve.token_info.price_oracle)?;
    let (settle, fee) = if is_arbitrary {
        user_obligation.validate_liquidation_2(liquidity_decimals)?;
        let settle = user_obligation.liquidate_2(
            index,
            amount,
            Rate::from_scaled_val(collateral_market_reserve.collateral_info.config.liquidation_2_repay_rate),
            liquidity_decimals,
            collateral_price,
        )?;

        (settle, 0)
    } else {
        user_obligation.validate_liquidation(liquidity_decimals)?;
        let settle = user_obligation.liquidate(
            index,
            amount,
            Rate::from_percent(collateral_market_reserve.collateral_info.config.close_factor),
            collateral_price
        )?;
        // calculate liquidation fee
        let fee = calculate_liquidation_fee(
            collateral_price,
            calculate_decimals(collateral_market_reserve.token_info.decimal)?,
            amount,
            user_obligation.loan_market_price,
            liquidity_decimals,
            settle.total,
            Rate::from_scaled_val(collateral_market_reserve.collateral_info.config.liquidation_1_fee_rate),
        )?;

        (settle, fee)
    };
    user_obligation.last_update.update_slot(clock.slot, true);
    UserObligation::pack(user_obligation, &mut user_obligatiton_info.try_borrow_mut_data()?)?;

    // handle liquidity market reserve
    liquidity_info.liquidity.liquidate(&settle, fee)?;
    liquidity_market_reserve.timestamp = clock.slot;
    MarketReserve::pack(liquidity_market_reserve, &mut liquidity_market_reserve_info.try_borrow_mut_data()?)?;

    // handle collateral market reserve
    collateral_market_reserve.collateral_info.redeem(amount)?;
    collateral_market_reserve.timestamp = clock.slot;
    MarketReserve::pack(collateral_market_reserve, &mut colleteral_market_reserve_info.try_borrow_mut_data()?)?;

    // transfer (liquidity)
    spl_token_transfer(TokenTransferParams {
        source: liquidator_liquidity_account_info.clone(),
        destination: manager_liquidity_token_account_info.clone(),
        amount: settle.total,
        authority: liquidator_authority_info.clone(),
        authority_signer_seeds: &[],
        token_program: token_program_id.clone(),
    })?;
    // 9. transfer (collateral)
    spl_token_transfer(TokenTransferParams {
        source: manager_collateral_token_account_info.clone(),
        destination: liquidator_collateral_account_info.clone(),
        amount,
        authority: manager_authority_info.clone(),
        authority_signer_seeds,
        token_program: token_program_id.clone(),
    })
}

fn process_feed_rate_oracle(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    interest_rate: u64,
    borrow_rate: u64,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let clock = &Clock::from_account_info(next_account_info(account_info_iter)?)?;
    let rate_oracle_info = next_account_info(account_info_iter)?;
    let authority_info = next_account_info(account_info_iter)?;

    if rate_oracle_info.owner != program_id {
        msg!("Rate oracle owner provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut rate_oracle = RateOracle::unpack(&rate_oracle_info.try_borrow_data()?)?;

    if !authority_info.is_signer {
        msg!("authority is not a signer");
        return Err(LendingError::InvalidSigner.into());
    }
    if authority_info.key != &rate_oracle.owner {
        return Err(LendingError::InvalidOracleAuthority.into())
    }

    rate_oracle.feed(interest_rate, borrow_rate, clock.slot);
    RateOracle::pack(rate_oracle, &mut rate_oracle_info.try_borrow_mut_data()?)
}

fn process_pause_rate_oracle(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let rate_oracle_info = next_account_info(account_info_iter)?;
    let authority_info = next_account_info(account_info_iter)?;

    if rate_oracle_info.owner != program_id {
        msg!("Rate oracle owner provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut rate_oracle = RateOracle::unpack(&rate_oracle_info.try_borrow_data()?)?;

    if !authority_info.is_signer {
        msg!("authority is not a signer");
        return Err(LendingError::InvalidSigner.into());
    }
    if authority_info.key != &rate_oracle.owner {
        return Err(LendingError::InvalidOracleAuthority.into())
    }

    rate_oracle.mark_stale();
    RateOracle::pack(rate_oracle, &mut rate_oracle_info.try_borrow_mut_data()?)
}

fn process_add_liquidity_to_market_reserve(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    liquidity_config: LiquidityConfig,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let clock = &Clock::from_account_info(next_account_info(account_info_iter)?)?;
    let manager_info = next_account_info(account_info_iter)?;
    let market_reserve_info = next_account_info(account_info_iter)?;
    let rate_oracle_info = next_account_info(account_info_iter)?;
    let authority_info = next_account_info(account_info_iter)?;

    if manager_info.owner != program_id {
        msg!("Manager ower provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let manager = Manager::unpack(&manager_info.try_borrow_data()?)?;

    if market_reserve_info.owner != program_id {
        msg!("MarketReserve owner provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut market_reserve = MarketReserve::unpack(&market_reserve_info.try_borrow_data()?)?;
    if &market_reserve.manager != manager_info.key {
        msg!("MarketReserve manager provided is not matched with manager info");
        return Err(LendingError::InvalidMarketReserve.into());
    }
    if market_reserve.liquidity_info.is_some() {
        return Err(LendingError::MarketReserveLiquidityAlreadyExist.into());
    }

    if rate_oracle_info.owner != program_id {
        return Err(LendingError::InvalidAccountOwner.into());
    }
    RateOracle::unpack(&rate_oracle_info.try_borrow_data()?)?;

    if authority_info.key != &manager.owner {
        msg!("Only manager owner can create reserve");
        return Err(LendingError::InvalidManagerOwner.into());
    }
    if !authority_info.is_signer {
        msg!("authority is not a signer");
        return Err(LendingError::InvalidSigner.into());
    }

    market_reserve.liquidity_info = COption::Some(
        LiquidityInfo{
            rate_oracle: *rate_oracle_info.key,
            liquidity: Liquidity::default(),
            config: liquidity_config,
        }
    );
    market_reserve.timestamp = clock.slot;
    MarketReserve::pack(market_reserve, &mut market_reserve_info.try_borrow_mut_data()?)
}

fn process_withdraw_fee(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    amount: u64,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let clock = &Clock::from_account_info(next_account_info(account_info_iter)?)?;
    let manager_info = next_account_info(account_info_iter)?;
    let manager_authority_info = next_account_info(account_info_iter)?;
    let market_reserve_info = next_account_info(account_info_iter)?;
    let manager_token_account_info = next_account_info(account_info_iter)?;
    let authority_info = next_account_info(account_info_iter)?;
    let receiver_token_account_info = next_account_info(account_info_iter)?;
    let token_program_id = next_account_info(account_info_iter)?;

    if manager_info.owner != program_id {
        msg!("Manager ower provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let manager = Manager::unpack(&manager_info.try_borrow_data()?)?;
    let authority_signer_seeds = &[
        manager_info.key.as_ref(),
        &[manager.bump_seed],
    ];
    let manager_authority = Pubkey::create_program_address(authority_signer_seeds, program_id)?;

    if manager_authority_info.key != &manager_authority {
        msg!("Manager authority is not matched with program address derived from manager info");
        return Err(LendingError::InvalidManagerAuthority.into());
    }

    if market_reserve_info.owner != program_id {
        msg!("MarketReserve owner provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut market_reserve = MarketReserve::unpack(&market_reserve_info.try_borrow_data()?)?;
    if &market_reserve.manager != manager_info.key {
        msg!("MarketReserve manager provided is not matched with manager info");
        return Err(LendingError::InvalidMarketReserve.into());
    }
    let liquidity_info = market_reserve.liquidity_info
        .as_mut()
        .ok_or(LendingError::MarketReserveLiquidityNotExist)?;

    if manager_token_account_info.key != &market_reserve.token_info.account {
        return Err(LendingError::InvalidManagerTokenAccount.into()); 
    }
    
    if authority_info.key != &manager.owner {
        msg!("Only manager owner can withdraw fee");
        return Err(LendingError::InvalidManagerOwner.into());
    }
    if !authority_info.is_signer {
        msg!("authority is not a signer");
        return Err(LendingError::InvalidSigner.into());
    }

    // withdraw fee
    liquidity_info.liquidity.withdraw_fee(amount)?;
    // update timestamp
    market_reserve.timestamp = clock.slot;
    // pack
    MarketReserve::pack(market_reserve, &mut market_reserve_info.try_borrow_mut_data()?)?;
    // transfer
    spl_token_transfer(TokenTransferParams {
        source: manager_token_account_info.clone(),
        destination: receiver_token_account_info.clone(),
        amount,
        authority: manager_authority_info.clone(),
        authority_signer_seeds,
        token_program: token_program_id.clone(),
    })
}

fn assert_rent_exempt(rent: &Rent, account_info: &AccountInfo) -> ProgramResult {
    if !rent.is_exempt(account_info.lamports(), account_info.data_len()) {
        msg!(&rent.minimum_balance(account_info.data_len()).to_string());
        Err(LendingError::NotRentExempt.into())
    } else {
        Ok(())
    }
}

fn assert_uninitialized<T: Pack + IsInitialized>(account_info: &AccountInfo) -> ProgramResult {
    let account: T = T::unpack_unchecked(&account_info.try_borrow_data()?)?;
    if account.is_initialized() {
        Err(LendingError::AlreadyInitialized.into())
    } else {
        Ok(())
    }
}

fn get_pyth_product_quote_currency(pyth_product: &pyth::Product) -> Result<[u8; 32], ProgramError> {
    const LEN: usize = 14;
    const KEY: &[u8; LEN] = b"quote_currency";

    let mut start = 0;
    while start < pyth::PROD_ATTR_SIZE {
        let mut length = pyth_product.attr[start] as usize;
        start += 1;

        if length == LEN {
            let mut end = start + length;
            if end > pyth::PROD_ATTR_SIZE {
                msg!("Pyth product attribute key length too long");
                return Err(LendingError::InvalidOracleConfig.into());
            }

            let key = &pyth_product.attr[start..end];
            if key == KEY {
                start += length;
                length = pyth_product.attr[start] as usize;
                start += 1;

                end = start + length;
                if length > 32 || end > pyth::PROD_ATTR_SIZE {
                    msg!("Pyth product quote currency value too long");
                    return Err(LendingError::InvalidOracleConfig.into());
                }

                let mut value = [0u8; 32];
                value[0..length].copy_from_slice(&pyth_product.attr[start..end]);
                return Ok(value);
            }
        }

        start += length;
        start += 1 + pyth_product.attr[start] as usize;
    }

    msg!("Pyth product quote currency not found");
    Err(LendingError::InvalidOracleConfig.into())
}

fn get_pyth_price(pyth_price_info: &AccountInfo, clock: &Clock) -> Result<Decimal, ProgramError> {
    const STALE_AFTER_SLOTS_ELAPSED: u64 = 5;

    let pyth_price_data = pyth_price_info.try_borrow_data()?;
    let pyth_price = pyth::load::<pyth::Price>(&pyth_price_data)
        .map_err(|_| ProgramError::InvalidAccountData)?;

    if pyth_price.ptype != pyth::PriceType::Price {
        msg!("Oracle price type is invalid");
        return Err(LendingError::InvalidOracleConfig.into());
    }

    let slots_elapsed = clock.slot
        .checked_sub(pyth_price.valid_slot)
        .ok_or(LendingError::MathOverflow)?;
    if slots_elapsed >= STALE_AFTER_SLOTS_ELAPSED {
        msg!("Oracle price is stale");
        return Err(LendingError::InvalidOracleConfig.into());
    }

    let price: u64 = pyth_price.agg.price.try_into().map_err(|_| {
        msg!("Oracle price cannot be negative");
        LendingError::InvalidOracleConfig
    })?;

    if pyth_price.expo >= 0 {
        let exponent = pyth_price.expo
            .try_into()
            .map_err(|_| LendingError::MathOverflow)?;
        let zeros = 10u64
            .checked_pow(exponent)
            .ok_or(LendingError::MathOverflow)?;
        Decimal::from(price).try_mul(zeros)
    } else {
        let exponent = pyth_price.expo
            .checked_abs()
            .ok_or(LendingError::MathOverflow)?
            .try_into()
            .map_err(|_| LendingError::MathOverflow)?;
        let decimals = 10u64
            .checked_pow(exponent)
            .ok_or(LendingError::MathOverflow)?;
        Decimal::from(price).try_div(decimals)
    }
}

/// Issue a spl_token `Transfer` instruction.
#[inline(always)]
fn spl_token_transfer(params: TokenTransferParams<'_, '_>) -> ProgramResult {
    let TokenTransferParams {
        source,
        destination,
        authority,
        token_program,
        amount,
        authority_signer_seeds,
    } = params;
    let result = invoke_signed(
        &spl_token::instruction::transfer(
            token_program.key,
            source.key,
            destination.key,
            authority.key,
            &[],
            amount,
        )?,
        &[source, destination, authority, token_program],
        &[authority_signer_seeds],
    );
    result.map_err(|_| LendingError::TokenTransferFailed.into())
}

struct TokenTransferParams<'a: 'b, 'b> {
    source: AccountInfo<'a>,
    destination: AccountInfo<'a>,
    amount: u64,
    authority: AccountInfo<'a>,
    authority_signer_seeds: &'b [&'b [u8]],
    token_program: AccountInfo<'a>,
}

impl PrintProgramError for LendingError {
    fn print<E>(&self)
    where
        E: 'static + std::error::Error + DecodeError<E> + PrintProgramError + FromPrimitive,
    {
        msg!(&self.to_string());
    }
}