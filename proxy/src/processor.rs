#![allow(missing_docs)]
use num_traits::FromPrimitive;
use typenum::{Bit, True, False, Unsigned, U1, U9};
use crate::{
    error::ProxyError,
    instruction::{ProxyInstruction, SwapInput, RouterSwapInput},
};
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    decode_error::DecodeError,
    entrypoint::ProgramResult,
    instruction::{Instruction, AccountMeta},
    program::{invoke, invoke_signed},
    program_pack::Pack,
    msg,
    sysvar::{Sysvar, rent::Rent},
    system_instruction,
    system_program,
    program_error::{ProgramError, PrintProgramError},
    pubkey::Pubkey,
};
use spl_token::{
    state::{Account, Mint},
    native_mint,
    instruction as token_instruction,
};
use soda_lending::{
    state::{Manager, MarketReserve, UserObligation, CollateralConfig, LiquidityConfig, RateModel},
    oracle::OracleConfig,
    instruction as lending_instruction,
};
use spl_associated_token_account::create_associated_token_account;

const RESERVE_LAMPORTS: u64 = 1_000_000;

/// Processes an instruction
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    input: &[u8],
) -> ProgramResult {
    let instruction = ProxyInstruction::unpack(input)?;
    match instruction {
        ProxyInstruction::CreateManager => {
            process_create_manager(accounts)
        }
        ProxyInstruction::CreateMarketReserve(
            oracle_config,
            collateral_config,
            liquidity_config,
            rate_model,
        ) => {
            process_create_market_reserve(
                accounts,
                oracle_config,
                collateral_config,
                liquidity_config,
                rate_model,
            )
        }
        ProxyInstruction::DepositAndPledge(amount) => {
            process_deposit_and_pledge(program_id, accounts, amount)
        }
        ProxyInstruction::RedeemAndWithdraw(amount) => {
            process_redeem_and_withdraw_or_borrow::<False, True>(accounts, amount)
        }
        ProxyInstruction::RedeemWithoutLoanAndWithdraw(amount) => {
            process_redeem_and_withdraw_or_borrow::<False, False>(accounts, amount)
        }
        ProxyInstruction::Borrow(amount) => {
            process_redeem_and_withdraw_or_borrow::<True, False>(accounts, amount)
        }
        ProxyInstruction::Repay(amount) => {
            process_repay(accounts, amount)
        }
        ProxyInstruction::SolanaRouterSwap(input) => {
            process_solana_router_swap(accounts, input)
        }
        ProxyInstruction::RaydiumRouterSwap(input) => {
            process_raydium_router_swap(accounts, input)
        }
        ProxyInstruction::SaberRouterSwap(input) => {
            process_saber_router_swap(accounts, input)
        }
        ProxyInstruction::SolanaSwap(input) => {
            process_solana_swap(accounts, input)
        }
        ProxyInstruction::RaydiumSwap(input) => {
            process_raydium_swap(accounts, input)
        }
        ProxyInstruction::SaberSwap(input) => {
            process_saber_swap(accounts, input)
        }
    }
}

fn process_create_manager(accounts: &[AccountInfo]) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();

    let rent_info = next_account_info(account_info_iter)?;
    let manager_info = next_account_info(account_info_iter)?;
    let authority_info = next_account_info(account_info_iter)?;
    let lending_program_info = next_account_info(account_info_iter)?;
    let system_program_info = next_account_info(account_info_iter)?;

    _process_create_account::<Manager>(
        rent_info,
        manager_info,
        authority_info,
        system_program_info,
        lending_program_info.key,
    )?;

    invoke(
        &lending_instruction::init_manager(
            *manager_info.key,
            *authority_info.key,
        ),
        &[
            rent_info.clone(),
            manager_info.clone(),
            authority_info.clone(),
            lending_program_info.clone(),
        ],
    )
}

