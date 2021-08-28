//! Program state processor
use std::{convert::TryInto, any::Any};
use crate::{
    error::LendingError,
    instruction::LendingInstruction,
    math::{Decimal, TryDiv, TryMul},
    pyth,
    state::{CollateralConfig, CollateralInfo, EnableBorrow, LastUpdate,
        LiquidityConfig, LiquidityInfo, Manager, MarketReserve, Operator,
        Param, Pause, PROGRAM_VERSION, RateOracle, RateOracleConfig,
        TokenInfo, UserObligation,
    },
};
use num_traits::FromPrimitive;
use solana_program::{
    account_info::{AccountInfo, next_account_info},
    decode_error::DecodeError,
    entrypoint::ProgramResult,
    instruction::Instruction,
    msg,
    program::{invoke, invoke_signed},
    program_error::{PrintProgramError, ProgramError},
    program_option::COption,
    program_pack::{IsInitialized, Pack},
    pubkey::Pubkey,
    sysvar::{clock::Clock, rent::Rent, Sysvar}
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
        LendingInstruction::InitRateOracle { asset_index, config } => {
            msg!("Instruction: Init Rate Oracle");
            process_init_rate_oracle(program_id, accounts, asset_index, config)
        }
        LendingInstruction::InitMarketReserve {
            collateral_config,
            liquidity_config,
            enable_borrow,
        } => {
            msg!("Instruction: Init Market Reserve With Liquidity");
            process_init_market_reserve(program_id, accounts, collateral_config, liquidity_config, enable_borrow)
        }
        LendingInstruction::UpdateMarketReserves => {
            msg!("Instruction: Update Market Reserves");
            process_update_market_reserves(program_id, accounts)
        }
        LendingInstruction::Exchange { from_collateral, amount } => {
            let instruction = if from_collateral {
                "collateral to liquidity"
            } else {
                "liquidity to collateral"
            };
            msg!("Instruction: Exchange from {}, amount = {}", instruction, amount);
            process_exchange(program_id, accounts, amount, from_collateral)
        }
        LendingInstruction::InitUserObligation => {
            msg!("Instruction: Init User Obligation");
            process_init_user_obligation(program_id, accounts)
        }
        LendingInstruction::UpdateUserObligation => {
            msg!("Instruction: Update User Obligation");
            process_update_user_obligation(program_id, accounts)
        }
        LendingInstruction::BindOrUnbindFriend { is_bind } => {
            let instruction = if is_bind { "Bind" } else { "Unbind" };
            msg!("Instruction: {} Friend", instruction);
            process_bind_or_unbind_friend(program_id, accounts, is_bind)
        }
        LendingInstruction::DepositCollateral { amount } => {
            msg!("Instruction: Deposit Collateral: {}", amount);
            process_deposit_collateral(program_id, accounts, amount)
        }
        LendingInstruction::RedeemCollateral { amount } => {
            msg!("Instruction: Redeem Collateral: {}", amount);
            process_redeem_collateral(program_id, accounts, amount)
        }
        LendingInstruction::RedeemCollateralWithoutLoan { amount } => {
            msg!("Instruction: Redeem Collateral Without Loan: {}", amount);
            process_redeem_collateral_without_loan(program_id, accounts, amount)
        }
        LendingInstruction::ReplaceCollateral { out_amount, in_amount } => {
            msg!("Instruction: Replace Collateral: out amount = {}, in amount = {}", out_amount, in_amount);
            process_replace_collateral(program_id, accounts, out_amount,in_amount)
        }
        LendingInstruction::BorrowLiquidity { amount } => {
            msg!("Instruction: Borrow Liquidity: {}", amount);
            process_borrow_liquidity(program_id, accounts, amount)
        }
        LendingInstruction::RepayLoan { amount } => {
            msg!("Instruction: Repay Loan: {}", amount);
            process_repay_loan(program_id, accounts, amount)
        }
        LendingInstruction::Liquidate { amount } => {
            msg!("Instruction: Liquidation: amount = {}", amount);
            process_liquidate(program_id, accounts, amount)
        }
        LendingInstruction::FeedRateOracle { asset_index } => {
            msg!("Instruction: Feed Rate Oracle: asset index = {}", asset_index);
            process_feed_rate_oracle(program_id, accounts, asset_index)
        }
        LendingInstruction::PauseRateOracle => {
            msg!("Instruction: Pause Rate Oracle");
            process_operate_rate_oracle(program_id, accounts, Pause())
        }
        LendingInstruction::UpdateRateOracleConfig { config } => {
            msg!("Instruction: Updae Rate Oracle Config");
            process_operate_rate_oracle(program_id, accounts, config)
        }
        LendingInstruction::EnableBorrowForMarketReserve => {
            msg!("Instruction: Enable Borrow For Market Reserve");
            process_operate_market_reserve(program_id, accounts, EnableBorrow())
        }
        LendingInstruction::UpdateMarketReserveCollateralConfig { config } => {
            msg!("Instruction: Update Market Reserve Collateral Config");
            process_operate_market_reserve(program_id, accounts, config)
        }
        LendingInstruction::UpdateMarketReserveLiquidityConfig { config } => {
            msg!("Instruction: Update Market Reserve Liquidity Config");
            process_operate_market_reserve(program_id, accounts, config)
        }
        LendingInstruction::WithdrawFee { amount } => {
            msg!("Instruction: Withdraw Fee: {}", amount);
            process_withdraw_fee(program_id, accounts, amount)
        }
        #[cfg(feature = "case-injection")]
        LendingInstruction::InjectCase { is_liquidation } => {
            let instruction = if is_liquidation { "Liquidation" } else { "Unhealthy" };
            msg!("Instruction(Test): Inject {}", instruction);
            process_inject_case(program_id, accounts, is_liquidation)
        }
    }
}

