#![allow(missing_docs)]
pub mod types;
pub mod error;

use error::SodaError;

use std::str::FromStr;
use std::collections::HashMap;
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    hash::Hash,
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
    instruction::{
        bind_friend, borrow_liquidity, deposit, withdraw, pledge_collateral,
        init_manager, init_market_reserve, init_rate_oracle, init_user_obligation,
        liquidate, pause_rate_oracle, redeem_collateral, redeem_collateral_without_loan,
        repay_loan, replace_collateral, unbind_friend, update_market_reserves,
        update_user_obligation, withdraw_fee, inject_no_borrow, inject_liquidation
    },
    math::WAD, pyth::{self, Product},
    state::{CollateralConfig, LiquidityConfig, Manager,
        MarketReserve, RateOracle, RateOracleConfig, UserObligation
    }
};

// mutual
pub const DEV_NET: &str = "http://65.21.40.30";
pub const PYTH_ID: &str = "gSbePebfvPy7tRqimPoVecS2UsBvYv46ynrzWocc92s";
pub const QUOTE_CURRENCY: &[u8; 32] = &[85, 83, 68, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
pub const GLOBAL_OWNER: &str = "vG2VqMokQyY82xKda116qAmvMQm4ymoKEV92UtxNVmu4tKDt4X33ELY4rdCfiR1NxJnbek39m5X9rLJnxASNbmQ";
pub const MANAGER: &str = "F93DUk6QDpLBRd6pVQNtXvgrU4mBNMv5d1JaYkHvhcr5";
pub const OBLIGATION: &str = "5DJ3bZDmzv1NruAiEeJ3BgWcw568eKBqdHPkP68vGgXH";
pub const RATE_ORACLE: &str = "7nHzMWXrse8Mcp3Qc5KSJwG5J16wA75DMNEz7jV6hFpf";

// BNB
pub const BNB_MINT: &str = "6mhUyoQR5CcHN4RJ5PSfcvTjRuWF742ypZeMwptPgFnK";
pub const SOBNB_MINT: &str = "24Y4Wd6tYBiCgVU4naLAVA8wFSDb9v1hVwdK93x5g7Vw";
pub const BNB_PRICE: &str = "GwzBgrXb4PG59zjce24SF2b9JXbLEjJJTBkmytuEZj1b";
pub const BNB_MANAGER_TOKEN_ACCOUNT: &str = "DgHRo44dn63LJhTC5hheGhM9dERDQrhAn7V7smz5wWrC";
pub const BNB_RESERVE: &str = "3raP2UGyEqgQRpza4m6zwqpy6wumioq3fzaqdy1YYcvm";
pub const BNB_LONE_TOKEN_ACCOUNT: &str = "EnpPrZtpsKb2CK6Jyue6tYi4vPmztLXPDKdco3WnRYuS";
pub const SOBNB_LONE_TOKEN_ACCOUNT: &str = "3ikRkx3WZkY2puEo89tk2QpN6FSbcrENN4y5bQk2GmwL";

// BTC
pub const BTC_MINT: &str = "9bRWBCW4BHHoLXFLFcLU3FQCDXXLNds1SJBmpeKYFeBZ";
pub const SOBTC_MINT: &str = "4YPUDRM9LbxemxoAoZDECoigeRuWa1csxAdGHWoztDT9";
pub const BTC_PRICE: &str = "HovQMDrbAgAYPCmHVSrezcSmkMtXSSUsLDFANExrZh2J";
pub const BTC_MANAGER_TOKEN_ACCOUNT: &str = "3D3KLLYbnSY9ZxxXRRpLpFwzWTaCahj5YNYFso7FRExu";
pub const BTC_RESERVE: &str = "73wnWaSncUBgmEp5RFS1ZBLG7Y3SFv45Etnv92UN2WeQ";
pub const BTC_LONE_TOKEN_ACCOUNT: &str = "GjGcDEVXWTZznUGPnzrBfyVYEJaaDEVz8eraBR7pJEEN";
pub const SOBTC_LONE_TOKEN_ACCOUNT: &str = "4VnYHEeDi4UpHnKZFjLcZJKXTLzevotkciJY7Vb1JuZz";

// SOL
pub const SOL_MINT: &str = "2S2BU735fcn9ZSNg1BWvLx8QW4dznH9xS5DQAkcVTvfo";
pub const SOSOL_MINT: &str = "9bZ5VRbyvXZBY2GAbShh6XpgRBW3s4ZWnRKDuJMi2ZTh";
pub const SOL_PRODUCT: &str = "3Mnn2fX6rQyUsyELYms1sBJyChWofzSNRoqYzvgMVz5E";
pub const SOL_PRICE: &str = "J83w4HKfqxwcq3BEMMkPFSppX3gqekLyLJBexebFVkix";
pub const SOL_MANAGER_TOKEN_ACCOUNT: &str = "9zQhZL8G2s8Dzm7XYPLfmQYeJWdgCzTaMCdYhr5GhVKQ";
pub const SOL_RESERVE: &str = "5pEjFswbWGonMzAy2qfrZ4JYGHmrHWkC51hRodeeVERj";
pub const SOL_LONE_TOKEN_ACCOUNT: &str = "HBUXQFJFxd5eG87F7D9Pm3WjSWtsaqEDwi5HURogZThy";
pub const SOSOL_LONE_TOKEN_ACCOUNT: &str = "GosjQk7Y9TmbmAMAm5DUkdyPQ7Xadm3ZJgm6Yfpgkgwa";

// SRM
pub const SRM_MINT: &str = "DHE21jjMoGcdT8gEqjRopRrxxBbEJQpG6v9fK8Vg4xhp";
pub const SOSRM_MINT: &str = "C97kvEV17bjTvSLFghgHyMAffCKhk6mAsj3rQWst3YJR";
pub const SRM_PRODUCT: &str = "6MEwdxe4g1NeAF9u6KDG14anJpFsVEa2cvr5H6iriFZ8";
pub const SRM_PRICE: &str = "992moaMQKs32GKZ9dxi8keyM2bUmbrwBZpK4p2K6X5Vs";
pub const SRM_MANAGER_TOKEN_ACCOUNT: &str = "8GTK5gzyUJuScEzRCAu5bozfw1ahAFB87Y3pnaGhGaGQ";
pub const SRM_RESERVE: &str = "3C7MQVmySk5qEA1ANrUPKSSLcuwZyTNUr1bjwFT6J7ae";
pub const SRM_LONE_TOKEN_ACCOUNT: &str = "AFpruZrhxXVpBwojKuP5qjTAwJCH6R22QQNrba6nG2wN";
pub const SOSRM_LONE_TOKEN_ACCOUNT: &str = "6LB5hoYxgREFaY5g51tS3UyifjZTcBqG2tqSeYTHzmET";

// DOGE
pub const DOGE_MINT: &str = "2j89teL9PzbMiHXFwwEFTHa5JE682AhxcjECPivTH8od";
pub const SODOGE_MINT: &str = "BYGBQETZW7KVwH1U89H5GibRPLfvbgFAQraYt2fWW1Ac";
pub const DOGE_PRODUCT: &str = "4zvUzWGBxZA9nTgBZWAf1oGYw6nCEYRscdt14umTNWhM";
pub const DOGE_PRICE: &str = "4L6YhY8VvUgmqG5MvJkUJATtzB2rFqdrJwQCmFLv4Jzy";
pub const DOGE_MANAGER_TOKEN_ACCOUNT: &str = "HaJtbC1ymm4FsrodTLWUNBXXC7RfSquSeh58zGVHmXZ8";
pub const DOGE_RESERVE: &str = "43UVEk9mCxBHHtNXCMviK3G9YDX3zwcRHXFACJp5QNjr";
pub const DOGE_LONE_TOKEN_ACCOUNT: &str = "FME19BXhejvDnwMhwxEkkjpYeTc8J48o1wtzbu29qkMf";
pub const SODOGE_LONE_TOKEN_ACCOUNT: &str = "GpGFXZWpvtMVssmusQwQdDLZjJj38DuPiBs8udDrMcGD";

// LUNA
pub const LUNA_MINT: &str = "W8Upru1icsmcrpDtjpmt17xxUW9zBLcVHjwLkWtrZwK";
pub const SOLUNA_MINT: &str = "AwQDcYjyRf4tNJvAKfEFoEZH7WoEwAYCwcxW81iNpH9G";
pub const LUNA_PRODUCT: &str = "25tCF4ChvZyNP67xwLuYoAKuoAcSV13xrmP9YTwSPnZY";
pub const LUNA_PRICE: &str = "8PugCXTAHLM9kfLSQWe2njE5pzAgUdpPk3Nx5zSm7BD3";
pub const LUNA_MANAGER_TOKEN_ACCOUNT: &str = "D91r9qeLLaaDdqUMea72WzE9UXzcD8NgVZe1eGLxD2p3";
pub const LUNA_RESERVE: &str = "CNeHPGmEY58MsiYEBb6AhUj5vFHnsrp2zyaqj1dEeQsZ";
pub const LUNA_LONE_TOKEN_ACCOUNT: &str = "8gBhewFMmydvurfNV6Fbxrynwr6hfqUAKKC4ytHwrJEE";
pub const SOLUNA_LONE_TOKEN_ACCOUNT: &str = "BtiNij9vNX9A691egXM1E5M3UftX6iyBDtU6BiU83mm3";

// USDC
pub const USDC_MINT: &str = "Bj9LaiV7aR1z2263r5fuPjZN1asu3QXHUnGkHUAcZ4e1";
pub const SOUSDC_MINT: &str = "GQLpsMxhEmdX6kSPioCenW4C5aJiAYqZTnZixDHtePtw";
pub const USDC_PRODUCT: &str = "6NpdXrQEpmDZ3jZKmM2rhdmkd3H6QAk23j2x8bkXcHKA";
pub const USDC_PRICE: &str = "5SSkXsEKQepHHAewytPVwdej4epN1nxgLVM84L4KXgy7";
pub const USDC_MANAGER_TOKEN_ACCOUNT: &str = "68rh6pWuipFPZCna56uHFLDHWTpGRZQ2r4e1uHjRcGFp";
pub const USDC_RESERVE: &str = "bSrhY4SaKYSf4BWjQWgWa7oJ2NTMJtU5vCFwEuDfvwZ";
pub const USDC_LONE_TOKEN_ACCOUNT: &str = "4axY5PF6qUEC1RZ8V5TJ7Dhq6rMMgV2iCPN6yNRUo6QR";
pub const SOUSDC_LONE_TOKEN_ACCOUNT: &str = "pPed3aVtFQ41ifHZJJLgz667nskzLf2rmfSLjePa9fT";

#[allow(clippy::too_many_arguments)]
pub fn create_test_token(
    mint: Keypair,
    authority: Keypair,
    account: Keypair,
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

    println!("mint: {:?}", mint_pubkey);
    println!("lone account: {:?}", account_pubkey);

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
        &[&mint, &account, &authority],
        recent_blockhash,
    ))
}