#[inline(never)]
fn process_create_market_reserve(
    accounts: &[AccountInfo],
    oracle_config: OracleConfig,
    collateral_config: CollateralConfig,
    liquidity_config: LiquidityConfig,
    rate_model: RateModel,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();

    let rent_info = next_account_info(account_info_iter)?;
    let clock_info = next_account_info(account_info_iter)?;
    let manager_info = next_account_info(account_info_iter)?;
    let manager_authority_info = next_account_info(account_info_iter)?;
    let supply_token_account_info = next_account_info(account_info_iter)?;
    let market_reserve_info = next_account_info(account_info_iter)?;
    let token_mint_info = next_account_info(account_info_iter)?;
    let sotoken_mint_info = next_account_info(account_info_iter)?;
    let authority_info = next_account_info(account_info_iter)?;
    let token_program_info = next_account_info(account_info_iter)?;
    let lending_program_info = next_account_info(account_info_iter)?;
    let system_program_info = next_account_info(account_info_iter)?;

    _process_create_account::<MarketReserve>(
        rent_info,
        market_reserve_info,
        authority_info,
        system_program_info,
        lending_program_info.key,
    )?;

    _process_create_account::<Account>(
        rent_info,
        supply_token_account_info,
        authority_info,
        system_program_info,
        token_program_info.key,
    )?;

    _process_create_account::<Mint>(
        rent_info,
        sotoken_mint_info,
        authority_info,
        system_program_info,
        token_program_info.key,
    )?;

    invoke(
        &lending_instruction::init_market_reserve(
            *manager_info.key,
            *supply_token_account_info.key,
            *market_reserve_info.key,
            *token_mint_info.key,
            *sotoken_mint_info.key,
            *authority_info.key,
            oracle_config,
            collateral_config,
            liquidity_config,
            rate_model,
        ),
        &[
            rent_info.clone(),
            clock_info.clone(),
            manager_info.clone(),
            manager_authority_info.clone(),
            supply_token_account_info.clone(),
            market_reserve_info.clone(),
            token_mint_info.clone(),
            sotoken_mint_info.clone(),
            authority_info.clone(),
            token_program_info.clone(),
            lending_program_info.clone(),
        ],
    )
}

#[inline(never)]
fn process_deposit_and_pledge(program_id: &Pubkey, accounts: &[AccountInfo], amount: u64) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();

    let rent_info = next_account_info(account_info_iter)?;
    let clock_info = next_account_info(account_info_iter)?;
    let manager_info = next_account_info(account_info_iter)?;
    let market_reserve_info = next_account_info(account_info_iter)?;
    let supply_mint_info = next_account_info(account_info_iter)?;
    let supply_token_account_info = next_account_info(account_info_iter)?;
    let user_obligation_info = next_account_info(account_info_iter)?;
    let authority_info = next_account_info(account_info_iter)?;
    let user_token_account_info = next_account_info(account_info_iter)?;
    let token_program_info = next_account_info(account_info_iter)?;
    let lending_program_info = next_account_info(account_info_iter)?;
    let system_program_info = next_account_info(account_info_iter)?;
    let spl_associated_program_info = next_account_info(account_info_iter)?;

    _process_create_user_obligation(
        program_id,
        rent_info,
        clock_info,
        manager_info,
        user_obligation_info,
        authority_info,
        lending_program_info,
        system_program_info,
    )?;

    _process_transfer_to_native_token_account(
        rent_info,
        supply_mint_info,
        user_token_account_info,
        authority_info,
        token_program_info,
        system_program_info,
        spl_associated_program_info,
        amount,
    )?;

    invoke(
        &lending_instruction::deposit_and_pledge(
            *market_reserve_info.key,
            *supply_token_account_info.key,
            *user_obligation_info.key,
            *authority_info.key,
            *user_token_account_info.key,
            amount,
        ),
        &[
            clock_info.clone(),
            market_reserve_info.clone(),
            supply_token_account_info.clone(),
            user_obligation_info.clone(),
            authority_info.clone(),
            user_token_account_info.clone(),
            token_program_info.clone(),
            lending_program_info.clone(),
        ],
    )?;

    _process_close_account(
        supply_mint_info,
        user_token_account_info,
        authority_info,
        token_program_info,
    )
}

#[inline(never)]
fn process_repay(accounts: &[AccountInfo], amount: u64) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();

    let rent_info = next_account_info(account_info_iter)?;
    let clock_info = next_account_info(account_info_iter)?;
    let market_reserve_info = next_account_info(account_info_iter)?;
    let supply_mint_info = next_account_info(account_info_iter)?;
    let supply_token_account_info = next_account_info(account_info_iter)?;
    let user_obligation_info = next_account_info(account_info_iter)?;
    let authority_info = next_account_info(account_info_iter)?;
    let user_token_account_info = next_account_info(account_info_iter)?;
    let token_program_info = next_account_info(account_info_iter)?;
    let lending_program_info = next_account_info(account_info_iter)?;
    let system_program_info = next_account_info(account_info_iter)?;
    let spl_associated_program_info = next_account_info(account_info_iter)?;

    _process_transfer_to_native_token_account(
        rent_info,
        supply_mint_info,
        user_token_account_info,
        authority_info,
        token_program_info,
        system_program_info,
        spl_associated_program_info,
        amount,
    )?;

    invoke(
        &lending_instruction::repay_loan(
            *market_reserve_info.key,
            *supply_token_account_info.key,
            *user_obligation_info.key,
            *authority_info.key,
            *user_token_account_info.key,
            amount,
        ),
        &[
            clock_info.clone(),
            market_reserve_info.clone(),
            supply_token_account_info.clone(),
            user_obligation_info.clone(),
            authority_info.clone(),
            user_token_account_info.clone(),
            token_program_info.clone(),
            lending_program_info.clone(),
        ],
    )?;

    _process_close_account(
        supply_mint_info,
        user_token_account_info,
        authority_info,
        token_program_info,
    )
}