fn process_init_manager(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    quote_currency: [u8; 32],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    // 1
    let rent = &Rent::from_account_info(next_account_info(account_info_iter)?)?;
    // 2
    let manager_info = next_account_info(account_info_iter)?;
    if manager_info.owner != program_id {
        msg!("manager provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    assert_rent_exempt(rent, manager_info)?;
    assert_uninitialized::<Manager>(manager_info)?;
    // 3
    let owner_info = next_account_info(account_info_iter)?;
    // 4
    let oracle_program_id = next_account_info(account_info_iter)?;
    // 5
    let token_program_id = next_account_info(account_info_iter)?;
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
    asset_index: u64,
    config: RateOracleConfig,
) -> ProgramResult {
    // check config
    config.is_valid()?;

    let account_info_iter = &mut accounts.iter();
    // 1
    let rent = &Rent::from_account_info(next_account_info(account_info_iter)?)?;
    // 2
    let clock = &Clock::from_account_info(next_account_info(account_info_iter)?)?;
    // 3
    let rate_oracle_info = next_account_info(account_info_iter)?;
    if rate_oracle_info.owner != program_id {
        msg!("Rate oracle owner provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    assert_rent_exempt(rent, rate_oracle_info)?;
    assert_uninitialized::<RateOracle>(rate_oracle_info)?;
    // 4
    let owner_info = next_account_info(account_info_iter)?;

    let rate_oracle = RateOracle {
        version: PROGRAM_VERSION,
        owner: *owner_info.key,
        available: true,
        last_slot: clock.slot,
        asset_index,
        config,
    };
    RateOracle::pack(rate_oracle, &mut rate_oracle_info.try_borrow_mut_data()?)
}

fn process_init_market_reserve(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    collateral_config: CollateralConfig,
    liquidity_config: LiquidityConfig,
    enable_borrow: bool,
) -> ProgramResult {
    // check config
    collateral_config.is_valid()?;
    liquidity_config.is_valid()?;

    let account_info_iter = &mut accounts.iter();
    // 1
    let rent = &Rent::from_account_info(next_account_info(account_info_iter)?)?;
    // 2
    let clock = &Clock::from_account_info(next_account_info(account_info_iter)?)?;
    // 3
    let manager_info = next_account_info(account_info_iter)?;
    if manager_info.owner != program_id {
        msg!("Manager ower provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let manager = Manager::unpack(&manager_info.try_borrow_data()?)?;
    let manager_authority = &Pubkey::create_program_address(
        &[manager_info.key.as_ref(), &[manager.bump_seed]],
        program_id
    )?;
    // 4
    let market_reserve_info = next_account_info(account_info_iter)?;
    if market_reserve_info.owner != program_id {
        msg!("MarketReserve owner provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    assert_rent_exempt(rent, market_reserve_info)?;
    assert_uninitialized::<MarketReserve>(market_reserve_info)?;
    // 5
    let pyth_product_info = next_account_info(account_info_iter)?;
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
    // 6
    let pyth_price_info = next_account_info(account_info_iter)?;
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
    let market_price = get_pyth_price(pyth_price_info, clock)?;
    // 7
    let rate_oracle_info = next_account_info(account_info_iter)?;
    if rate_oracle_info.owner != program_id {
        return Err(LendingError::InvalidAccountOwner.into());
    }
    RateOracle::unpack(&rate_oracle_info.try_borrow_data()?)?;
    // 8
    let token_mint_info = next_account_info(account_info_iter)?;
    if token_mint_info.owner != &manager.token_program_id {
        msg!("Token mint info owner provided is not owned by the token program in manager");
        return Err(LendingError::InvalidTokenProgram.into()); 
    }
    let token_mint = Mint::unpack(&token_mint_info.try_borrow_data()?)?;
    // 9
    let sotoken_mint_info = next_account_info(account_info_iter)?;
    if sotoken_mint_info.owner != &manager.token_program_id {
        msg!("SoToken mint info owner provided is not owned by the token program in manager");
        return Err(LendingError::InvalidTokenProgram.into()); 
    }
    let sotoken_mint = Mint::unpack(&sotoken_mint_info.try_borrow_data()?)?;
    if sotoken_mint.mint_authority.as_ref().unwrap() != manager_authority {
        msg!("SoToken mint owner provided is matched with manager_authority");
        return Err(LendingError::InvalidSoTokenMint.into());
    }
    if sotoken_mint.decimals != token_mint.decimals {
        msg!("SoToken mint owner provided is matched with manager_authority");
        return Err(LendingError::InvalidSoTokenMint.into());
    }
    // 10
    let token_account_info = next_account_info(account_info_iter)?;
    if token_account_info.owner != &manager.token_program_id {
        msg!("Token account info owner provided is not owned by the token program in manager");
        return Err(LendingError::InvalidTokenProgram.into());
    }
    let token_account = Account::unpack(&token_account_info.try_borrow_data()?)?;
    if &token_account.mint != token_mint_info.key {
        msg!("Token account mint is not matched with token mint provided");
        return Err(LendingError::InvalidTokenAccount.into());
    }
    if &token_account.owner != manager_authority {
        msg!("Token account owner is not matched with manager authority");
        return Err(LendingError::InvalidTokenAccount.into());
    }
    // 11
    let authority_info = next_account_info(account_info_iter)?;
    if authority_info.key != &manager.owner {
        msg!("Only manager owner can create reserve");
        return Err(LendingError::InvalidManagerOwner.into());
    }
    if !authority_info.is_signer {
        msg!("authority is not a signer");
        return Err(LendingError::InvalidSigner.into());
    }

    let market_reserve = MarketReserve {
        version: PROGRAM_VERSION,
        last_update: LastUpdate::new(clock.slot),
        manager: *manager_info.key,
        market_price,
        token_info: TokenInfo {
            account: *token_account_info.key,
            price_oracle: *pyth_price_info.key,
            decimal: token_mint.decimals,
        },
        liquidity_info: LiquidityInfo {
            enable_borrow,
            rate_oracle: *rate_oracle_info.key,
            available: 0,
            borrowed_amount_wads: Decimal::zero(),
            acc_borrow_rate_wads: Decimal::one(),
            fee: 0,
            config: liquidity_config,
        },
        collateral_info: CollateralInfo {
            sotoken_mint_pubkey: *sotoken_mint_info.key,
            total_mint: 0,
            config: collateral_config,
        },
    };
    MarketReserve::pack(market_reserve, &mut market_reserve_info.try_borrow_mut_data()?)
}

fn process_update_market_reserves(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let (clock_account, accounts) = accounts
        .split_first()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    // 1
    let clock = &Clock::from_account_info(clock_account)?;
    accounts
        .chunks_exact(3)
        .try_for_each(|accounts_info| {
            // 2 + i * 3
            let market_reserve_info = &accounts_info[0];
            // 3 + i * 3
            let pyth_price_info = &accounts_info[1];
            // 4 + i * 3
            let rate_oracle_info = &accounts_info[3];
        
            if market_reserve_info.owner != program_id {
                msg!("MarketReserve owner provided is not owned by the lending program");
                return Err(LendingError::InvalidAccountOwner.into());
            }
            let mut market_reserve = MarketReserve::unpack(&market_reserve_info.try_borrow_data()?)?;
        
            if pyth_price_info.key != &market_reserve.token_info.price_oracle {
                return Err(LendingError::InvalidPriceOracle.into());
            }
            let market_price = get_pyth_price(pyth_price_info, clock)?;
        
            if rate_oracle_info.key != &market_reserve.liquidity_info.rate_oracle {
                msg!("MarketReserve liquidity rate oracle is not matched with provided");
                return Err(LendingError::InvalidRateOracle.into());
            }
            let rate_oracle = RateOracle::unpack(&rate_oracle_info.try_borrow_data()?)?;
            let borrow_rate = rate_oracle.calculate_borrow_rate(
                clock.slot,
                market_reserve.liquidity_info.utilization_rate()?,
            )?;
        
            // update
            market_reserve.market_price = market_price;
            market_reserve.accrue_interest(borrow_rate, clock.slot)?;
            market_reserve.last_update.update_slot(clock.slot, false);

            // pack
            MarketReserve::pack(market_reserve, &mut market_reserve_info.try_borrow_mut_data()?)
        })
}

fn process_exchange(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    amount: u64,
    from_collateral: bool,
) -> ProgramResult {
    if amount == 0 {
        msg!("Liquidity amount provided cannot be zero");
        return Err(LendingError::InvalidAmount.into());
    }

    let account_info_iter = &mut accounts.iter();
    // 1
    let clock = &Clock::from_account_info(next_account_info(account_info_iter)?)?;
    // 2
    let manager_info = next_account_info(account_info_iter)?;
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
    // 3
    let manager_authority_info = next_account_info(account_info_iter)?;
    if manager_authority_info.key != &manager_authority {
        msg!("Manager authority is not matched with program address derived from manager info");
        return Err(LendingError::InvalidManagerAuthority.into());
    }
    // 4
    let market_reserve_info = next_account_info(account_info_iter)?;
    if market_reserve_info.owner != program_id {
        msg!("MarketReserve owner provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut market_reserve = MarketReserve::unpack(&market_reserve_info.try_borrow_data()?)?;
    if &market_reserve.manager != manager_info.key {
        return Err(LendingError::InvalidMarketReserve.into())
    }
    // 5
    let sotoken_mint_info = next_account_info(account_info_iter)?;
    if sotoken_mint_info.key != &market_reserve.collateral_info.sotoken_mint_pubkey {
        return Err(LendingError::InvalidSoTokenMint.into())
    }
    // 6
    let manager_token_account_info = next_account_info(account_info_iter)?;
    if manager_token_account_info.key != &market_reserve.token_info.account {
        return Err(LendingError::InvalidManagerTokenAccount.into()); 
    }
    // 7
    let rate_oracle_info = next_account_info(account_info_iter)?;
    if rate_oracle_info.key != &market_reserve.liquidity_info.rate_oracle {
        msg!("MarketReserve liquidity rate oracle is not matched with provided");
        return Err(LendingError::InvalidRateOracle.into());
    }
    let rate_oracle = RateOracle::unpack(&rate_oracle_info.try_borrow_data()?)?;
    let borrow_rate = rate_oracle.calculate_borrow_rate(
        clock.slot,
        market_reserve.liquidity_info.utilization_rate()?,
    )?;
    // 8
    let user_authority_info = next_account_info(account_info_iter)?;
    // 9
    let user_token_account_info = next_account_info(account_info_iter)?;
    // 10
    let user_sotoken_account_info = next_account_info(account_info_iter)?;
    // 11
    let token_program_id = next_account_info(account_info_iter)?;

    // update in reserve
    market_reserve.accrue_interest(borrow_rate, clock.slot)?;
    market_reserve.last_update.update_slot(clock.slot, true);

    if from_collateral {
        let redeem_amount = market_reserve.exchange_collateral_to_liquidity(amount)?;
        MarketReserve::pack(market_reserve, &mut market_reserve_info.try_borrow_mut_data()?)?;
    
        // burn sotoken
        spl_token_burn(TokenBurnParams {
            mint: sotoken_mint_info.clone(),
            source: user_sotoken_account_info.clone(),
            amount,
            authority: user_authority_info.clone(),
            authority_signer_seeds: &[],
            token_program: token_program_id.clone(),
        })?;
    
        // transfer from manager to user
        spl_token_transfer(TokenTransferParams {
            source: manager_token_account_info.clone(),
            destination: user_token_account_info.clone(),
            amount: redeem_amount,
            authority: manager_authority_info.clone(),
            authority_signer_seeds,
            token_program: token_program_id.clone(),
        })
    } else {
        let mint_amount = market_reserve.exchange_liquidity_to_collateral(amount)?;
        MarketReserve::pack(market_reserve, &mut market_reserve_info.try_borrow_mut_data()?)?;

        // transfer from user to manager
        spl_token_transfer(TokenTransferParams {
            source: user_token_account_info.clone(),
            destination: manager_token_account_info.clone(),
            amount,
            authority: user_authority_info.clone(),
            authority_signer_seeds: &[],
            token_program: token_program_id.clone(),
        })?;

        // mint to user
        spl_token_mint_to(TokenMintToParams {
            mint: sotoken_mint_info.clone(),
            destination: user_sotoken_account_info.clone(),
            amount: mint_amount,
            authority: manager_authority_info.clone(),
            authority_signer_seeds,
            token_program: token_program_id.clone(),
        })
    }
}

fn process_init_user_obligation(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    // 1
    let rent = &Rent::from_account_info(next_account_info(account_info_iter)?)?;
    // 2
    let clock = &Clock::from_account_info(next_account_info(account_info_iter)?)?;
    // 3
    let manager_info = next_account_info(account_info_iter)?;
    if manager_info.owner != program_id {
        msg!("Manager ower provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    Manager::unpack(&manager_info.try_borrow_data()?)?;
    // 4
    let user_obligation_info = next_account_info(account_info_iter)?;
    if user_obligation_info.owner != program_id {
        msg!("UserObligation owner provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    assert_rent_exempt(rent, user_obligation_info)?;
    assert_uninitialized::<UserObligation>(user_obligation_info)?;
    // 5
    let owner_info = next_account_info(account_info_iter)?;

    let user_obligation = UserObligation {
        version: PROGRAM_VERSION,
        manager: *manager_info.key,
        owner: *owner_info.key,
        last_update: LastUpdate::new(clock.slot),
        friend: COption::None,
        collaterals: Vec::new(),
        collaterals_borrow_value: Decimal::zero(),
        collaterals_liquidation_value: Decimal::zero(),
        loans: Vec::new(),
        loans_value: Decimal::zero(),
    };
    UserObligation::pack(user_obligation, &mut user_obligation_info.try_borrow_mut_data()?)
}

// must after update reserves
fn process_update_user_obligation(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    if accounts.len() < 2 {
        return Err(ProgramError::NotEnoughAccountKeys);
    }
    let (ahead_accounts, accounts) = accounts.split_at(2);
    // 1
    let clock = &Clock::from_account_info(&ahead_accounts[0])?;
    // 2
    let user_obligation_info = &ahead_accounts[1];
    if user_obligation_info.owner != program_id {
        msg!("User Obligation owner provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut user_obligation = UserObligation::unpack(&user_obligation_info.try_borrow_data()?)?;
    let manager = user_obligation.manager;
    // 3 + i
    let reserves = accounts
        .iter()
        .map(|market_reserve_info| {
            if market_reserve_info.owner != program_id {
                msg!("Market reserve owner provided is not owned by the lending program");
                return Err(LendingError::InvalidAccountOwner.into());
            }

            let market_reserve = MarketReserve::unpack(&market_reserve_info.try_borrow_data()?)?;
            if market_reserve.manager != manager {
                msg!("User Obligation manager provided is matched with market reserve provided");
                return Err(LendingError::InvalidManager.into());
            }
            if market_reserve.last_update.is_stale(clock.slot)? {
                Err(LendingError::MarketReserveStale.into())
            } else {
                Ok((*market_reserve_info.key, market_reserve))
            }
        })
        .collect::<Result<Vec<_>, ProgramError>>()?;

    // update
    user_obligation.update_user_obligation(&mut reserves.iter())?;
    user_obligation.last_update.update_slot(clock.slot, false);

    // pack
    UserObligation::pack(user_obligation, &mut user_obligation_info.try_borrow_mut_data()?)
}

fn process_bind_or_unbind_friend(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    is_bind: bool,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    // 1
    let user_obligation_info = next_account_info(account_info_iter)?;
    if user_obligation_info.owner != program_id {
        msg!("User Obligation owner provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut user_obligation = UserObligation::unpack(&user_obligation_info.try_borrow_data()?)?;
    // 2
    let friend_obligation_info = next_account_info(account_info_iter)?;
    if friend_obligation_info.owner != program_id {
        msg!("Friend Obligation owner provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut friend_obligation = UserObligation::unpack(&friend_obligation_info.try_borrow_data()?)?;

    if user_obligation.manager != friend_obligation.manager {
        return Err(LendingError::UserObligationFriendNotMatched.into());
    }
    // 3
    let user_authority_info = next_account_info(account_info_iter)?;
    if user_authority_info.key != &user_obligation.owner {
        return Err(LendingError::InvalidUserAuthority.into());
    }
    if !user_authority_info.is_signer {
        msg!("authority is not a signer");
        return Err(LendingError::InvalidSigner.into());
    }
    // 4
    let friend_authority_info = next_account_info(account_info_iter)?;
    if friend_authority_info.key != &friend_obligation.owner {
        return Err(LendingError::InvalidUserAuthority.into());
    }
    if !friend_authority_info.is_signer {
        msg!("authority is not a signer");
        return Err(LendingError::InvalidSigner.into());
    }

    if is_bind {
        // bind
        user_obligation.bind_friend(*friend_obligation_info.key)?;
        friend_obligation.bind_friend(*user_obligation_info.key)?;
    } else {
        // unbind
        user_obligation.unbind_friend()?;
        friend_obligation.unbind_friend()?;
    }

    // pack
    UserObligation::pack(user_obligation, &mut user_obligation_info.try_borrow_mut_data()?)?;
    UserObligation::pack(friend_obligation, &mut friend_obligation_info.try_borrow_mut_data()?)
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
    // 1
    let market_reserve_info = next_account_info(account_info_iter)?;
    if market_reserve_info.owner != program_id {
        msg!("Market reserve owner provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let market_reserve = MarketReserve::unpack(&market_reserve_info.try_borrow_data()?)?;
    // 2
    let sotoken_mint_info = next_account_info(account_info_iter)?;
    if sotoken_mint_info.key != &market_reserve.collateral_info.sotoken_mint_pubkey {
        return Err(LendingError::InvalidSoTokenMint.into()); 
    }
    // 3
    let user_obligation_info = next_account_info(account_info_iter)?;
    if user_obligation_info.owner != program_id {
        msg!("User Obligation owner provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut user_obligation = UserObligation::unpack(&user_obligation_info.try_borrow_data()?)?;
    if user_obligation.manager != market_reserve.manager {
        msg!("User Obligation manager provided is matched with market reserve provided");
        return Err(LendingError::InvalidManager.into());
    }
    // 4
    let user_authority_info = next_account_info(account_info_iter)?;
    if user_authority_info.key != &user_obligation.owner {
        return Err(LendingError::InvalidUserAuthority.into());
    }
    // 5
    let user_sotoken_account_info = next_account_info(account_info_iter)?;
    // 6
    let token_program_id = next_account_info(account_info_iter)?;

    // handle obligation
    if let Ok(index) = user_obligation.find_collateral(*market_reserve_info.key) {
        user_obligation.deposit(amount, index)?;
    } else {
        user_obligation.new_deposit(amount, *market_reserve_info.key, &market_reserve)?;
    }

    // pack
    UserObligation::pack(user_obligation, &mut user_obligation_info.try_borrow_mut_data()?)?;
    
    // burn from user
    spl_token_burn(TokenBurnParams {
        mint: sotoken_mint_info.clone(),
        source: user_sotoken_account_info.clone(),
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
        msg!("Collateral amount provided cannot be zero");
        return Err(LendingError::InvalidAmount.into());
    }

    let account_info_iter = &mut accounts.iter();
    // 1
    let clock = &Clock::from_account_info(next_account_info(account_info_iter)?)?;
    // 2
    let manager_info = next_account_info(account_info_iter)?;
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
    // 3
    let manager_authority_info = next_account_info(account_info_iter)?;
    if manager_authority_info.key != &manager_authority {
        msg!("Manager authority is not matched with program address derived from manager info");
        return Err(LendingError::InvalidManagerAuthority.into());
    }
    // 4
    let market_reserve_info = next_account_info(account_info_iter)?;
    if market_reserve_info.owner != program_id {
        msg!("Market reserve owner provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let market_reserve = MarketReserve::unpack(&market_reserve_info.try_borrow_data()?)?;
    if &market_reserve.manager != manager_info.key {
        msg!("Market reserve manager provided is not matched with manager info");
        return Err(LendingError::InvalidMarketReserve.into());
    }
    if market_reserve.last_update.is_stale(clock.slot)? {
        return Err(LendingError::MarketReserveStale.into());
    }
    // 5
    let sotoken_mint_info = next_account_info(account_info_iter)?;
    if sotoken_mint_info.key != &market_reserve.collateral_info.sotoken_mint_pubkey {
        return Err(LendingError::InvalidSoTokenMint.into());
    }
    // 6
    let user_obligation_info = next_account_info(account_info_iter)?;
    if user_obligation_info.owner != program_id {
        msg!("User Obligation owner provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut user_obligation = UserObligation::unpack(&user_obligation_info.try_borrow_data()?)?;
    if &user_obligation.manager != manager_info.key {
        return Err(LendingError::InvalidManager.into());
    }
    if user_obligation.last_update.is_stale(clock.slot)? {
        return Err(LendingError::ObligationStale.into());
    }

    let user_obligation_2 = if let COption::Some(friend) = user_obligation.friend.as_ref() {
        // 7
        let user_obligation_2_info = next_account_info(account_info_iter)?;
        if user_obligation_2_info.key != friend {
            return Err(LendingError::UserObligationFriendNotMatched.into());
        }
        let user_obligation_2 = UserObligation::unpack(&user_obligation_2_info.try_borrow_data()?)?;
        if user_obligation_2.last_update.is_stale(clock.slot)? {
            return Err(LendingError::ObligationStale.into());
        }

        Some(user_obligation_2)
    } else {
        None
    };
    // 7/8
    let user_authority_info = next_account_info(account_info_iter)?;
    if user_authority_info.key != &user_obligation.owner {
        return Err(LendingError::InvalidUserAuthority.into());
    }
    if !user_authority_info.is_signer {
        msg!("authority is not a signer");
        return Err(LendingError::InvalidSigner.into());
    }
    // 8/9
    let user_sotoken_account_info = next_account_info(account_info_iter)?;
    // 9/10
    let token_program_id = next_account_info(account_info_iter)?;

    // redeem in obligation
    let index = user_obligation.find_collateral(*market_reserve_info.key)?;
    let amount = user_obligation.redeem(amount, index, &market_reserve, user_obligation_2)?;
    
    // pack
    UserObligation::pack(user_obligation, &mut user_obligation_info.try_borrow_mut_data()?)?;
    
    // mint to user
    spl_token_mint_to(TokenMintToParams {
        mint: sotoken_mint_info.clone(),
        destination: user_sotoken_account_info.clone(),
        amount,
        authority: manager_authority_info.clone(),
        authority_signer_seeds,
        token_program: token_program_id.clone(),
    })
}

fn process_redeem_collateral_without_loan(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    amount: u64,
) -> ProgramResult {
    if amount == 0 {
        msg!("Collateral amount provided cannot be zero");
        return Err(LendingError::InvalidAmount.into());
    }

    let account_info_iter = &mut accounts.iter();
    // 1
    let manager_info = next_account_info(account_info_iter)?;
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
    // 2
    let manager_authority_info = next_account_info(account_info_iter)?;
    if manager_authority_info.key != &manager_authority {
        msg!("Manager authority is not matched with program address derived from manager info");
        return Err(LendingError::InvalidManagerAuthority.into());
    }
    // 3
    let market_reserve_info = next_account_info(account_info_iter)?;
    if market_reserve_info.owner != program_id {
        msg!("Market reserve owner provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let market_reserve = MarketReserve::unpack(&market_reserve_info.try_borrow_data()?)?;
    if &market_reserve.manager != manager_info.key {
        msg!("Market reserve manager provided is not matched with manager info");
        return Err(LendingError::InvalidMarketReserve.into());
    }
    // 4
    let sotoken_mint_info = next_account_info(account_info_iter)?;
    if sotoken_mint_info.key != &market_reserve.collateral_info.sotoken_mint_pubkey {
        return Err(LendingError::InvalidSoTokenMint.into());
    }
    // 5
    let user_obligation_info = next_account_info(account_info_iter)?;
    if user_obligation_info.owner != program_id {
        msg!("User Obligation owner provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut user_obligation = UserObligation::unpack(&user_obligation_info.try_borrow_data()?)?;
    if &user_obligation.manager != manager_info.key {
        return Err(LendingError::InvalidManager.into());
    }

    let user_obligation_2 = if let COption::Some(friend) = user_obligation.friend.as_ref() {
        // 6
        let user_obligation_2_info = next_account_info(account_info_iter)?;
        if user_obligation_2_info.key != friend {
            return Err(LendingError::UserObligationFriendNotMatched.into());
        }
        let user_obligation_2 = UserObligation::unpack(&user_obligation_2_info.try_borrow_data()?)?;

        Some(user_obligation_2)
    } else {
        None
    };
    // 6/7
    let user_authority_info = next_account_info(account_info_iter)?;
    if user_authority_info.key != &user_obligation.owner {
        return Err(LendingError::InvalidUserAuthority.into());
    }
    if !user_authority_info.is_signer {
        msg!("authority is not a signer");
        return Err(LendingError::InvalidSigner.into());
    }
    // 7/8
    let user_sotoken_account_info = next_account_info(account_info_iter)?;
    // 8/9
    let token_program_id = next_account_info(account_info_iter)?;

    // redeem in obligation
    let index = user_obligation.find_collateral(*market_reserve_info.key)?;
    let amount = user_obligation.redeem_without_loan(amount, index, user_obligation_2)?;
    
    // pack
    UserObligation::pack(user_obligation, &mut user_obligation_info.try_borrow_mut_data()?)?;
    
    // mint to user
    spl_token_mint_to(TokenMintToParams {
        mint: sotoken_mint_info.clone(),
        destination: user_sotoken_account_info.clone(),
        amount,
        authority: manager_authority_info.clone(),
        authority_signer_seeds,
        token_program: token_program_id.clone(),
    })
}

fn process_replace_collateral(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    out_amount: u64,
    in_amount: u64,
) -> ProgramResult {
    if out_amount == 0 || in_amount == 0 {
        msg!("Collateral amount provided cannot be zero");
        return Err(LendingError::InvalidAmount.into());
    }

    let account_info_iter = &mut accounts.iter();
    // 1
    let clock = &Clock::from_account_info(next_account_info(account_info_iter)?)?;
    // 2
    let manager_info = next_account_info(account_info_iter)?;
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
    // 3
    let manager_authority_info = next_account_info(account_info_iter)?;
    if manager_authority_info.key != &manager_authority {
        msg!("Manager authority is not matched with program address derived from manager info");
        return Err(LendingError::InvalidManagerAuthority.into());
    }
    // 4
    let out_market_reserve_info = next_account_info(account_info_iter)?;
    if out_market_reserve_info.owner != program_id {
        msg!("Out market reserve owner provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let out_market_reserve = MarketReserve::unpack(&out_market_reserve_info.try_borrow_data()?)?;
    if &out_market_reserve.manager != manager_info.key {
        msg!("Out market reserve manager provided is not matched with manager info");
        return Err(LendingError::InvalidMarketReserve.into());
    }
    if out_market_reserve.last_update.is_stale(clock.slot)? {
        return Err(LendingError::MarketReserveStale.into());
    }
    // 5
    let out_sotoken_mint_info = next_account_info(account_info_iter)?;
    if out_sotoken_mint_info.key != &out_market_reserve.collateral_info.sotoken_mint_pubkey {
        return Err(LendingError::InvalidSoTokenMint.into());
    }
    // 6
    let in_market_reserve_info = next_account_info(account_info_iter)?;
    if in_market_reserve_info.owner != program_id {
        msg!("In market reserve owner provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let in_market_reserve = MarketReserve::unpack(&in_market_reserve_info.try_borrow_data()?)?;
    if &in_market_reserve.manager != manager_info.key {
        msg!("In market reserve manager provided is not matched with manager info");
        return Err(LendingError::InvalidMarketReserve.into());
    }
    if in_market_reserve.last_update.is_stale(clock.slot)? {
        return Err(LendingError::MarketReserveStale.into());
    }
    // 7
    let in_sotoken_mint_info = next_account_info(account_info_iter)?;
    if in_sotoken_mint_info.key != &in_market_reserve.collateral_info.sotoken_mint_pubkey {
        return Err(LendingError::InvalidSoTokenMint.into());
    }
    // 8
    let user_obligation_info = next_account_info(account_info_iter)?;
    if user_obligation_info.owner != program_id {
        msg!("User Obligation owner provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut user_obligation = UserObligation::unpack(&user_obligation_info.try_borrow_data()?)?;
    if &user_obligation.manager != manager_info.key {
        return Err(LendingError::InvalidManager.into());
    }
    if user_obligation.last_update.is_stale(clock.slot)? {
        return Err(LendingError::ObligationStale.into());
    }

    let user_obligation_2 = if let COption::Some(friend) = user_obligation.friend.as_ref() {
        // 9
        let user_obligation_2_info = next_account_info(account_info_iter)?;
        if user_obligation_2_info.key != friend {
            return Err(LendingError::UserObligationFriendNotMatched.into());
        }
        let user_obligation_2 = UserObligation::unpack(&user_obligation_2_info.try_borrow_data()?)?;
        if user_obligation_2.last_update.is_stale(clock.slot)? {
            return Err(LendingError::ObligationStale.into());
        }

        Some(user_obligation_2)
    } else {
        None
    };
    // 9/10
    let user_authority_info = next_account_info(account_info_iter)?;
    if user_authority_info.key != &user_obligation.owner {
        return Err(LendingError::InvalidUserAuthority.into());
    }
    if !user_authority_info.is_signer {
        msg!("authority is not a signer");
        return Err(LendingError::InvalidSigner.into());
    }
    // 10/11
    let user_out_sotoken_account_info = next_account_info(account_info_iter)?;
    // 12/13
    let user_in_sotoken_account_info = next_account_info(account_info_iter)?;
    // 13/14
    let token_program_id = next_account_info(account_info_iter)?;

    // replace
    let out_index = user_obligation.find_collateral(*out_market_reserve_info.key)?;
    let out_amount = if let Ok(in_index) = user_obligation.find_collateral(*in_market_reserve_info.key) {
        user_obligation.replace_collateral(
            out_amount,
            in_amount,
            out_index,
            in_index,
            &out_market_reserve,
            &in_market_reserve,
            user_obligation_2,
        )?
    } else {
        user_obligation.new_replace_collateral(
            out_amount,
            in_amount,
            out_index,
            *in_market_reserve_info.key,
            &out_market_reserve,
            &in_market_reserve,
            user_obligation_2,
        )?
    };

    // pack
    UserObligation::pack(user_obligation, &mut user_obligation_info.try_borrow_mut_data()?)?;

    // mint to user
    spl_token_mint_to(TokenMintToParams {
        mint: out_sotoken_mint_info.clone(),
        destination: user_out_sotoken_account_info.clone(),
        amount: out_amount,
        authority: manager_authority_info.clone(),
        authority_signer_seeds,
        token_program: token_program_id.clone(),
    })?;

    // burn from user
    spl_token_burn(TokenBurnParams {
        mint: in_sotoken_mint_info.clone(),
        source: user_in_sotoken_account_info.clone(),
        amount: in_amount,
        authority: user_authority_info.clone(),
        authority_signer_seeds: &[],
        token_program: token_program_id.clone(),
    })
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
    // 1
    let clock = &Clock::from_account_info(next_account_info(account_info_iter)?)?;
    // 2
    let manager_info = next_account_info(account_info_iter)?;
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
    // 3
    let manager_authority_info = next_account_info(account_info_iter)?;
    if manager_authority_info.key != &manager_authority {
        msg!("Manager authority is not matched with program address derived from manager info");
        return Err(LendingError::InvalidManagerAuthority.into());
    }
    // 4
    let market_reserve_info = next_account_info(account_info_iter)?;
    if market_reserve_info.owner != program_id {
        msg!("Market reserve owner provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut market_reserve = MarketReserve::unpack(&market_reserve_info.try_borrow_data()?)?;
    if &market_reserve.manager != manager_info.key {
        msg!("MarketReserve manager provided is not matched with manager info");
        return Err(LendingError::InvalidMarketReserve.into());
    }
    if market_reserve.last_update.is_stale(clock.slot)? {
        return Err(LendingError::MarketReserveStale.into());
    }
    // 5
    let manager_token_account_info = next_account_info(account_info_iter)?;
    if manager_token_account_info.key != &market_reserve.token_info.account {
        return Err(LendingError::InvalidManagerTokenAccount.into()); 
    }
    // 6
    let user_obligation_info = next_account_info(account_info_iter)?;
    if user_obligation_info.owner != program_id {
        msg!("User Obligation owner provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut user_obligation = UserObligation::unpack(&user_obligation_info.try_borrow_data()?)?;
    if &user_obligation.manager != manager_info.key {
        return Err(LendingError::InvalidManager.into());
    }
    if user_obligation.last_update.is_stale(clock.slot)? {
        return Err(LendingError::ObligationStale.into());
    }
    
    let user_obligation_2 = if let COption::Some(friend) = user_obligation.friend.as_ref() {
        // 7
        let user_obligation_2_info = next_account_info(account_info_iter)?;
        if user_obligation_2_info.key != friend {
            return Err(LendingError::UserObligationFriendNotMatched.into());
        }
        let user_obligation_2 = UserObligation::unpack(&user_obligation_2_info.try_borrow_data()?)?;
        if user_obligation_2.last_update.is_stale(clock.slot)? {
            return Err(LendingError::ObligationStale.into());
        }

        Some(user_obligation_2)
    } else {
        None
    };
    // 7/8
    let user_authority_info = next_account_info(account_info_iter)?;
    if user_authority_info.key != &user_obligation.owner {
        return Err(LendingError::InvalidUserAuthority.into());
    }
    if !user_authority_info.is_signer {
        msg!("authority is not a signer");
        return Err(LendingError::InvalidSigner.into());
    }
    // 8/9
    let user_token_account_info = next_account_info(account_info_iter)?;
    // 9/10
    let token_program_id = next_account_info(account_info_iter)?;

    // borrow
    let borrow_with_fee = if let Ok(index) = user_obligation.find_loan(*market_reserve_info.key) {
        user_obligation.borrow_in(amount, index, &market_reserve, user_obligation_2)?
    } else {
        user_obligation.new_borrow_in(amount, *market_reserve_info.key, &market_reserve, user_obligation_2)?
    };
    let amount = borrow_with_fee.receiving()?;
    market_reserve.liquidity_info.borrow_out(borrow_with_fee)?;

    // pack
    UserObligation::pack(user_obligation, &mut user_obligation_info.try_borrow_mut_data()?)?;
    MarketReserve::pack(market_reserve, &mut market_reserve_info.try_borrow_mut_data()?)?;

    // transfer token to user
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
        msg!("Liquidity amount provided cannot be zero");
        return Err(LendingError::InvalidAmount.into());
    }

    let account_info_iter = &mut accounts.iter();
    // 1
    let clock = &Clock::from_account_info(next_account_info(account_info_iter)?)?;
    // 2
    let market_reserve_info = next_account_info(account_info_iter)?;
    if market_reserve_info.owner != program_id {
        msg!("Market reserve owner provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut market_reserve = MarketReserve::unpack(&market_reserve_info.try_borrow_data()?)?;
    // 3
    let manager_token_account_info = next_account_info(account_info_iter)?;
    if manager_token_account_info.key != &market_reserve.token_info.account {
        return Err(LendingError::InvalidManagerTokenAccount.into())
    }
    // 4
    let rate_oracle_info = next_account_info(account_info_iter)?;
    if rate_oracle_info.key != &market_reserve.liquidity_info.rate_oracle {
        return Err(LendingError::InvalidRateOracle.into());
    }
    let rate_oracle = RateOracle::unpack(&rate_oracle_info.try_borrow_data()?)?;
    let borrow_rate = rate_oracle.calculate_borrow_rate(
        clock.slot,
        market_reserve.liquidity_info.utilization_rate()?,
    )?;
    // 5
    let user_obligation_info = next_account_info(account_info_iter)?;
    if user_obligation_info.owner != program_id {
        msg!("User Obligation owner provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut user_obligation = UserObligation::unpack(&user_obligation_info.try_borrow_data()?)?;
    if user_obligation.manager != market_reserve.manager {
        msg!("User Obligation manager provided is matched with market reserve provided");
        return Err(LendingError::InvalidManager.into());
    }
    // 6
    let user_authority_info = next_account_info(account_info_iter)?;
    // 7
    let user_token_account_info = next_account_info(account_info_iter)?;
    // 8
    let token_program_id = next_account_info(account_info_iter)?;    

    // accrue interest
    market_reserve.accrue_interest(borrow_rate, clock.slot)?;
    market_reserve.last_update.update_slot(clock.slot, true);
    // accrue interest
    let index = user_obligation.find_loan(*market_reserve_info.key)?;
    user_obligation.loans[index].accrue_interest(&market_reserve)?;

    // repay
    let amount = user_obligation.repay(amount, index)?;
    market_reserve.liquidity_info.repay(amount)?;

    // pack
    UserObligation::pack(user_obligation, &mut user_obligation_info.try_borrow_mut_data()?)?;
    MarketReserve::pack(market_reserve, &mut market_reserve_info.try_borrow_mut_data()?)?;

    // transfer to manager
    spl_token_transfer(TokenTransferParams {
        source: user_token_account_info.clone(),
        destination: manager_token_account_info.clone(),
        amount,
        authority: user_authority_info.clone(),
        authority_signer_seeds: &[],
        token_program: token_program_id.clone(),
    })
}

fn process_liquidate(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    amount: u64,
) -> ProgramResult {
    if amount == 0 {
        msg!("Collateral amount provided cannot be zero");
        return Err(LendingError::InvalidAmount.into());
    }

    let account_info_iter = &mut accounts.iter();
    // 1
    let clock = &Clock::from_account_info(next_account_info(account_info_iter)?)?;
    // 2
    let manager_info = next_account_info(account_info_iter)?;
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
    // 3
    let manager_authority_info = next_account_info(account_info_iter)?;
    if manager_authority_info.key != &manager_authority {
        msg!("Manager authority is not matched with program address derived from manager info");
        return Err(LendingError::InvalidManagerAuthority.into());
    }
    // 4
    let collateral_market_reserve_info = next_account_info(account_info_iter)?;
    if collateral_market_reserve_info.owner != program_id {
        msg!("Collateral market reserve owner provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let collateral_market_reserve = MarketReserve::unpack(&collateral_market_reserve_info.try_borrow_data()?)?;
    if &collateral_market_reserve.manager != manager_info.key {
        msg!("Collateral market reserve manager provided is not matched with manager info");
        return Err(LendingError::InvalidMarketReserve.into());
    }
    if collateral_market_reserve.last_update.is_stale(clock.slot)? {
        return Err(LendingError::MarketReserveStale.into());
    }
    // 5
    let sotoken_mint_info = next_account_info(account_info_iter)?;
    if sotoken_mint_info.key != &collateral_market_reserve.collateral_info.sotoken_mint_pubkey {
        return Err(LendingError::InvalidSoTokenMint.into());
    }
    // 6
    let loan_market_reserve_info = next_account_info(account_info_iter)?;
    if loan_market_reserve_info.owner != program_id {
        msg!("Loan market reserve owner provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut loan_market_reserve = MarketReserve::unpack(&loan_market_reserve_info.try_borrow_data()?)?;
    if &loan_market_reserve.manager != manager_info.key {
        msg!("Loan market reserve manager provided is not matched with manager info");
        return Err(LendingError::InvalidMarketReserve.into());
    }
    if loan_market_reserve.last_update.is_stale(clock.slot)? {
        return Err(LendingError::MarketReserveStale.into());
    }
    // 7
    let manager_token_account_info = next_account_info(account_info_iter)?;
    if manager_token_account_info.key != &loan_market_reserve.token_info.account {
        return Err(LendingError::InvalidManagerTokenAccount.into());
    }
    // 8
    let user_obligation_info = next_account_info(account_info_iter)?;
    if user_obligation_info.owner != program_id {
        msg!("User Obligation owner provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut user_obligation = UserObligation::unpack(&user_obligation_info.try_borrow_data()?)?;
    if &user_obligation.manager != manager_info.key {
        return Err(LendingError::InvalidManager.into());
    }
    if user_obligation.last_update.is_stale(clock.slot)? {
        return Err(LendingError::ObligationStale.into());
    }

    let user_obligation_2 = if let COption::Some(friend) = user_obligation.friend.as_ref() {
        // 9
        let user_obligation_2_info = next_account_info(account_info_iter)?;
        if user_obligation_2_info.key != friend {
            return Err(LendingError::UserObligationFriendNotMatched.into());
        }
        let user_obligation_2 = UserObligation::unpack(&user_obligation_2_info.try_borrow_data()?)?;
        if user_obligation_2.last_update.is_stale(clock.slot)? {
            return Err(LendingError::ObligationStale.into());
        }

        Some(user_obligation_2)
    } else {
        None
    };
    // 10/11
    let liquidator_authority_info = next_account_info(account_info_iter)?;
    // 11/12
    let liquidator_token_account_info = next_account_info(account_info_iter)?;
    // 12/13
    let liquidator_sotoken_account_info = next_account_info(account_info_iter)?;
    // 13/14
    let token_program_id = next_account_info(account_info_iter)?;

    // liquidate
    let collateral_index = user_obligation.find_collateral(*collateral_market_reserve_info.key)?;
    let loan_index = user_obligation.find_collateral(*loan_market_reserve_info.key)?;
    let (amount, liquidation_with_fee) = user_obligation.liquidate(
        amount,
        collateral_index,
        loan_index,
        &collateral_market_reserve,
        &loan_market_reserve,
        user_obligation_2,
    )?;
    let repay_amount = liquidation_with_fee.need_pay()?;
    loan_market_reserve.liquidity_info.liquidate(liquidation_with_fee)?;

    // pack
    UserObligation::pack(user_obligation, &mut user_obligation_info.try_borrow_mut_data()?)?;
    MarketReserve::pack(loan_market_reserve, &mut loan_market_reserve_info.try_borrow_mut_data()?)?;

    // transfer token to manager
    spl_token_transfer(TokenTransferParams {
        source: liquidator_token_account_info.clone(),
        destination: manager_token_account_info.clone(),
        amount: repay_amount,
        authority: liquidator_authority_info.clone(),
        authority_signer_seeds: &[],
        token_program: token_program_id.clone(),
    })?;

    // mint to user
    spl_token_mint_to(TokenMintToParams {
        mint: sotoken_mint_info.clone(),
        destination: liquidator_sotoken_account_info.clone(),
        amount,
        authority: manager_authority_info.clone(),
        authority_signer_seeds,
        token_program: token_program_id.clone(),
    })
}

// by rate oracle manager
fn process_feed_rate_oracle(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    asset_index: u64,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    // 1
    let clock = &Clock::from_account_info(next_account_info(account_info_iter)?)?;
    // 2
    let rate_oracle_info = next_account_info(account_info_iter)?;
    if rate_oracle_info.owner != program_id {
        msg!("Rate oracle owner provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut rate_oracle = RateOracle::unpack(&rate_oracle_info.try_borrow_data()?)?;
    // 3
    let authority_info = next_account_info(account_info_iter)?;
    if !authority_info.is_signer {
        msg!("authority is not a signer");
        return Err(LendingError::InvalidSigner.into());
    }
    if authority_info.key != &rate_oracle.owner {
        return Err(LendingError::InvalidOracleAuthority.into())
    }

    rate_oracle.feed_asset_index(clock.slot, asset_index)?;
    RateOracle::pack(rate_oracle, &mut rate_oracle_info.try_borrow_mut_data()?)
}

// by rate oracle manager
fn process_operate_rate_oracle<P: Any + Copy + Param>(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    param: P,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    // 1
    let rate_oracle_info = next_account_info(account_info_iter)?;
    if rate_oracle_info.owner != program_id {
        msg!("Rate oracle owner provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut rate_oracle = RateOracle::unpack(&rate_oracle_info.try_borrow_data()?)?;
    // 2
    let authority_info = next_account_info(account_info_iter)?;
    if !authority_info.is_signer {
        msg!("authority is not a signer");
        return Err(LendingError::InvalidSigner.into());
    }
    if authority_info.key != &rate_oracle.owner {
        return Err(LendingError::InvalidOracleAuthority.into())
    }

    rate_oracle.operate_checked(param)?;
    RateOracle::pack(rate_oracle, &mut rate_oracle_info.try_borrow_mut_data()?)
}

// by manager
fn process_operate_market_reserve<P: Any + Copy + Param>(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    param: P,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    // 1
    let manager_info = next_account_info(account_info_iter)?;
    if manager_info.owner != program_id {
        msg!("Manager ower provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let manager = Manager::unpack(&manager_info.try_borrow_data()?)?;
    // 2
    let market_reserve_info = next_account_info(account_info_iter)?;
    if market_reserve_info.owner != program_id {
        msg!("MarketReserve owner provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut market_reserve = MarketReserve::unpack(&market_reserve_info.try_borrow_data()?)?;
    if &market_reserve.manager != manager_info.key {
        msg!("MarketReserve manager provided is not matched with manager info");
        return Err(LendingError::InvalidMarketReserve.into());
    }
    // 3
    let authority_info = next_account_info(account_info_iter)?;
    if authority_info.key != &manager.owner {
        msg!("Only manager owner can create reserve");
        return Err(LendingError::InvalidManagerOwner.into());
    }
    if !authority_info.is_signer {
        msg!("authority is not a signer");
        return Err(LendingError::InvalidSigner.into());
    }

    market_reserve.operate_checked(param)?;
    // pack
    MarketReserve::pack(market_reserve, &mut market_reserve_info.try_borrow_mut_data()?)
}

// by manager
fn process_withdraw_fee(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    amount: u64,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    // 1
    let manager_info = next_account_info(account_info_iter)?;
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
    // 2
    let manager_authority_info = next_account_info(account_info_iter)?;
    if manager_authority_info.key != &manager_authority {
        msg!("Manager authority is not matched with program address derived from manager info");
        return Err(LendingError::InvalidManagerAuthority.into());
    }
    // 3
    let market_reserve_info = next_account_info(account_info_iter)?;
    if market_reserve_info.owner != program_id {
        msg!("MarketReserve owner provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut market_reserve = MarketReserve::unpack(&market_reserve_info.try_borrow_data()?)?;
    if &market_reserve.manager != manager_info.key {
        msg!("MarketReserve manager provided is not matched with manager info");
        return Err(LendingError::InvalidMarketReserve.into());
    }
    // 4
    let manager_token_account_info = next_account_info(account_info_iter)?;
    if manager_token_account_info.key != &market_reserve.token_info.account {
        return Err(LendingError::InvalidManagerTokenAccount.into()); 
    }
    // 5
    let authority_info = next_account_info(account_info_iter)?;
    if authority_info.key != &manager.owner {
        msg!("Only manager owner can withdraw fee");
        return Err(LendingError::InvalidManagerOwner.into());
    }
    if !authority_info.is_signer {
        msg!("authority is not a signer");
        return Err(LendingError::InvalidSigner.into());
    }
    // 6
    let receiver_token_account_info = next_account_info(account_info_iter)?;
    // 7
    let token_program_id = next_account_info(account_info_iter)?;

    // withdraw fee
    market_reserve.liquidity_info.withdraw_fee(amount)?;
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

#[cfg(feature = "case-injection")]
fn process_inject_case(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    is_liquidation: bool,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    // 1
    let user_obligation_info = next_account_info(account_info_iter)?;
    if user_obligation_info.owner != program_id {
        msg!("User Obligation owner provided is not owned by the lending program");
        return Err(LendingError::InvalidAccountOwner.into());
    }
    let mut user_obligation = UserObligation::unpack(&user_obligation_info.try_borrow_data()?)?;

    if is_liquidation {
        user_obligation.loans_value = user_obligation.collaterals_liquidation_value
    } else {
        user_obligation.loans_value = user_obligation.collaterals_borrow_value;
    }
    // pack
    UserObligation::pack(user_obligation, &mut user_obligation_info.try_borrow_mut_data()?)
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

/// Invoke signed unless signers seeds are empty
#[inline(always)]
fn invoke_optionally_signed(
    instruction: &Instruction,
    account_infos: &[AccountInfo],
    authority_signer_seeds: &[&[u8]],
) -> ProgramResult {
    if authority_signer_seeds.is_empty() {
        invoke(instruction, account_infos)
    } else {
        invoke_signed(instruction, account_infos, &[authority_signer_seeds])
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
    let result = invoke_optionally_signed(
        &spl_token::instruction::transfer(
            token_program.key,
            source.key,
            destination.key,
            authority.key,
            &[],
            amount,
        )?,
        &[source, destination, authority, token_program],
        authority_signer_seeds,
    );
    result.map_err(|_| LendingError::TokenTransferFailed.into())
}

/// Issue a spl_token `MintTo` instruction.
fn spl_token_mint_to(params: TokenMintToParams<'_, '_>) -> ProgramResult {
    let TokenMintToParams {
        mint,
        destination,
        authority,
        token_program,
        amount,
        authority_signer_seeds,
    } = params;
    let result = invoke_optionally_signed(
        &spl_token::instruction::mint_to(
            token_program.key,
            mint.key,
            destination.key,
            authority.key,
            &[],
            amount,
        )?,
        &[mint, destination, authority, token_program],
        authority_signer_seeds,
    );
    result.map_err(|_| LendingError::TokenMintToFailed.into())
}

/// Issue a spl_token `Burn` instruction.
#[inline(always)]
fn spl_token_burn(params: TokenBurnParams<'_, '_>) -> ProgramResult {
    let TokenBurnParams {
        mint,
        source,
        authority,
        token_program,
        amount,
        authority_signer_seeds,
    } = params;
    let result = invoke_optionally_signed(
        &spl_token::instruction::burn(
            token_program.key,
            source.key,
            mint.key,
            authority.key,
            &[],
            amount,
        )?,
        &[source, mint, authority, token_program],
        authority_signer_seeds,
    );
    result.map_err(|_| LendingError::TokenBurnFailed.into())
}

struct TokenTransferParams<'a: 'b, 'b> {
    source: AccountInfo<'a>,
    destination: AccountInfo<'a>,
    amount: u64,
    authority: AccountInfo<'a>,
    authority_signer_seeds: &'b [&'b [u8]],
    token_program: AccountInfo<'a>,
}

struct TokenMintToParams<'a: 'b, 'b> {
    mint: AccountInfo<'a>,
    destination: AccountInfo<'a>,
    amount: u64,
    authority: AccountInfo<'a>,
    authority_signer_seeds: &'b [&'b [u8]],
    token_program: AccountInfo<'a>,
}

struct TokenBurnParams<'a: 'b, 'b> {
    mint: AccountInfo<'a>,
    source: AccountInfo<'a>,
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