pub fn do_init_token_account(
    authority: Keypair,
    account: Keypair,
    mint_key: Pubkey,
    lamports: u64,
    recent_blockhash: Hash,
) -> Result<Transaction, ProgramError> {
    let authority_key = &authority.pubkey();
    let account_key = &account.pubkey();
    let program_id = spl_token::id();

    println!("account is {:?}", account_key);

    Ok(Transaction::new_signed_with_payer(&[
            create_account(
                authority_key,
                &account_key,
                lamports,
                Account::LEN as u64,
                &program_id,
            ),
            initialize_account(
                &program_id,
                &account_key,
                &mint_key,
                authority_key,
            )?,
        ],
        Some(authority_key),
        &[&account, &authority],
        recent_blockhash,
    ))
}

pub fn create_manager(
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

pub fn create_rate_oracle(
    rate_oracle: Keypair,
    authority: Keypair,
    config: RateOracleConfig,
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
            config,
        )
    ],
    Some(authority_key),
        &[&rate_oracle, &authority],
        recent_blockhash,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn create_market_reserve(
    authority: Keypair,
    manager_key: Pubkey,
    pyth_product_key: Pubkey,
    pyth_price_key: Pubkey,
    rate_oracle_key: Pubkey,
    token_mint_key: Pubkey,
    collateral_config: CollateralConfig,
    liquidity_config: LiquidityConfig,
    enable_borrow: bool,
    reserve_lamports: u64,
    account_lamports: u64,
    mint_lamports: u64,
    recent_blockhash: Hash,
) -> Result<Transaction, ProgramError> {
    let market_reserve = Keypair::new();
    let manager_token_account = Keypair::new();
    let sotoken_mint = Keypair::new();
    let market_reserve_key = market_reserve.pubkey();
    let manager_token_account_key = manager_token_account.pubkey();
    let sotoken_mint_key = sotoken_mint.pubkey();

    println!("market reserve key: {:?}", market_reserve_key);
    println!("manager token account key: {:?}", manager_token_account_key);
    println!("sotoken mint key: {:?}", sotoken_mint_key);

    let token_program_id = &spl_token::id();
    let authority_key = &authority.pubkey();

    Ok(Transaction::new_signed_with_payer(&[
        create_account(
            authority_key,
            &market_reserve_key,
            reserve_lamports,
            MarketReserve::LEN as u64,
            &soda_lending_contract::id(),
        ),
        create_account(
            authority_key,
            &manager_token_account_key,
            account_lamports,
            Account::LEN as u64,
            token_program_id,
        ),
        create_account(
            authority_key,
            &sotoken_mint_key,
            mint_lamports,
            Mint::LEN as u64,
            token_program_id,
        ),
        init_market_reserve(
            manager_key,
            manager_token_account_key,
            market_reserve_key,
            pyth_product_key,
            pyth_price_key,
            rate_oracle_key,
            token_mint_key,
            sotoken_mint_key,
            *authority_key,
            collateral_config,
            liquidity_config,
            enable_borrow,
        )
    ],
    Some(authority_key),
        &[
            &market_reserve,
            &manager_token_account,
            &sotoken_mint,
            &authority,
        ],
        recent_blockhash,
    ))
}