#[inline(never)]
fn process_redeem_and_withdraw_or_borrow<IsBorrow: Bit, WithLoan: Bit>(
    accounts: &[AccountInfo],
    amount: u64,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();

    let rent_info = next_account_info(account_info_iter)?;
    let clock_info = next_account_info(account_info_iter)?;
    let manager_info = next_account_info(account_info_iter)?;
    let manager_authority_info = next_account_info(account_info_iter)?;
    let market_reserve_info = next_account_info(account_info_iter)?;
    let supply_mint_info = next_account_info(account_info_iter)?;
    let supply_token_account_info = next_account_info(account_info_iter)?;
    let user_obligation_info = next_account_info(account_info_iter)?;
    let authority_info = next_account_info(account_info_iter)?;
    let user_token_account_info = next_account_info(account_info_iter)?;
    let token_program_info = next_account_info(account_info_iter)?;
    let lending_program_info = next_account_info(account_info_iter)?;
    let system_program_info = next_account_info(account_info_iter)?;
    let spl_associated_program_info = next_account_info(account_info_iter)?;

    _process_create_associated_token_account(
        rent_info,
        supply_mint_info,
        user_token_account_info,
        authority_info,
        token_program_info,
        system_program_info,
        spl_associated_program_info,
    )?;

    let account_infos = &[
        clock_info.clone(),
        manager_info.clone(),
        manager_authority_info.clone(),
        market_reserve_info.clone(),
        supply_token_account_info.clone(),
        user_obligation_info.clone(),
        authority_info.clone(),
        user_token_account_info.clone(),
        token_program_info.clone(),
        lending_program_info.clone(),
    ];

    if IsBorrow::BOOL {
        invoke(
            &lending_instruction::borrow_liquidity(
                *manager_info.key,
                *market_reserve_info.key,
                *supply_token_account_info.key,
                *user_obligation_info.key,
                None,
                *authority_info.key,
                *user_token_account_info.key,
                amount,
            ),
            account_infos,
        )?;
    } else {
        invoke(
            &lending_instruction::redeem_and_withdraw::<WithLoan>(
                *manager_info.key,
                *market_reserve_info.key,
                *supply_token_account_info.key,
                *user_obligation_info.key,
                None,
                *authority_info.key,
                *user_token_account_info.key,
                amount,
            ),
            account_infos,
        )?;
    }

    _process_close_account(
        supply_mint_info,
        user_token_account_info,
        authority_info,
        token_program_info,
    )
}

#[inline(never)]
fn process_solana_router_swap(accounts: &[AccountInfo], router_input: RouterSwapInput) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();

    let user_authority_info = next_account_info(account_info_iter)?;
    let token_program_info = next_account_info(account_info_iter)?;
    let swap_program_info = next_account_info(account_info_iter)?;

    let mut pair_accounts = Vec::with_capacity(router_input.router_num as usize + 1);
    for _ in 0..router_input.router_num + 1 {
        let user_token_account = next_account_info(account_info_iter)?;
        let pool_token_account = next_account_info(account_info_iter)?;
        pair_accounts.push((user_token_account, pool_token_account));
    }

    let mut input = SwapInput { amount_in: router_input.amount_in, minimum_amount_out: 0 };
    for i in 0..router_input.router_num as usize {
        let authority_info = next_account_info(account_info_iter)?;
        let swap_info = next_account_info(account_info_iter)?;
        let pool_mint_info = next_account_info(account_info_iter)?;
        let pool_fee_account_info = next_account_info(account_info_iter)?;

        let (user_source_info, swap_source_info) = pair_accounts[i];
        let (user_dest_info, swap_dest_info) = pair_accounts[i + 1];

        if i + 1 == router_input.router_num as usize {
            input.minimum_amount_out = router_input.minimum_amount_out;
        }

        type SolanaSwapTag = U1;
        input.amount_in = _process_swap_and_return_balance::<SolanaSwapTag>(
            input,
            user_dest_info,
            swap_program_info,
            vec![
                swap_info.clone(),
                authority_info.clone(),
                user_authority_info.clone(),
                user_source_info.clone(),
                swap_source_info.clone(),
                swap_dest_info.clone(),
                user_dest_info.clone(),
                pool_mint_info.clone(),
                pool_fee_account_info.clone(),
                token_program_info.clone(),
            ],
        )?;
    }

    Ok(())
}

