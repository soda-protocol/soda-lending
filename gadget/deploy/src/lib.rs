#![allow(missing_docs)]
pub mod types;

use std::{str::FromStr, time::Duration, error::Error, thread};

use solana_client::{
    blockhash_query::BlockhashQuery, 
    rpc_client::RpcClient, 
    rpc_request::TokenAccountsFilter,
};
use solana_sdk::{
    commitment_config::CommitmentConfig, 
    hash::Hash, instruction::Instruction, 
    program_error::ProgramError, 
    program_pack::Pack, 
    pubkey::Pubkey, 
    signer::{Signer, keypair::Keypair}, 
    system_instruction::create_account, 
    transaction::Transaction
};
use spl_token::{
    instruction::{initialize_mint, initialize_account, mint_to},
    state::{Mint, Account},
};
use soda_lending_contract::{
    math::WAD,
    state::{
        Manager, MarketReserve, RateOracle, UserAsset, UserObligation, 
        CollateralConfig, LiquidityConfig
    },
    instruction::{
        init_manager, init_rate_oracle, init_market_reserve_without_liquidity,
        init_market_reserve_with_liquidity, init_user_obligation,
        init_user_asset, deposit_liquidity, withdraw_liquidity,
        deposit_collateral, update_user_obligation, borrow_liquidity, repay_loan,
        redeem_collateral, redeem_collateral_without_loan, liquidate, feed_rate_oracle,
        pause_rate_oracle, add_liquidity_to_market_reserve, withdraw_fee,
    },
    pyth::{self, Product},
};