pub fn do_deposit(
    authority: Keypair,
    manager_key: Pubkey,
    market_reserve_key: Pubkey,
    sotoken_mint_key: Pubkey,
    manager_token_account_key: Pubkey,
    rate_oracle_key: Pubkey,
    user_token_account_key: Pubkey,
    user_sotoken_account_key: Pubkey,
    amount: u64,
    recent_blockhash: Hash,
) -> Transaction {
    let authority_key = &authority.pubkey();

    Transaction::new_signed_with_payer(&[
        deposit(
            manager_key,
            market_reserve_key,
            sotoken_mint_key,
            manager_token_account_key,
            rate_oracle_key,
            *authority_key,
            user_token_account_key,
            user_sotoken_account_key,
            amount,
        ),
    ],
    Some(authority_key),
        &[&authority],
        recent_blockhash,
    )
}

pub fn do_withdraw(
    authority: Keypair,
    manager_key: Pubkey,
    market_reserve_key: Pubkey,
    sotoken_mint_key: Pubkey,
    manager_token_account_key: Pubkey,
    rate_oracle_key: Pubkey,
    user_token_account_key: Pubkey,
    user_sotoken_account_key: Pubkey,
    amount: u64,
    recent_blockhash: Hash,
) -> Transaction {
    let authority_key = &authority.pubkey();

    Transaction::new_signed_with_payer(&[
        withdraw(
            manager_key,
            market_reserve_key,
            sotoken_mint_key,
            manager_token_account_key,
            rate_oracle_key,
            *authority_key,
            user_token_account_key,
            user_sotoken_account_key,
            amount,
        ),
    ],
    Some(authority_key),
        &[&authority],
        recent_blockhash,
    )
}