#[inline(never)]
fn process_raydium_router_swap(accounts: &[AccountInfo], router_input: RouterSwapInput) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();

    let amm_authority_info = next_account_info(account_info_iter)?;
    let user_authority_info = next_account_info(account_info_iter)?;
    let token_program_info = next_account_info(account_info_iter)?;
    let swap_program_info = next_account_info(account_info_iter)?;
    let serum_program_info = next_account_info(account_info_iter)?;

    let mut pair_accounts = Vec::with_capacity(router_input.router_num as usize + 1);
    for _ in 0..router_input.router_num + 1 {
        let user_token_account = next_account_info(account_info_iter)?;
        let pool_token_account = next_account_info(account_info_iter)?;
        // serum
        let serum_vault_account = next_account_info(account_info_iter)?;

        pair_accounts.push((user_token_account, pool_token_account, serum_vault_account));
    }

    let mut input = SwapInput { amount_in: router_input.amount_in, minimum_amount_out: 0 };
    for i in 0..router_input.router_num as usize {
        let amm_info = next_account_info(account_info_iter)?;
        let amm_open_orders_info = next_account_info(account_info_iter)?;
        let amm_target_orders_info = next_account_info(account_info_iter)?;
        let serum_market_info =  next_account_info(account_info_iter)?;
        let serum_bids_info = next_account_info(account_info_iter)?;
        let serum_asks_info = next_account_info(account_info_iter)?;
        let serum_event_queue_info = next_account_info(account_info_iter)?;
        let serum_vault_signer_info = next_account_info(account_info_iter)?;

        let (user_source_account_info, pool_coin_account_info, serum_coin_vault_account_info) = pair_accounts[i];
        let (user_dest_account_info, pool_pc_account_info, serum_pc_vault_account_info) = pair_accounts[i + 1];
        
        if i + 1 == router_input.router_num as usize {
            input.minimum_amount_out = router_input.minimum_amount_out;
        }

        type RaydiumSwapTag = U9;
        input.amount_in = _process_swap_and_return_balance::<RaydiumSwapTag>(
            input,
            user_dest_account_info,
            swap_program_info,
            vec![
                token_program_info.clone(),
                amm_info.clone(),
                amm_authority_info.clone(),
                amm_open_orders_info.clone(),
                amm_target_orders_info.clone(),
                pool_coin_account_info.clone(),
                pool_pc_account_info.clone(),
                serum_program_info.clone(),
                serum_market_info.clone(),
                serum_bids_info.clone(),
                serum_asks_info.clone(),
                serum_event_queue_info.clone(),
                serum_coin_vault_account_info.clone(),
                serum_pc_vault_account_info.clone(),
                serum_vault_signer_info.clone(),
                user_source_account_info.clone(),
                user_dest_account_info.clone(),
                user_authority_info.clone(),
            ],
        )?;
    }

    Ok(())
}

