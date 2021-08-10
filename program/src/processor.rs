//! Program state processor
use crate::{
    error::LendingError,
    instruction::LendingInstruction,
    math::Rate,
    state::{
        CollateralConfig, CollateralInfo, Collateral, LastUpdate, Liquidity, LiquidityConfig,
        LiquidityInfo, Manager, MarketReserve, PROGRAM_VERSION, TokenInfo, UserAsset, UserObligation,
        Fund, Settle, price_conversion, calculate_interest_fee, validate_liquidation_limit,
    },
};
use flux_aggregator::{borsh_state::InitBorshState, read_median, state::Aggregator};
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
use spl_token::state::Account;

/// Processes an instruction
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    input: &[u8],
) -> ProgramResult {
    let instruction = LendingInstruction::unpack(input)?;
    match instruction {
        LendingInstruction::InitManager { quote_decimal } => {
            msg!("Instruction: Init Lending Manager");
            process_init_manager(program_id, accounts, quote_decimal)
        }
        LendingInstruction::InitMarketReserveWithoutLiquidity {
            liquidate_fee_rate,
            liquidate_limit_rate,
        } => {
            msg!("Instruction: Init Market Reserve Without Liquidity");
            process_init_market_reserve(program_id, accounts, &CollateralConfig{
                liquidate_fee_rate,
                liquidate_limit_rate,
            }, None)
        }
        LendingInstruction::InitMarketReserveWithLiquidity {
            liquidate_fee_rate,
            liquidate_limit_rate,
            min_borrow_utilization_rate,
            max_borrow_utilization_rate,
            interest_fee_rate,
        } => {
            msg!("Instruction: Init Market Reserve With Liquidity");
            process_init_market_reserve(program_id, accounts, &CollateralConfig{
                liquidate_fee_rate,
                liquidate_limit_rate,
            }, Some(&LiquidityConfig{
                min_borrow_utilization_rate,
                max_borrow_utilization_rate,
                interest_fee_rate,
            }))
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
        LendingInstruction::BorrowLiquidity { amount } => {
            msg!("Instruction: Borrow Collateral: {}", amount);
            process_borrow_liquidity(program_id, accounts, amount)
        }
    }
}

fn process_init_manager(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    quote_decimal: u8,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let rent = &Rent::from_account_info(next_account_info(account_info_iter)?)?;
    let manager_info = next_account_info(account_info_iter)?;
    let owner_info = next_account_info(account_info_iter)?;
    let token_program_id = next_account_info(account_info_iter)?;

    if manager_info.owner != program_id {
        msg!("manager provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    assert_rent_exempt(rent, manager_info)?;
    assert_uninitialized::<Manager>(manager_info)?;

    let manager = Manager{
        version: PROGRAM_VERSION,
        bump_seed: Pubkey::find_program_address(&[manager_info.key.as_ref()], program_id).1,
        quote_decimal,
        owner: *owner_info.key,
        token_program_id: *token_program_id.key,
    };
    Manager::pack(manager, &mut manager_info.data.borrow_mut())
}

fn process_init_market_reserve(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    collateral_config: &CollateralConfig,
    liquidity_config: Option<&LiquidityConfig>,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let rent = &Rent::from_account_info(next_account_info(account_info_iter)?)?;
    let clock = &Clock::from_account_info(next_account_info(account_info_iter)?)?;
    let manager_info = next_account_info(account_info_iter)?;
    let market_reserve_info = next_account_info(account_info_iter)?;
    let price_oracle_info = next_account_info(account_info_iter)?;
    let token_account_info = next_account_info(account_info_iter)?;
    let authority_info = next_account_info(account_info_iter)?;

    if manager_info.owner != program_id {
        msg!("Manager ower provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let manager = Manager::unpack(&manager_info.data.borrow())?;
    let manager_authority = &Pubkey::create_program_address(&[manager_info.key.as_ref(),
        &[manager.bump_seed]], program_id)?;

    if market_reserve_info.owner != program_id {
        msg!("MarketReserve owner provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    assert_rent_exempt(rent, market_reserve_info)?;
    assert_uninitialized::<MarketReserve>(market_reserve_info)?;

    let price_oracle = Aggregator::load_initialized(price_oracle_info)?;
    if price_oracle.config.decimals != manager.quote_decimal {
        msg!("Quote token mint decimals does not match the aggregator config decimals provided");
        return Err(LendingError::InvalidOracleConfig.into());
    }
    drop(price_oracle);

    if token_account_info.owner != &manager.token_program_id {
        msg!("Token account info owner provided is not owned by the token program in manager");
        return Err(LendingError::InvalidTokenProgram.into());
    }
    let token_account = Account::unpack(&token_account_info.data.borrow())?;
    if &token_account.owner != manager_authority {
        msg!("Token account owner is not matched with manager authority");
        return Err(LendingError::InvalidTokenAccount.into());
    }
    drop(token_account);

    if authority_info.key != &manager.owner {
        msg!("Only manager owner can create reserve");
        return Err(LendingError::InvalidManagerOwner.into());
    }
    drop(manager);
    if !authority_info.is_signer {
        msg!("authority is not a signer");
        return Err(LendingError::InvalidSigner.into());
    }

    let liquidity_info = if let Some(liquidity_config) = liquidity_config {
        let interest_rate_oracle_info = next_account_info(account_info_iter)?;
        Aggregator::load_initialized(interest_rate_oracle_info)?;

        let borrow_rate_oracle_info = next_account_info(account_info_iter)?;
        Aggregator::load_initialized(borrow_rate_oracle_info)?;

        COption::Some(LiquidityInfo{
            interest_rate_oracle: *interest_rate_oracle_info.key,
            borrow_rate_oracle: *borrow_rate_oracle_info.key,
            min_borrow_utilization_rate: liquidity_config.min_borrow_utilization_rate,
            max_borrow_utilization_rate: liquidity_config.max_borrow_utilization_rate,
            interest_fee_rate: liquidity_config.interest_fee_rate,
            liquidity: Liquidity::default(),
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
            price_oracle: *price_oracle_info.key,
        },
        collateral_info: CollateralInfo{
            liquidate_fee_rate: collateral_config.liquidate_fee_rate,
            liquidate_limit_rate: collateral_config.liquidate_limit_rate,
            amount: 0,
        },
        liquidity_info,
    };
    MarketReserve::pack(market_reserve, &mut market_reserve_info.data.borrow_mut())
}

fn process_init_user_obligation(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let market_reserve_info = next_account_info(account_info_iter)?;
    let user_obligation_info = next_account_info(account_info_iter)?;
    let owner_info = next_account_info(account_info_iter)?;
    let rent = &Rent::from_account_info(next_account_info(account_info_iter)?)?;
    let clock = &Clock::from_account_info(next_account_info(account_info_iter)?)?;

    if market_reserve_info.owner != program_id {
        msg!("MarketReserve owner provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    MarketReserve::unpack(&market_reserve_info.data.borrow())?;

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
        borrowed_amount: 0,
        dept_amount: 0,
    };
    UserObligation::pack(user_obligation, &mut user_obligation_info.data.borrow_mut())
}

fn process_init_user_asset(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let market_reserve_info = next_account_info(account_info_iter)?;
    let user_asset_info = next_account_info(account_info_iter)?;
    let owner_info = next_account_info(account_info_iter)?;
    let rent = &Rent::from_account_info(next_account_info(account_info_iter)?)?;
    let clock = &Clock::from_account_info(next_account_info(account_info_iter)?)?;

    if market_reserve_info.owner != program_id {
        msg!("MarketReserve owner provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    MarketReserve::unpack(&market_reserve_info.data.borrow())?;

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
    UserAsset::pack(user_asset, &mut user_asset_info.data.borrow_mut())
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
    let market_reserve_info = next_account_info(account_info_iter)?;
    let interest_rate_oracle_info = next_account_info(account_info_iter)?;
    let user_asset_info = next_account_info(account_info_iter)?;
    let manager_token_account_info = next_account_info(account_info_iter)?;
    let user_token_account_info = next_account_info(account_info_iter)?;
    let user_authority_info = next_account_info(account_info_iter)?;
    let token_program_id = next_account_info(account_info_iter)?;
    let clock = &Clock::from_account_info(next_account_info(account_info_iter)?)?;

    if market_reserve_info.owner != program_id {
        msg!("MarketReserve owner provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut market_reserve = MarketReserve::unpack(&market_reserve_info.data.borrow())?;
    let liquidity_info = market_reserve.liquidity_info
        .as_mut()
        .ok_or(LendingError::MarketReserveLiquidityNotAvailable)?;

    if interest_rate_oracle_info.key != &liquidity_info.interest_rate_oracle {
        msg!("MarketReserve liquidity interest rate oracle is not matched with provided");
        return Err(LendingError::InvalidInterestOracle.into());
    }
    let interest_rate = read_median(interest_rate_oracle_info)?.median;

    if user_asset_info.owner != program_id {
        msg!("UserAsset owner provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut user_asset = UserAsset::unpack(&user_asset_info.data.borrow())?;
    if &user_asset.reserve != market_reserve_info.key {
        msg!("UserAsset market reserve is not matched with accounts provided");
        return Err(LendingError::InvalidMarketReserve.into());
    }

    if manager_token_account_info.key != &market_reserve.token_info.account {
        return Err(LendingError::InvalidManagerTokenAccount.into()); 
    }

    if user_authority_info.key != &user_asset.owner {
        return Err(LendingError::InvalidUserAuthority.into());
    }

    // 1. update
    user_asset.update_interest(clock.slot, Rate::from_scaled_val(interest_rate))?;
    // 2. deposit in obligation
    user_asset.deposit(amount)?;
    // 3. deposit and update in market reserve
    liquidity_info.liquidity.deposit(amount)?;
    market_reserve.timestamp = clock.slot;
    // 4. pack data
    MarketReserve::pack(market_reserve, &mut market_reserve_info.data.borrow_mut())?;
    UserAsset::pack(user_asset, &mut user_asset_info.data.borrow_mut())?;
    // 5. transfer from user to manager
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
    let manager_info = next_account_info(account_info_iter)?;
    let manager_authority_info = next_account_info(account_info_iter)?;
    let market_reserve_info = next_account_info(account_info_iter)?;
    let manager_token_account_info = next_account_info(account_info_iter)?;
    let interest_rate_oracle_info = next_account_info(account_info_iter)?;
    let user_asset_info = next_account_info(account_info_iter)?;
    let user_token_account_info = next_account_info(account_info_iter)?;
    let user_authority_info = next_account_info(account_info_iter)?;
    let token_program_id = next_account_info(account_info_iter)?;
    let clock = &Clock::from_account_info(next_account_info(account_info_iter)?)?;

    if manager_info.owner != program_id {
        msg!("Manager ower provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let manager = Manager::unpack(&manager_info.data.borrow())?;
    let authority_signer_seeds = &[
        manager_info.key.as_ref(),
        &[manager.bump_seed]
    ];
    drop(manager);
    let manager_authority = Pubkey::create_program_address(authority_signer_seeds, program_id)?;

    if manager_authority_info.key != &manager_authority {
        msg!("Manager authority is not matched with program address derived from manager info");
        return Err(LendingError::InvalidManagerAuthority.into());
    }

    if market_reserve_info.owner != program_id {
        msg!("MarketReserve owner provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut market_reserve = MarketReserve::unpack(&market_reserve_info.data.borrow())?;
    if &market_reserve.manager != manager_info.key {
        msg!("MarketReserve manager provided is not matched with manager info");
        return Err(LendingError::InvalidMarketReserve.into());
    }
    let liquidity_info = market_reserve.liquidity_info
        .as_mut()
        .ok_or(LendingError::MarketReserveLiquidityNotAvailable)?;

    if manager_token_account_info.key != &market_reserve.token_info.account {
        return Err(LendingError::InvalidManagerTokenAccount.into()); 
    }

    if interest_rate_oracle_info.key != &liquidity_info.interest_rate_oracle {
        msg!("MarketReserve liquidity interest rate oracle is not matched with provided");
        return Err(LendingError::InvalidInterestOracle.into());
    }
    let interest_rate = read_median(interest_rate_oracle_info)?.median;

    if user_asset_info.owner != program_id {
        msg!("UserAsset owner provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut user_asset = UserAsset::unpack(&user_asset_info.data.borrow())?;
    if &user_asset.reserve != market_reserve_info.key {
        msg!("UserAsset market reserve is not matched with accounts provided");
        return Err(LendingError::InvalidMarketReserve.into());
    }

    if user_authority_info.key != &user_asset.owner {
        return Err(LendingError::InvalidUserAuthority.into());
    }

    // 1. update
    user_asset.update_interest(clock.slot, Rate::from_scaled_val(interest_rate))?;
    // 2. withdraw
    let fund = user_asset.withdraw(amount)?;
    // 3. withdraw and update in market reserve
    let fee = calculate_interest_fee(fund.interest, Rate::from_scaled_val(liquidity_info.interest_fee_rate))?;
    liquidity_info.liquidity.withdraw(&fund, fee)?;
    market_reserve.timestamp = clock.slot;
    // 4. pack data
    MarketReserve::pack(market_reserve, &mut market_reserve_info.data.borrow_mut())?;
    UserAsset::pack(user_asset, &mut user_asset_info.data.borrow_mut())?;
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
    let market_reserve_info = next_account_info(account_info_iter)?;
    let manager_token_account_info = next_account_info(account_info_iter)?;
    let user_obligatiton_info = next_account_info(account_info_iter)?;
    let user_token_account_info = next_account_info(account_info_iter)?;
    let user_authority_info = next_account_info(account_info_iter)?;
    let token_program_id = next_account_info(account_info_iter)?;
    let clock = &Clock::from_account_info(next_account_info(account_info_iter)?)?;

    if market_reserve_info.owner != program_id {
        msg!("Market reserve owner provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut market_reserve = MarketReserve::unpack(&market_reserve_info.data.borrow())?;

    if manager_token_account_info.key != &market_reserve.token_info.account {
        return Err(LendingError::InvalidManagerTokenAccount.into()); 
    }

    if user_obligatiton_info.owner != program_id {
        msg!("User Obligation owner provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut user_obligation = UserObligation::unpack(&user_obligatiton_info.data.borrow())?;

    if user_authority_info.key != &user_obligation.owner {
        return Err(LendingError::InvalidUserAuthority.into());
    }

    let price_oracle = &market_reserve.token_info.price_oracle;
    // 1. pledge collateral in obligation
    if let Some(index) = user_obligation.find_collateral(price_oracle) {
        user_obligation.pledge(index, amount)?;
    } else {
        user_obligation.new_pledge(Collateral{
            price_oracle: price_oracle.clone(),
            liquidate_limit_rate: market_reserve.collateral_info.liquidate_limit_rate,
            amount,
        })?;
    }
    // 2. pledge collateral and update in market reserve
    market_reserve.collateral_info.add(amount)?;
    market_reserve.timestamp = clock.slot;
    // 3. pack
    UserObligation::pack(user_obligation, &mut user_obligatiton_info.data.borrow_mut())?;
    MarketReserve::pack(market_reserve, &mut market_reserve_info.data.borrow_mut())?;
    // 4. transfer from user to manager
    spl_token_transfer(TokenTransferParams {
        source: user_token_account_info.clone(),
        destination: manager_token_account_info.clone(),
        amount,
        authority: user_authority_info.clone(),
        authority_signer_seeds: &[],
        token_program: token_program_id.clone(),
    })
}

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
    let liquidity_price_oracle_info = next_account_info(account_info_iter)?;
    let borrow_rate_oracle_info = next_account_info(account_info_iter)?;
    let manager_token_account_info = next_account_info(account_info_iter)?;
    let user_obligatiton_info = next_account_info(account_info_iter)?;
    let user_token_account_info = next_account_info(account_info_iter)?;
    let user_authority_info = next_account_info(account_info_iter)?;
    let token_program_id = next_account_info(account_info_iter)?;

    if manager_info.owner != program_id {
        msg!("Manager ower provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let manager = Manager::unpack(&manager_info.data.borrow())?;
    let authority_signer_seeds = &[
        manager_info.key.as_ref(),
        &[manager.bump_seed]
    ];
    let decimal = manager.quote_decimal;
    drop(manager);
    let manager_authority = Pubkey::create_program_address(authority_signer_seeds, program_id)?;

    if manager_authority_info.key != &manager_authority {
        msg!("Manager authority is not matched with program address derived from manager info");
        return Err(LendingError::InvalidManagerAuthority.into());
    }
    drop(manager_authority);

    if market_reserve_info.owner != program_id {
        msg!("Market reserve owner provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut market_reserve = MarketReserve::unpack(&market_reserve_info.data.borrow())?;
    if &market_reserve.manager != manager_info.key {
        msg!("MarketReserve manager provided is not matched with manager info");
        return Err(LendingError::InvalidMarketReserve.into());
    }
    let liquidity_info = market_reserve.liquidity_info
        .as_mut()
        .ok_or(LendingError::MarketReserveLiquidityNotAvailable)?;

    if liquidity_price_oracle_info.key != &market_reserve.token_info.price_oracle {
        return Err(LendingError::InvalidPriceOracle.into());
    }

    if borrow_rate_oracle_info.key != &liquidity_info.borrow_rate_oracle {
        return Err(LendingError::InvalidBorrowRateOracle.into());
    }

    if manager_token_account_info.key != &market_reserve.token_info.account {
        return Err(LendingError::InvalidManagerTokenAccount.into()); 
    }

    if user_obligatiton_info.owner != program_id {
        msg!("User Obligation owner provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut user_obligation = UserObligation::unpack(&user_obligatiton_info.data.borrow())?;
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

    // 1. update obligation
    let borrow_rate = read_median(borrow_rate_oracle_info)?.median;
    user_obligation.update_borrow_interest(clock.slot, Rate::from_scaled_val(borrow_rate))?;
    // 2. borrow
    user_obligation.borrow_out(amount)?;
    // 3. calculate loan value
    let price = price_conversion(read_median(liquidity_price_oracle_info)?.median, decimal)?;
    let loan_value = user_obligation.loan_value(price)?;
    // 4. calculate collaterals value
    let settles = account_info_iter.map(|price_oracle_info| {
        let price = price_conversion(read_median(price_oracle_info)?.median, decimal)?;

        Ok(Settle{
            price_oracle: *price_oracle_info.key,
            price,
        })
    }).collect::<Result<Vec<_>, ProgramError>>()?;
    let collaterals_value = user_obligation.collaterals_value(&settles)?;
    drop(settles);
    // 5. validation
    validate_liquidation_limit(loan_value, collaterals_value)?;
    // 6. borrow and update in reserve
    liquidity_info.liquidity.borrow_out(amount)?;
    market_reserve.timestamp = clock.slot;
    // 7. pack
    UserObligation::pack(user_obligation, &mut user_obligatiton_info.data.borrow_mut())?;
    MarketReserve::pack(market_reserve, &mut market_reserve_info.data.borrow_mut())?;
    // 8. transfer
    spl_token_transfer(TokenTransferParams {
        source: manager_token_account_info.clone(),
        destination: user_token_account_info.clone(),
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
    let account: T = T::unpack_unchecked(&account_info.data.borrow())?;
    if account.is_initialized() {
        Err(LendingError::AlreadyInitialized.into())
    } else {
        Ok(())
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