pub fn create_user_obligation(
    authority: Keypair,
    manager_key: Pubkey,
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
            manager_key,
            *obligation_key,
            *authority_pubkey,
        ),
    ],
    Some(authority_pubkey),
        &[&obligation, &authority],
        recent_blockhash,
    )
}

pub fn do_bind_friend(
    user_authority: Keypair,
    friend_authority: Keypair,
    user_obligation_key: Pubkey,
    friend_obligation_key: Pubkey,
    recent_blockhash: Hash,
) -> Transaction {
    let user_authority_key = &user_authority.pubkey();
    let friend_authority_key = &friend_authority.pubkey();

    Transaction::new_signed_with_payer(&[
        bind_friend(
            user_obligation_key,
            friend_obligation_key,
            *user_authority_key,
            *friend_authority_key,
        ),
    ],
    Some(user_authority_key),
        &[&user_authority, &friend_authority],
        recent_blockhash,
    )
}

pub fn do_unbind_friend(
    user_authority: Keypair,
    friend_authority: Keypair,
    user_updating_keys: Vec<(Pubkey, Pubkey, Pubkey)>,
    friend_updating_keys: Vec<(Pubkey, Pubkey, Pubkey)>,
    user_obligation_key: Pubkey,
    friend_obligation_key: Pubkey,
    recent_blockhash: Hash,
) -> Transaction {
    let user_authority_key = &user_authority.pubkey();
    let friend_authority_key = &friend_authority.pubkey();

    let user_market_reserves = user_updating_keys
        .iter()
        .map(|reserve| reserve.0)
        .collect::<Vec<_>>();

    let friend_market_reserves = friend_updating_keys
        .iter()
        .map(|reserve| reserve.0)
        .collect::<Vec<_>>();

    Transaction::new_signed_with_payer(&[
        update_market_reserves(user_updating_keys),
        update_user_obligation(user_obligation_key, user_market_reserves),
        update_market_reserves(friend_updating_keys),
        update_user_obligation(friend_obligation_key, friend_market_reserves),
        unbind_friend(
            user_obligation_key,
            friend_obligation_key,
            *user_authority_key,
            *friend_authority_key,
        ),
    ],
    Some(user_authority_key),
        &[&user_authority, &friend_authority],
        recent_blockhash,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn do_pledge_collateral(
    user_authority: Keypair,
    market_reserve_key: Pubkey,
    sotoken_mint_key: Pubkey,
    user_obligatiton_key: Pubkey,
    user_sotoken_account_key: Pubkey,
    amount: u64,
    recent_blockhash: Hash,
) -> Transaction {
    let user_authority_key = &user_authority.pubkey();

    Transaction::new_signed_with_payer(&[
        pledge_collateral(
            market_reserve_key,
            sotoken_mint_key,
            user_obligatiton_key,
            *user_authority_key,
            user_sotoken_account_key,
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
    updating_keys: Vec<(Pubkey, Pubkey, Pubkey)>,
    redeem_index: usize,
    manager_key: Pubkey,
    sotoken_mint_key: Pubkey,
    user_obligation_key: Pubkey,
    user_sotoken_account_key: Pubkey,
    amount: u64,
    recent_blockhash: Hash,
) -> Result<Transaction, ProgramError> {
    let user_authority_key = &user_authority.pubkey();    
    let market_reserves = updating_keys
        .iter()
        .map(|reserve| reserve.0)
        .collect::<Vec<_>>();

    let market_reserve_key = market_reserves
        .get(redeem_index)
        .ok_or(ProgramError::NotEnoughAccountKeys)?
        .clone();

    let transaction = Transaction::new_signed_with_payer(&[
        update_market_reserves(updating_keys),
        update_user_obligation(user_obligation_key, market_reserves),
        redeem_collateral(
            manager_key,
            market_reserve_key,
            sotoken_mint_key,
            user_obligation_key,
            None,
            *user_authority_key,
            user_sotoken_account_key,
            amount,
        ),
    ],
    Some(user_authority_key),
        &[&user_authority],
        recent_blockhash,
    );

    Ok(transaction)
}

#[allow(clippy::too_many_arguments)]
pub fn do_redeem_collateral_with_friend(
    user_authority: Keypair,
    user_updating_keys: Vec<(Pubkey, Pubkey, Pubkey)>,
    friend_updating_keys: Vec<(Pubkey, Pubkey, Pubkey)>,
    redeem_index: usize,
    manager_key: Pubkey,
    sotoken_mint_key: Pubkey,
    user_obligation_key: Pubkey,
    friend_obligation_key: Pubkey,
    user_sotoken_account_key: Pubkey,
    amount: u64,
    recent_blockhash: Hash,
) -> Result<Transaction, ProgramError> {
    let user_authority_key = &user_authority.pubkey();    
    let user_market_reserves = user_updating_keys
        .iter()
        .map(|reserve| reserve.0)
        .collect::<Vec<_>>();

    let friend_market_reserves = friend_updating_keys
        .iter()
        .map(|reserve| reserve.0)
        .collect::<Vec<_>>();

    let market_reserve_key = user_market_reserves
        .get(redeem_index)
        .ok_or(ProgramError::NotEnoughAccountKeys)?
        .clone();

    let transaction = Transaction::new_signed_with_payer(&[
        update_market_reserves(user_updating_keys),
        update_user_obligation(user_obligation_key, user_market_reserves),
        update_market_reserves(friend_updating_keys),
        update_user_obligation(friend_obligation_key, friend_market_reserves),
        redeem_collateral(
            manager_key,
            market_reserve_key,
            sotoken_mint_key,
            user_obligation_key,
            Some(friend_obligation_key),
            *user_authority_key,
            user_sotoken_account_key,
            amount,
        ),
    ],
    Some(user_authority_key),
        &[&user_authority],
        recent_blockhash,
    );

    Ok(transaction)
}

#[allow(clippy::too_many_arguments)]
pub fn do_redeem_collateral_without_loan(
    user_authority: Keypair,
    manager_key: Pubkey,
    market_reserve_key: Pubkey,
    sotoken_mint_key: Pubkey,
    user_obligation_key: Pubkey,
    friend_obligation_key: Option<Pubkey>,
    user_sotoken_account_key: Pubkey,
    amount: u64,
    recent_blockhash: Hash,
) -> Transaction {
    let user_authority_key = &user_authority.pubkey();

    Transaction::new_signed_with_payer(&[
        redeem_collateral_without_loan(
            manager_key,
            market_reserve_key,
            sotoken_mint_key,
            user_obligation_key,
            friend_obligation_key,
            *user_authority_key,
            user_sotoken_account_key,
            amount,
        ),
    ],
    Some(user_authority_key),
        &[&user_authority],
        recent_blockhash,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn do_replace_collateral(
    user_authority: Keypair,
    updating_keys: Vec<(Pubkey, Pubkey, Pubkey)>,
    replace_out_index: usize,
    replace_in_index: usize,
    manager_key: Pubkey,
    out_sotoken_mint_key: Pubkey,
    in_sotoken_mint_key: Pubkey,
    user_obligation_key: Pubkey,
    user_out_sotoken_account_key: Pubkey,
    user_in_sotoken_account_key: Pubkey,
    amount: u64,
    recent_blockhash: Hash,
) -> Result<Transaction, ProgramError> {
    let user_authority_key = &user_authority.pubkey();    
    let market_reserves = updating_keys
        .iter()
        .map(|reserve| reserve.0)
        .collect::<Vec<_>>();

    let out_market_reserve_key = market_reserves
        .get(replace_out_index)
        .ok_or(ProgramError::NotEnoughAccountKeys)?
        .clone();

    let in_market_reserve_key = market_reserves
        .get(replace_in_index)
        .ok_or(ProgramError::NotEnoughAccountKeys)?
        .clone();

    let transaction = Transaction::new_signed_with_payer(&[
        update_market_reserves(updating_keys),
        update_user_obligation(user_obligation_key, market_reserves),
        replace_collateral(
            manager_key,
            out_market_reserve_key,
            out_sotoken_mint_key,
            in_market_reserve_key,
            in_sotoken_mint_key,
            user_obligation_key,
            None,
            *user_authority_key,
            user_out_sotoken_account_key,
            user_in_sotoken_account_key,
            amount
        ),
    ],
    Some(user_authority_key),
        &[&user_authority],
        recent_blockhash,
    );

    Ok(transaction)
}

#[allow(clippy::too_many_arguments)]
pub fn do_replace_collateral_with_friend(
    user_authority: Keypair,
    user_updating_keys: Vec<(Pubkey, Pubkey, Pubkey)>,
    friend_updating_keys: Vec<(Pubkey, Pubkey, Pubkey)>,
    replace_out_index: usize,
    replace_in_index: usize,
    manager_key: Pubkey,
    out_sotoken_mint_key: Pubkey,
    in_sotoken_mint_key: Pubkey,
    user_obligation_key: Pubkey,
    friend_obligation_key: Pubkey,
    user_out_sotoken_account_key: Pubkey,
    user_in_sotoken_account_key: Pubkey,
    amount: u64,
    recent_blockhash: Hash,
) -> Result<Transaction, ProgramError> {
    let user_authority_key = &user_authority.pubkey();    
    let user_market_reserves = user_updating_keys
        .iter()
        .map(|reserve| reserve.0)
        .collect::<Vec<_>>();

    let friend_market_reserves = friend_updating_keys
        .iter()
        .map(|reserve| reserve.0)
        .collect::<Vec<_>>();

    let out_market_reserve_key = user_market_reserves
        .get(replace_out_index)
        .ok_or(ProgramError::NotEnoughAccountKeys)?
        .clone();

    let in_market_reserve_key = user_market_reserves
        .get(replace_in_index)
        .ok_or(ProgramError::NotEnoughAccountKeys)?
        .clone();

    let transaction = Transaction::new_signed_with_payer(&[
        update_market_reserves(user_updating_keys),
        update_user_obligation(user_obligation_key, user_market_reserves),
        update_market_reserves(friend_updating_keys),
        update_user_obligation(friend_obligation_key, friend_market_reserves),
        replace_collateral(
            manager_key,
            out_market_reserve_key,
            out_sotoken_mint_key,
            in_market_reserve_key,
            in_sotoken_mint_key,
            user_obligation_key,
            Some(friend_obligation_key),
            *user_authority_key,
            user_out_sotoken_account_key,
            user_in_sotoken_account_key,
            amount
        ),
    ],
    Some(user_authority_key),
        &[&user_authority],
        recent_blockhash,
    );

    Ok(transaction)
}

#[allow(clippy::too_many_arguments)]
pub fn do_borrow_liquidity(
    user_authority: Keypair,
    updating_keys: Vec<(Pubkey, Pubkey, Pubkey)>,
    borrow_index: usize,
    manager_key: Pubkey,
    manager_token_account_key: Pubkey,
    user_obligation_key: Pubkey,
    user_token_account_key: Pubkey,
    amount: u64,
    recent_blockhash: Hash,
) -> Result<Transaction, ProgramError> {
    let user_authority_key = &user_authority.pubkey();    
    let market_reserves = updating_keys
        .iter()
        .map(|reserve| reserve.0)
        .collect::<Vec<_>>();

    let market_reserve_key = market_reserves
        .get(borrow_index)
        .ok_or(ProgramError::NotEnoughAccountKeys)?
        .clone();

    let transaction = Transaction::new_signed_with_payer(&[
        update_market_reserves(updating_keys),
        update_user_obligation(user_obligation_key, market_reserves),
        borrow_liquidity(
            manager_key,
            market_reserve_key,
            manager_token_account_key,
            user_obligation_key,
            None,
            *user_authority_key,
            user_token_account_key,
            amount,
        ),
    ],
    Some(user_authority_key),
        &[&user_authority],
        recent_blockhash,
    );

    Ok(transaction)
}

#[allow(clippy::too_many_arguments)]
pub fn do_borrow_liquidity_with_friend(
    user_authority: Keypair,
    user_updating_keys: Vec<(Pubkey, Pubkey, Pubkey)>,
    friend_updating_keys: Vec<(Pubkey, Pubkey, Pubkey)>,
    borrow_index: usize,
    manager_key: Pubkey,
    manager_token_account_key: Pubkey,
    user_obligation_key: Pubkey,
    friend_obligation_key: Pubkey,
    user_token_account_key: Pubkey,
    amount: u64,
    recent_blockhash: Hash,
) -> Result<Transaction, ProgramError> {
    let user_authority_key = &user_authority.pubkey();    
    let user_market_reserves = user_updating_keys
        .iter()
        .map(|reserve| reserve.0)
        .collect::<Vec<_>>();

    let friend_market_reserves = friend_updating_keys
        .iter()
        .map(|reserve| reserve.0)
        .collect::<Vec<_>>();

    let market_reserve_key = user_market_reserves
        .get(borrow_index)
        .ok_or(ProgramError::NotEnoughAccountKeys)?
        .clone();

    let transaction = Transaction::new_signed_with_payer(&[
        update_market_reserves(user_updating_keys),
        update_user_obligation(user_obligation_key, user_market_reserves),
        update_market_reserves(friend_updating_keys),
        update_user_obligation(friend_obligation_key, friend_market_reserves),
        borrow_liquidity(
            manager_key,
            market_reserve_key,
            manager_token_account_key,
            user_obligation_key,
            Some(friend_obligation_key),
            *user_authority_key,
            user_token_account_key,
            amount,
        ),
    ],
    Some(user_authority_key),
        &[&user_authority],
        recent_blockhash,
    );

    Ok(transaction)
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
pub fn do_liquidate(
    liquidator_authority: Keypair,
    updating_keys: Vec<(Pubkey, Pubkey, Pubkey)>,
    collateral_index: usize,
    loan_index: usize,
    manager_key: Pubkey,
    manager_token_account_key: Pubkey,
    sotoken_mint_key: Pubkey,
    user_obligation_key: Pubkey,
    liquidator_token_account_key: Pubkey,
    liquidator_sotoken_account_key: Pubkey,
    amount: u64,
    recent_blockhash: Hash,
) -> Result<Transaction, ProgramError> {
    let liquidator_authority_key = &liquidator_authority.pubkey();    
    let market_reserves = updating_keys
        .iter()
        .map(|reserve| reserve.0)
        .collect::<Vec<_>>();

    let collateral_market_reserve_key = market_reserves
        .get(collateral_index)
        .ok_or(ProgramError::NotEnoughAccountKeys)?
        .clone();

    let loan_market_reserve_key = market_reserves
        .get(loan_index)
        .ok_or(ProgramError::NotEnoughAccountKeys)?
        .clone();

    let transaction = Transaction::new_signed_with_payer(&[
        update_market_reserves(updating_keys),
        update_user_obligation(user_obligation_key, market_reserves),
        liquidate(
            manager_key,
            collateral_market_reserve_key,
            sotoken_mint_key,
            loan_market_reserve_key,
            manager_token_account_key,
            user_obligation_key,
            None,
            *liquidator_authority_key,
            liquidator_token_account_key,
            liquidator_sotoken_account_key,
            amount,
        ),
    ],
    Some(liquidator_authority_key),
        &[&liquidator_authority],
        recent_blockhash,
    );

    Ok(transaction)
}

#[allow(clippy::too_many_arguments)]
pub fn do_liquidate_with_friend(
    liquidator_authority: Keypair,
    user_updating_keys: Vec<(Pubkey, Pubkey, Pubkey)>,
    friend_updating_keys: Vec<(Pubkey, Pubkey, Pubkey)>,
    collateral_index: usize,
    loan_index: usize,
    manager_key: Pubkey,
    manager_token_account_key: Pubkey,
    sotoken_mint_key: Pubkey,
    user_obligation_key: Pubkey,
    friend_obligation_key: Pubkey,
    liquidator_token_account_key: Pubkey,
    liquidator_sotoken_account_key: Pubkey,
    amount: u64,
    recent_blockhash: Hash,
) -> Result<Transaction, ProgramError> {
    let liquidator_authority_key = &liquidator_authority.pubkey();    
    let user_market_reserves = user_updating_keys
        .iter()
        .map(|reserve| reserve.0)
        .collect::<Vec<_>>();

    let friend_market_reserves = friend_updating_keys
        .iter()
        .map(|reserve| reserve.0)
        .collect::<Vec<_>>();

    let collateral_market_reserve_key = user_market_reserves
        .get(collateral_index)
        .ok_or(ProgramError::NotEnoughAccountKeys)?
        .clone();

    let loan_market_reserve_key = user_market_reserves
        .get(loan_index)
        .ok_or(ProgramError::NotEnoughAccountKeys)?
        .clone();

    let transaction = Transaction::new_signed_with_payer(&[
        update_market_reserves(user_updating_keys),
        update_user_obligation(user_obligation_key, user_market_reserves),
        update_market_reserves(friend_updating_keys),
        update_user_obligation(friend_obligation_key, friend_market_reserves),
        liquidate(
            manager_key,
            collateral_market_reserve_key,
            sotoken_mint_key,
            loan_market_reserve_key,
            manager_token_account_key,
            user_obligation_key,
            Some(friend_obligation_key),
            *liquidator_authority_key,
            liquidator_token_account_key,
            liquidator_sotoken_account_key,
            amount,
        ),
    ],
    Some(liquidator_authority_key),
        &[&liquidator_authority],
        recent_blockhash,
    );

    Ok(transaction)
}

pub fn do_inject_no_borrow(
    authority: Keypair,
    user_obligation_key: Pubkey,
    recent_blockhash: Hash,
) -> Transaction {
    let authority_key = &authority.pubkey();

    Transaction::new_signed_with_payer(&[
        inject_no_borrow(
            user_obligation_key,
        ),
    ],
    Some(authority_key),
        &[&authority],
        recent_blockhash,
    )
}

pub fn do_inject_liquidation(
    authority: Keypair,
    user_obligation_key: Pubkey,
    recent_blockhash: Hash,
) -> Transaction {
    let authority_key = &authority.pubkey();

    Transaction::new_signed_with_payer(&[
        inject_liquidation(
            user_obligation_key,
        ),
    ],
    Some(authority_key),
        &[&authority],
        recent_blockhash,
    )
}

pub fn do_update_market_reserves_and_obligation(
    authority: Keypair,
    updating_keys: Vec<(Pubkey, Pubkey, Pubkey)>,
    user_obligation_key: Pubkey,
    recent_blockhash: Hash,
) -> Transaction {
    let authority_key = &authority.pubkey(); 

    let market_reserves = updating_keys
        .iter()
        .map(|reserve| reserve.0)
        .collect::<Vec<_>>();

    Transaction::new_signed_with_payer(&[
            update_market_reserves(updating_keys),
            update_user_obligation(user_obligation_key, market_reserves),
        ],
        Some(authority_key),
        &[&authority],
        recent_blockhash,
    )
}

pub fn get_market_and_price_map(client: &RpcClient) -> Result<HashMap::<Pubkey, (Vec<u8>, Vec<u8>, Vec<u8>)>, SodaError> {
    let mut collaterals_price_oracle_map = HashMap::new();
    // BNB
    collaterals_price_oracle_map.insert(
        Pubkey::from_str(BNB_RESERVE).unwrap(),
        (
            client.get_account_data(&Pubkey::from_str(BNB_RESERVE).unwrap())?,
            client.get_account_data(&Pubkey::from_str(BNB_PRICE).unwrap())?,
            client.get_account_data(&Pubkey::from_str(RATE_ORACLE).unwrap())?,
        ),
    );
    // BTC
    collaterals_price_oracle_map.insert(
        Pubkey::from_str(BTC_RESERVE).unwrap(),
        (
            client.get_account_data(&Pubkey::from_str(BTC_RESERVE).unwrap())?,
            client.get_account_data(&Pubkey::from_str(BTC_PRICE).unwrap())?,
            client.get_account_data(&Pubkey::from_str(RATE_ORACLE).unwrap())?,
        ),
    );
    // SRM
    collaterals_price_oracle_map.insert(
        Pubkey::from_str(SRM_RESERVE).unwrap(),
        (
            client.get_account_data(&Pubkey::from_str(SRM_RESERVE).unwrap())?,
            client.get_account_data(&Pubkey::from_str(SRM_PRICE).unwrap())?,
            client.get_account_data(&Pubkey::from_str(RATE_ORACLE).unwrap())?,
        ),
    );
    // DOGE
    collaterals_price_oracle_map.insert(
        Pubkey::from_str(DOGE_RESERVE).unwrap(),
        (
            client.get_account_data(&Pubkey::from_str(DOGE_RESERVE).unwrap())?,
            client.get_account_data(&Pubkey::from_str(DOGE_PRICE).unwrap())?,
            client.get_account_data(&Pubkey::from_str(RATE_ORACLE).unwrap())?,
        ),
    );
    // LUNA
    collaterals_price_oracle_map.insert(
        Pubkey::from_str(LUNA_RESERVE).unwrap(),
        (
            client.get_account_data(&Pubkey::from_str(LUNA_RESERVE).unwrap())?,
            client.get_account_data(&Pubkey::from_str(LUNA_PRICE).unwrap())?,
            client.get_account_data(&Pubkey::from_str(RATE_ORACLE).unwrap())?,
        ),
    );
    // SOL
    collaterals_price_oracle_map.insert(
        Pubkey::from_str(SOL_RESERVE).unwrap(),
        (
            client.get_account_data(&Pubkey::from_str(SOL_RESERVE).unwrap())?,
            client.get_account_data(&Pubkey::from_str(SOL_PRICE).unwrap())?,
            client.get_account_data(&Pubkey::from_str(RATE_ORACLE).unwrap())?,
        ),
    );
    // USDC
    collaterals_price_oracle_map.insert(
        Pubkey::from_str(USDC_RESERVE).unwrap(),
        (
            client.get_account_data(&Pubkey::from_str(USDC_RESERVE).unwrap())?,
            client.get_account_data(&Pubkey::from_str(USDC_PRICE).unwrap())?,
            client.get_account_data(&Pubkey::from_str(RATE_ORACLE).unwrap())?,
        ),
    );

    Ok(collaterals_price_oracle_map)
}