#[inline(never)]
fn process_saber_router_swap(accounts: &[AccountInfo], router_input: RouterSwapInput) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();

    let clock_info = next_account_info(account_info_iter)?;
    let user_authority_info = next_account_info(account_info_iter)?;
    let token_program_info = next_account_info(account_info_iter)?;
    let swap_program_info = next_account_info(account_info_iter)?;

    let mut pair_accounts = Vec::with_capacity(router_input.router_num as usize + 1);
    for _ in 0..router_input.router_num + 1 {
        let user_token_account = next_account_info(account_info_iter)?;
        let pool_token_account = next_account_info(account_info_iter)?;
        pair_accounts.push((user_token_account, pool_token_account));
    }

    let mut input = SwapInput { amount_in: router_input.amount_in, minimum_amount_out: 0 };
    for i in 0..router_input.router_num as usize {
        let swap_info = next_account_info(account_info_iter)?;
        let swap_authority_info = next_account_info(account_info_iter)?;
        let admin_fee_dest_info = next_account_info(account_info_iter)?;

        let (user_source_info, swap_source_info) = pair_accounts[i];
        let (user_dest_info, swap_dest_info) = pair_accounts[i + 1];

        if i + 1 == router_input.router_num as usize {
            input.minimum_amount_out = router_input.minimum_amount_out;
        }

        type SaberSwapTag = U1;
        input.amount_in = _process_swap_and_return_balance::<SaberSwapTag>(
            input,
            user_dest_info,
            swap_program_info,
            vec![
                swap_info.clone(),
                swap_authority_info.clone(),
                user_authority_info.clone(),
                user_source_info.clone(),
                swap_source_info.clone(),
                swap_dest_info.clone(),
                user_dest_info.clone(),
                admin_fee_dest_info.clone(),
                token_program_info.clone(),
                clock_info.clone(),
            ],
        )?;
    }

    Ok(())
}

#[inline(never)]
fn process_solana_swap(accounts: &[AccountInfo], input: SwapInput) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    // inherent accounts input
    let _clock_info = next_account_info(account_info_iter)?;
    let lending_source_account_info = next_account_info(account_info_iter)?;
    let lending_dest_account_info = next_account_info(account_info_iter)?;
    let user_authority_info = next_account_info(account_info_iter)?;
    let token_program_info = next_account_info(account_info_iter)?;
    // solana
    let swap_info = next_account_info(account_info_iter)?;
    let authority_info = next_account_info(account_info_iter)?;
    let swap_source_info = next_account_info(account_info_iter)?;
    let swap_dest_info = next_account_info(account_info_iter)?;
    let user_dest_info = next_account_info(account_info_iter)?;
    let pool_mint_info = next_account_info(account_info_iter)?;
    let pool_fee_account_info = next_account_info(account_info_iter)?;
    // swap program id
    let solana_swap_program_info = next_account_info(account_info_iter)?;

    // Solana swap tag is 1
    type SolanaSwapTag = U1;
    _process_swap_and_repay_loan::<SolanaSwapTag>(
        input,
        lending_dest_account_info,
        user_dest_info,
        user_authority_info,
        solana_swap_program_info,
        token_program_info,
        vec![
            swap_info.clone(),
            authority_info.clone(),
            user_authority_info.clone(),
            lending_source_account_info.clone(),
            swap_source_info.clone(),
            swap_dest_info.clone(),
            user_dest_info.clone(),
            pool_mint_info.clone(),
            pool_fee_account_info.clone(),
            token_program_info.clone(),
        ],
    )
}

#[inline(never)]
fn process_raydium_swap(accounts: &[AccountInfo], input: SwapInput) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    // from flash liquidation instruction: inherent accounts input
    let _clock_info = next_account_info(account_info_iter)?;
    let lending_source_account_info = next_account_info(account_info_iter)?;
    let lending_dest_account_info = next_account_info(account_info_iter)?;
    let user_authority_info = next_account_info(account_info_iter)?;
    let token_program_info = next_account_info(account_info_iter)?;
    // raydium amm
    let amm_id = next_account_info(account_info_iter)?;
    let amm_authority_info = next_account_info(account_info_iter)?;
    let amm_open_orders_info = next_account_info(account_info_iter)?;
    let amm_target_orders_info = next_account_info(account_info_iter)?;
    let pool_dest_account_info = next_account_info(account_info_iter)?;
    let pool_source_account_info = next_account_info(account_info_iter)?;
    // serum
    let serum_program_info = next_account_info(account_info_iter)?;
    let serum_market_info = next_account_info(account_info_iter)?;
    let serum_bids_info = next_account_info(account_info_iter)?;
    let serum_asks_info = next_account_info(account_info_iter)?;
    let serum_event_queue_info = next_account_info(account_info_iter)?;
    let serum_dest_vault_account_info = next_account_info(account_info_iter)?;
    let serum_source_vault_account_info = next_account_info(account_info_iter)?;
    let serum_vault_signer_info = next_account_info(account_info_iter)?;
    // user
    let user_dest_account_info = next_account_info(account_info_iter)?;
    // swap program id
    let raydium_program_info = next_account_info(account_info_iter)?;

    // raydium swap tag is 9
    type RaydiumSwapTag = U9;
    _process_swap_and_repay_loan::<RaydiumSwapTag>(
        input,
        lending_dest_account_info,
        user_dest_account_info,
        user_authority_info,
        raydium_program_info,
        token_program_info,
        vec![
            token_program_info.clone(),
            amm_id.clone(),
            amm_authority_info.clone(),
            amm_open_orders_info.clone(),
            amm_target_orders_info.clone(),
            pool_dest_account_info.clone(),
            pool_source_account_info.clone(),
            serum_program_info.clone(),
            serum_market_info.clone(),
            serum_bids_info.clone(),
            serum_asks_info.clone(),
            serum_event_queue_info.clone(),
            serum_dest_vault_account_info.clone(),
            serum_source_vault_account_info.clone(),
            serum_vault_signer_info.clone(),
            lending_source_account_info.clone(),
            user_dest_account_info.clone(),
            user_authority_info.clone(),
        ],
    )
}