const PYTH_ID: &str = "gSbePebfvPy7tRqimPoVecS2UsBvYv46ynrzWocc92s";
const QUOTE_CURRENCY: &[u8; 32] = &[85, 83, 68, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

#[allow(clippy::too_many_arguments)]
pub fn create_token(
    mint: &Keypair,
    authority: &Keypair,
    account: &Keypair,
    mint_lamports: u64,
    acnt_lamports: u64,
    decimals: u8,
    amount: u64,
    recent_blockhash: Hash,
) -> Result<Transaction, ProgramError> {
    let program_id = spl_token::id();
    let mint_pubkey = &mint.pubkey();
    let account_pubkey = &account.pubkey();
    let authority_pubkey = &authority.pubkey();

    Ok(Transaction::new_signed_with_payer(&[
            create_account(
                authority_pubkey,
                mint_pubkey,
                mint_lamports,
                Mint::LEN as u64,
                &program_id,
            ),
            create_account(
                authority_pubkey,
                account_pubkey,
                acnt_lamports,
                Account::LEN as u64,
                &program_id,
            ),
            initialize_mint(
                &program_id,
                mint_pubkey,
                authority_pubkey,
                None,
                decimals,
            )?,
            initialize_account(
                &program_id,
                account_pubkey,
                mint_pubkey,
                authority_pubkey,
            )?,
            mint_to(
                &program_id,
                mint_pubkey,
                account_pubkey,
                authority_pubkey,
                &[authority_pubkey],
                amount
            )?,
        ],
        Some(authority_pubkey),
        &[mint, account, authority],
        recent_blockhash,
    ))
}

fn create_lending_manager(
    manager: Keypair,
    authority: Keypair,
    lamports: u64,
    recent_blockhash: Hash,
) -> Transaction {
    let manager_key = &manager.pubkey();
    let authority_key = &authority.pubkey();
    let pyth_id = Pubkey::from_str(PYTH_ID).unwrap();

    Transaction::new_signed_with_payer(&[
        create_account(
            authority_key,
            manager_key,
            lamports,
            Manager::LEN as u64,
            &soda_lending_contract::id(),
        ),
        init_manager(
            *manager_key,
            *authority_key,
            pyth_id,
            *QUOTE_CURRENCY,
        )
    ],
    Some(authority_key),
        &[&manager, &authority],
        recent_blockhash,
    )
}

fn create_rate_oracle(
    rate_oracle: Keypair,
    authority: Keypair,
    lamports: u64,
    recent_blockhash: Hash,
) -> Transaction {
    let authority_key = &authority.pubkey();
    let rate_oracle_key = &rate_oracle.pubkey();

    Transaction::new_signed_with_payer(&[
        create_account(
            authority_key,
            rate_oracle_key,
            lamports,
            RateOracle::LEN as u64,
            &soda_lending_contract::id(),
        ),
        init_rate_oracle(
            *rate_oracle_key,
            *authority_key,
        )
    ],
    Some(authority_key),
        &[&rate_oracle, &authority],
        recent_blockhash,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn create_market_reserve_without_liquidity(
    authority: Keypair,
    manager_key: Pubkey,
    pyth_product_key: Pubkey,
    pyth_price_key: Pubkey,
    mint_pubkey: Pubkey,
    config: CollateralConfig,
    reserve_lamports: u64,
    account_lamports: u64,
    recent_blockhash: Hash,
) -> Result<Transaction, ProgramError> {
    let market_reserve = Keypair::new();
    let token_account = Keypair::new();
    let market_reserve_key = &market_reserve.pubkey();
    let token_account_key = &token_account.pubkey();

    println!("market reserve key: {:?}, token account key: {:?}", market_reserve_key, token_account_key);

    let token_program_id = &spl_token::id();
    let authority_key = &authority.pubkey();
    let (ref manager_authority_key, _bump_seed) = Pubkey::find_program_address(
        &[manager_key.as_ref()],
        &soda_lending_contract::id(),
    );

    Ok(Transaction::new_signed_with_payer(&[
        create_account(
            authority_key,
            token_account_key,
            account_lamports,
            Account::LEN as u64,
            token_program_id,
        ),
        create_account(
            authority_key,
            market_reserve_key,
            reserve_lamports,
            MarketReserve::LEN as u64,
            &soda_lending_contract::id(),
        ),
        initialize_account(
            token_program_id,
            token_account_key,
            &mint_pubkey,
            manager_authority_key,
        )?,
        init_market_reserve_without_liquidity(
            manager_key,
            *market_reserve_key,
            pyth_product_key,
            pyth_price_key,
            mint_pubkey,
            *token_account_key,
            *authority_key,
            config,
        )
    ],
    Some(authority_key),
        &[&market_reserve, &token_account, &authority],
        recent_blockhash,
    ))
}

#[allow(clippy::too_many_arguments)]
pub fn create_market_reserve_with_liquidity(
    authority: Keypair,
    manager_key: Pubkey,
    pyth_product_key: Pubkey,
    pyth_price_key: Pubkey,
    mint_pubkey: Pubkey,
    rate_oracle_pubkey: Pubkey,
    collateral_config: CollateralConfig,
    liquidity_config: LiquidityConfig,
    reserve_lamports: u64,
    account_lamports: u64,
    recent_blockhash: Hash,
) -> Result<Transaction, ProgramError> {
    let market_reserve = Keypair::new();
    let token_account = Keypair::new();
    let market_reserve_key = &market_reserve.pubkey();
    let token_account_key = &token_account.pubkey();

    println!("market reserve key: {:?}, token account key: {:?}", market_reserve_key, token_account_key);

    let token_program_id = &spl_token::id();
    let authority_pubkey = &authority.pubkey();
    let (ref manager_authority_key, _bump_seed) = Pubkey::find_program_address(
        &[manager_key.as_ref()],
        &soda_lending_contract::id(),
    );

    Ok(Transaction::new_signed_with_payer(&[
        create_account(
            authority_pubkey,
            token_account_key,
            account_lamports,
            Account::LEN as u64,
            token_program_id,
        ),
        create_account(
            authority_pubkey,
            market_reserve_key,
            reserve_lamports,
            MarketReserve::LEN as u64,
            &soda_lending_contract::id(),
        ),
        initialize_account(
            token_program_id,
            token_account_key,
            &mint_pubkey,
            manager_authority_key,
        )?,
        init_market_reserve_with_liquidity(
            manager_key,
            *market_reserve_key,
            pyth_product_key,
            pyth_price_key,
            mint_pubkey,
            *token_account_key,
            *authority_pubkey,
            rate_oracle_pubkey,
            collateral_config,
            liquidity_config,
        )
    ],
    Some(authority_pubkey),
        &[&market_reserve, &token_account, &authority],
        recent_blockhash,
    ))
}

pub fn create_user_obligation(
    authority: Keypair,
    market_reserve_key: Pubkey,
    lamports: u64,
    recent_blockhash: Hash,
) -> Transaction {
    let obligation = Keypair::new();
    let obligation_key = &obligation.pubkey();
    let authority_pubkey = &authority.pubkey();

    println!("obligation key: {:?}", obligation_key);

    Transaction::new_signed_with_payer(&[
        create_account(
            authority_pubkey,
            obligation_key,
            lamports,
            UserObligation::LEN as u64,
            &soda_lending_contract::id(),
        ),
        init_user_obligation(
            market_reserve_key,
            *obligation_key,
            *authority_pubkey,
        ),
    ],
    Some(authority_pubkey),
        &[&obligation, &authority],
        recent_blockhash,
    )
}

pub fn create_user_asset(
    authority: Keypair,
    market_reserve_key: Pubkey,
    lamports: u64,
    recent_blockhash: Hash,
) -> Transaction {
    let asset = Keypair::new();
    let asset_key = &asset.pubkey();
    let authority_pubkey = &authority.pubkey();

    println!("User asset key: {:?}", asset_key);

    Transaction::new_signed_with_payer(&[
        create_account(
            authority_pubkey,
            asset_key,
            lamports,
            UserAsset::LEN as u64,
            &soda_lending_contract::id(),
        ),
        init_user_asset(
            market_reserve_key,
            *asset_key,
            *authority_pubkey,
        ),
    ],
    Some(authority_pubkey),
        &[&asset, &authority],
        recent_blockhash,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn do_deposit_liquidity(
    user_authority: Keypair,
    market_reserve_key: Pubkey,
    manager_token_account_key: Pubkey,
    rate_oracle_key: Pubkey,
    user_asset_key: Pubkey,
    user_token_account_key: Pubkey,
    amount: u64,
    recent_blockhash: Hash,
) -> Transaction {
    let user_authority_key = &user_authority.pubkey();

    Transaction::new_signed_with_payer(&[
        deposit_liquidity(
            market_reserve_key,
            manager_token_account_key,
            rate_oracle_key,
            user_asset_key,
            *user_authority_key,
            user_token_account_key,
            amount,
        ),
    ],
    Some(user_authority_key),
        &[&user_authority],
        recent_blockhash,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn do_withdraw_liquidity(
    user_authority: Keypair,
    manager_key: Pubkey,
    market_reserve_key: Pubkey,
    manager_token_account_key: Pubkey,
    rate_oracle_key: Pubkey,
    user_asset_key: Pubkey,
    user_token_account_key: Pubkey,
    amount: u64,
    recent_blockhash: Hash,
) -> Transaction {
    let user_authority_key = &user_authority.pubkey();

    Transaction::new_signed_with_payer(&[
        withdraw_liquidity(
            manager_key,
            market_reserve_key,
            manager_token_account_key,
            rate_oracle_key,
            user_asset_key,
            *user_authority_key,
            user_token_account_key,
            amount,
        ),
    ],
    Some(user_authority_key),
        &[&user_authority],
        recent_blockhash,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn do_deposit_collateral(
    user_authority: Keypair,
    market_reserve_key: Pubkey,
    manager_token_account_key: Pubkey,
    user_obligation_key: Pubkey,
    user_token_account_key: Pubkey,
    amount: u64,
    recent_blockhash: Hash,
) -> Transaction {
    let user_authority_key = &user_authority.pubkey();

    Transaction::new_signed_with_payer(&[
        deposit_collateral(
            market_reserve_key,
            manager_token_account_key,
            user_obligation_key,
            *user_authority_key,
            user_token_account_key,
            amount,
        ),
    ],
    Some(user_authority_key),
        &[&user_authority],
        recent_blockhash,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn do_borrow_liquidity(
    user_authority: Keypair,
    manager_key: Pubkey,
    market_reserve_key: Pubkey,
    manager_token_account_key: Pubkey,
    liquidity_price_oracle_key: Pubkey,
    rate_oracle_key: Pubkey,
    price_oracle_keys: Vec<Pubkey>,
    user_obligatiton_key: Pubkey,
    user_token_account_key: Pubkey,
    amount: u64,
    recent_blockhash: Hash,
) -> Transaction {
    let user_authority_key = &user_authority.pubkey();

    Transaction::new_signed_with_payer(&[
        update_user_obligation(
            market_reserve_key,
            liquidity_price_oracle_key,
            rate_oracle_key,
            user_obligatiton_key,
            price_oracle_keys,
        ),
        borrow_liquidity(
            manager_key,
            market_reserve_key,
            manager_token_account_key,
            user_obligatiton_key,
            *user_authority_key,
            user_token_account_key,
            amount,
        ),
    ],
    Some(user_authority_key),
        &[&user_authority],
        recent_blockhash,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn do_repay_loan(
    user_authority: Keypair,
    market_reserve_key: Pubkey,
    manager_token_account_key: Pubkey,
    rate_oracle_key: Pubkey,
    user_obligatiton_key: Pubkey,
    user_token_account_key: Pubkey,
    amount: u64,
    recent_blockhash: Hash,
) -> Transaction {
    let user_authority_key = &user_authority.pubkey();

    Transaction::new_signed_with_payer(&[
        repay_loan(
            market_reserve_key,
            manager_token_account_key,
            rate_oracle_key,
            user_obligatiton_key,
            *user_authority_key,
            user_token_account_key,
            amount,
        ),
    ],
    Some(user_authority_key),
        &[&user_authority],
        recent_blockhash,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn do_redeem_collateral(
    user_authority: Keypair,
    manager_key: Pubkey,
    liquidity_market_reserve_key: Pubkey,
    manager_token_account_key: Pubkey,
    liquidity_price_oracle_key: Pubkey,
    rate_oracle_key: Pubkey,
    collateral_market_reserve_key: Pubkey,
    collateral_price_oracle_key: Pubkey,
    price_oracle_keys: Vec<Pubkey>,
    user_obligatiton_key: Pubkey,
    user_token_account_key: Pubkey,
    amount: u64,
    recent_blockhash: Hash,
) -> Transaction {
    let user_authority_key = &user_authority.pubkey();

    Transaction::new_signed_with_payer(&[
        update_user_obligation(
            liquidity_market_reserve_key,
            liquidity_price_oracle_key,
            rate_oracle_key,
            user_obligatiton_key,
            price_oracle_keys,
        ),
        redeem_collateral(
            manager_key,
            liquidity_market_reserve_key,
            collateral_market_reserve_key,
            collateral_price_oracle_key,
            manager_token_account_key,
            user_obligatiton_key,
            *user_authority_key,
            user_token_account_key,
            amount,
        ),
    ],
    Some(user_authority_key),
        &[&user_authority],
        recent_blockhash,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn do_redeem_collateral_without_loan(
    user_authority: Keypair,
    manager_key: Pubkey,
    liquidity_market_reserve_key: Pubkey,
    manager_token_account_key: Pubkey,
    collateral_market_reserve_key: Pubkey,
    user_obligatiton_key: Pubkey,
    user_token_account_key: Pubkey,
    amount: u64,
    recent_blockhash: Hash,
) -> Transaction {
    let user_authority_key = &user_authority.pubkey();

    Transaction::new_signed_with_payer(&[
        redeem_collateral_without_loan(
            manager_key,
            liquidity_market_reserve_key,
            collateral_market_reserve_key,
            manager_token_account_key,
            user_obligatiton_key,
            *user_authority_key,
            user_token_account_key,
            amount,
        ),
    ],
    Some(user_authority_key),
        &[&user_authority],
        recent_blockhash,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn do_liquidate(
    liquidator_authority: Keypair,
    manager_key: Pubkey,
    liquidity_market_reserve_key: Pubkey,
    manager_liquidity_token_account_key: Pubkey,
    liquidity_price_oracle_key: Pubkey,
    rate_oracle_key: Pubkey,
    collateral_market_reserve_key: Pubkey,
    manager_collateral_token_account_key: Pubkey,
    price_oracle_keys: Vec<Pubkey>,
    collateral_oracle_index: usize,
    user_obligatiton_key: Pubkey,
    liquidator_liquidity_account_key: Pubkey,
    liquidator_collateral_account_key: Pubkey,
    is_arbitrary: bool,
    amount: u64,
    recent_blockhash: Hash,
) -> Transaction {
    let collateral_price_oracle_key = price_oracle_keys[collateral_oracle_index].clone();
    let liquidator_authority_key = &liquidator_authority.pubkey();

    Transaction::new_signed_with_payer(&[
        update_user_obligation(
            liquidity_market_reserve_key,
            liquidity_price_oracle_key,
            rate_oracle_key,
            user_obligatiton_key,
            price_oracle_keys,
        ),
        liquidate(
            manager_key,
            liquidity_market_reserve_key,
            manager_liquidity_token_account_key,
            collateral_market_reserve_key,
            collateral_price_oracle_key,
            manager_collateral_token_account_key,
            user_obligatiton_key,
            *liquidator_authority_key,
            liquidator_liquidity_account_key,
            liquidator_collateral_account_key,
            is_arbitrary,
            amount,
        ),
    ],
    Some(liquidator_authority_key),
        &[&liquidator_authority],
        recent_blockhash,
    )
}

pub fn do_feed_rate_oracle(
    authority: &Keypair,
    rate_oracle_key: &Pubkey,
    interest_rate: u64,
    borrow_rate: u64,
    recent_blockhash: Hash,
) -> Transaction {
    let authority_key = &authority.pubkey();

    Transaction::new_signed_with_payer(&[
        feed_rate_oracle(
            *rate_oracle_key,
            *authority_key,
            interest_rate,
            borrow_rate,
        ),
    ],
    Some(authority_key),
        &[authority],
        recent_blockhash,
    )
}