#[inline(never)]
fn process_saber_swap(accounts: &[AccountInfo], input: SwapInput) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    // from flash liquidation instruction: inherent accounts input
    let clock_info = next_account_info(account_info_iter)?;
    let lending_source_account_info = next_account_info(account_info_iter)?;
    let lending_dest_account_info = next_account_info(account_info_iter)?;
    let user_authority_info = next_account_info(account_info_iter)?;
    let token_program_info = next_account_info(account_info_iter)?;
    // saber
    let swap_info = next_account_info(account_info_iter)?;
    let swap_authority_info = next_account_info(account_info_iter)?;
    let swap_source_info = next_account_info(account_info_iter)?;
    let swap_dest_info = next_account_info(account_info_iter)?;
    let user_dest_info = next_account_info(account_info_iter)?;
    let admin_fee_dest_info = next_account_info(account_info_iter)?;
    // swap program id
    let saber_program_info = next_account_info(account_info_iter)?;

    // saber swap tag is 1
    type SaberSwapTag = U1;
    _process_swap_and_repay_loan::<SaberSwapTag>(
        input,
        lending_dest_account_info,
        user_dest_info,
        user_authority_info,
        saber_program_info,
        token_program_info,
        vec![
            swap_info.clone(),
            swap_authority_info.clone(),
            user_authority_info.clone(),
            lending_source_account_info.clone(),
            swap_source_info.clone(),
            swap_dest_info.clone(),
            user_dest_info.clone(),
            admin_fee_dest_info.clone(),
            token_program_info.clone(),
            clock_info.clone(),
        ],
    )
}

#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn _process_create_user_obligation<'a>(
    program_id: &Pubkey,
    rent_info: &AccountInfo<'a>,
    clock_info: &AccountInfo<'a>,
    manager_info: &AccountInfo<'a>,
    user_obligation_info: &AccountInfo<'a>,
    authority_info: &AccountInfo<'a>,
    lending_program_info: &AccountInfo<'a>,
    system_program_info: &AccountInfo<'a>,
) -> ProgramResult {
    if user_obligation_info.owner == &system_program::id() {
        let (associated_user_obligation_address, bump_seed) = Pubkey::find_program_address(
            &[
                lending_program_info.key.as_ref(),
                manager_info.key.as_ref(),
                authority_info.key.as_ref(),
            ],
            program_id,
        );
        if &associated_user_obligation_address != user_obligation_info.key {
            msg!("Associated user obligation address does not match seed derivation");
            return Err(ProgramError::InvalidSeeds);
        }
    
        _process_create_account_2::<UserObligation>(
            rent_info,
            user_obligation_info,
            authority_info,
            system_program_info,
            lending_program_info.key,
            &[
                lending_program_info.key.as_ref(),
                manager_info.key.as_ref(),
                authority_info.key.as_ref(),
                &[bump_seed],
            ],
        )?;
    
        invoke(
            &lending_instruction::init_user_obligation(
                *manager_info.key,
                *user_obligation_info.key,
                *authority_info.key,
            ),
            &[
                rent_info.clone(),
                clock_info.clone(),
                manager_info.clone(),
                user_obligation_info.clone(),
                authority_info.clone(),
                lending_program_info.clone(),
            ],
        )?;
    }

    Ok(())
}

fn _process_create_account<'a, P: Pack>(
    rent_info: &AccountInfo<'a>,
    target_account_info: &AccountInfo<'a>,
    authority_info: &AccountInfo<'a>,
    system_program_info: &AccountInfo<'a>,
    owner: &Pubkey,
) -> ProgramResult {
    let rent = &Rent::from_account_info(rent_info)?;
    let required_lamports = rent.minimum_balance(P::LEN);
        
    invoke(
        &system_instruction::create_account(
            authority_info.key,
            target_account_info.key,
            required_lamports,
            P::LEN as u64,
            owner,
        ),
        &[
            authority_info.clone(),
            target_account_info.clone(),
            system_program_info.clone(),
        ],
    )
}

#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn _process_create_account_2<'a, P: Pack>(
    rent_info: &AccountInfo<'a>,
    target_account_info: &AccountInfo<'a>,
    authority_info: &AccountInfo<'a>,
    system_program_info: &AccountInfo<'a>,
    owner: &Pubkey,
    signer_seeds: &[&[u8]],
) -> ProgramResult {
    let rent = &Rent::from_account_info(rent_info)?;
    let required_lamports = rent
        .minimum_balance(P::LEN)
        .saturating_sub(target_account_info.lamports());

    if required_lamports > 0 {
        invoke(
            &system_instruction::transfer(
                authority_info.key,
                target_account_info.key,
                required_lamports,
            ),
            &[
                authority_info.clone(),
                target_account_info.clone(),
                system_program_info.clone(),
            ],
        )?;
    }

    invoke_optionally_signed(
        &system_instruction::allocate(target_account_info.key, P::LEN as u64),
        &[target_account_info.clone(), system_program_info.clone()],
        signer_seeds,
    )?;

    invoke_optionally_signed(
        &system_instruction::assign(target_account_info.key, owner),
        &[target_account_info.clone(), system_program_info.clone()],
        signer_seeds,
    )
}

#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn _process_transfer_to_native_token_account<'a>(
    rent_info: &AccountInfo<'a>,
    supply_mint_info: &AccountInfo<'a>,
    user_token_account_info: &AccountInfo<'a>,
    authority_info: &AccountInfo<'a>,
    token_program_info: &AccountInfo<'a>,
    system_program_info: &AccountInfo<'a>,
    spl_associated_program_info: &AccountInfo<'a>,
    amount: u64,
) -> ProgramResult {
    if supply_mint_info.key == &native_mint::id() {
        _process_create_associated_token_account(
            rent_info,
            supply_mint_info,
            user_token_account_info,
            authority_info,
            token_program_info,
            system_program_info,
            spl_associated_program_info,   
        )?;

        let rent = &Rent::from_account_info(rent_info)?;

        let required_lamports = if amount == u64::MAX {
            authority_info.lamports().saturating_sub(RESERVE_LAMPORTS)
        } else {
            rent
                .minimum_balance(Account::LEN)
                .checked_add(amount)
                .ok_or(ProxyError::MathOverflow)?
                .saturating_sub(user_token_account_info.lamports())
        };

        if required_lamports > 0 {
            invoke(
                &system_instruction::transfer(
                    authority_info.key,
                    user_token_account_info.key,
                    required_lamports,
                ),
                &[
                    authority_info.clone(),
                    user_token_account_info.clone(),
                    system_program_info.clone(),
                ],
            )?;

            spl_token_sync_native(SyncNativeParams {
                account: user_token_account_info.clone(),
                token_program: token_program_info.clone(),
            })?;
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn _process_create_associated_token_account<'a>(
    rent_info: &AccountInfo<'a>,
    supply_mint_info: &AccountInfo<'a>,
    user_token_account_info: &AccountInfo<'a>,
    authority_info: &AccountInfo<'a>,
    token_program_info: &AccountInfo<'a>,
    system_program_info: &AccountInfo<'a>,
    spl_associated_program_info: &AccountInfo<'a>,
) -> ProgramResult {
    if user_token_account_info.owner == &system_program::id() {
        invoke(
            &create_associated_token_account(
                authority_info.key,
                authority_info.key,
                supply_mint_info.key,
            ),
            &[
                authority_info.clone(),
                user_token_account_info.clone(),
                supply_mint_info.clone(),
                system_program_info.clone(),
                token_program_info.clone(),
                rent_info.clone(),
                spl_associated_program_info.clone(),
            ],
        )?;
    }

    Ok(())
}

fn _process_close_account<'a>(
    supply_mint_info: &AccountInfo<'a>,
    user_token_account_info: &AccountInfo<'a>,
    authority_info: &AccountInfo<'a>,
    token_program_info: &AccountInfo<'a>,
) -> ProgramResult {
    if supply_mint_info.key == &native_mint::id() {
        spl_token_close_account(CloseAccountParams {
            account: user_token_account_info.clone(),
            authority: authority_info.clone(),
            token_program: token_program_info.clone(),
        })?;
    }

    Ok(())
}

fn _process_swap_and_return_balance<'a, T: Unsigned>(
    input: SwapInput,
    user_dest_account_info: &AccountInfo<'a>,
    swap_program_info: &AccountInfo<'a>,
    mut account_infos: Vec<AccountInfo<'a>>,
) -> Result<u64, ProgramError> {
    let instruction_accounts = account_infos
        .iter()
        .map(|account_info| {
            AccountMeta {
                pubkey: *account_info.key,
                is_signer: account_info.is_signer,
                is_writable: account_info.is_writable,  
            }
        }).collect::<Vec<_>>();
    account_infos.push(swap_program_info.clone());

    let mut data = Vec::with_capacity(1 + 8 + 8);
    data.push(T::U8);
    data.extend_from_slice(&input.amount_in.to_le_bytes());
    data.extend_from_slice(&input.minimum_amount_out.to_le_bytes());

    invoke(
        &Instruction {
            program_id: *swap_program_info.key,
            accounts: instruction_accounts,
            data,
        },
        &account_infos,
    )?;

    let token_account = Account::unpack(&user_dest_account_info.try_borrow_data()?)?;
    Ok(token_account.amount)
}

#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn _process_swap_and_repay_loan<'a, T: Unsigned>(
    input: SwapInput,
    lending_dest_account: &AccountInfo<'a>,
    user_dest_account_info: &AccountInfo<'a>,
    user_authority: &AccountInfo<'a>,
    swap_program_info: &AccountInfo<'a>,
    token_program_info: &AccountInfo<'a>,
    mut account_infos: Vec<AccountInfo<'a>>,
) -> ProgramResult {
    let instruction_accounts = account_infos
        .iter()
        .map(|account_info| {
            AccountMeta {
                pubkey: *account_info.key,
                is_signer: account_info.is_signer,
                is_writable: account_info.is_writable,  
            }
        }).collect::<Vec<_>>();
        account_infos.push(swap_program_info.clone());

    let mut data = Vec::with_capacity(1 + 8 + 8);
    data.push(T::U8);
    data.extend_from_slice(&input.amount_in.to_le_bytes());
    data.extend_from_slice(&input.minimum_amount_out.to_le_bytes());

    invoke(
        &Instruction {
            program_id: *swap_program_info.key,
            accounts: instruction_accounts,
            data,
        },
        &account_infos,
    )?;

    // repay to lending program
    spl_token_transfer(TokenTransferParams {
        source: user_dest_account_info.clone(),
        destination: lending_dest_account.clone(),
        amount: input.minimum_amount_out,
        authority: user_authority.clone(),
        authority_signer_seeds: &[],
        token_program: token_program_info.clone(),
    })
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
        &token_instruction::transfer(
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
    result.map_err(|_| ProxyError::TokenTransferFailed.into())
}

#[inline(always)]
fn spl_token_close_account(params: CloseAccountParams<'_>) -> ProgramResult {
    let CloseAccountParams {
        account,
        authority,
        token_program,
    } = params;
    let result = invoke(
        &token_instruction::close_account(
            token_program.key,
            account.key,
            authority.key,
            authority.key,
            &[],
        )?,
        &[account, authority, token_program],
    );
    result.map_err(|_| ProxyError::TokenAccountCloseFailed.into())
}

#[inline(always)]
fn spl_token_sync_native(params: SyncNativeParams<'_>) -> ProgramResult {
    let SyncNativeParams {
        account,
        token_program,
    } = params;
    let result = invoke(
        &token_instruction::sync_native(
            token_program.key,
            account.key,
        )?,
        &[account, token_program],
    );
    result.map_err(|_| ProxyError::TokenAccountSyncNativeFailed.into())
}

struct TokenTransferParams<'a: 'b, 'b> {
    source: AccountInfo<'a>,
    destination: AccountInfo<'a>,
    amount: u64,
    authority: AccountInfo<'a>,
    authority_signer_seeds: &'b [&'b [u8]],
    token_program: AccountInfo<'a>,
}

struct CloseAccountParams<'a> {
    account: AccountInfo<'a>,
    authority: AccountInfo<'a>,
    token_program: AccountInfo<'a>,
}

struct SyncNativeParams<'a> {
    account: AccountInfo<'a>,
    token_program: AccountInfo<'a>,
}

impl PrintProgramError for ProxyError {
    fn print<E>(&self)
    where
        E: 'static + std::error::Error + DecodeError<E> + PrintProgramError + FromPrimitive,
    {
        msg!(&self.to_string());
    }
}