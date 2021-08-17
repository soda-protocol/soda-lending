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
    state::{Manager, MarketReserve, RateOracle, UserAsset, UserObligation, 
        CollateralConfig, LiquidityConfig
    },
    instruction::{
        init_manager, init_rate_oracle, init_market_reserve_without_liquidity,
        init_market_reserve_with_liquidity, init_user_obligation,
        init_user_asset, deposit_liquidity, withdraw_liquidity,
        deposit_collateral, borrow_liquidity, repay_loan,
        redeem_collateral, liquidate, feed_rate_oracle, pause_rate_oracle,
        add_liquidity_to_market_reserve, withdraw_fee,
    },
    pyth::{self, Product},
};

const DEV_NET: &str = "http://65.21.40.30";
const PYTH_ID: &str = "gSbePebfvPy7tRqimPoVecS2UsBvYv46ynrzWocc92s";
const GLOBAL_OWNER: &str = "vG2VqMokQyY82xKda116qAmvMQm4ymoKEV92UtxNVmu4tKDt4X33ELY4rdCfiR1NxJnbek39m5X9rLJnxASNbmQ";
const QUOTE_CURRENCY: &[u8; 32] = &[85, 83, 68, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

fn main() {
    let client = RpcClient::new_with_commitment(DEV_NET.into(), CommitmentConfig::default());

    //// create manager
    // let lamports = client.get_minimum_balance_for_rent_exemption(Manager::LEN).unwrap();
    // let authority = &Keypair::from_base58_string(GLOBAL_OWNER);
    // let manager = &Keypair::new();
    // println!("manager key: {:?}", manager.pubkey());
    // let (block_hash, _) = client.get_recent_blockhash().unwrap();
    // let transaction = create_lending_manager(manager, authority, lamports, block_hash);
    // match client.send_and_confirm_transaction(&transaction) {
    //     Ok(sig) => println!("sig is {:?}", sig),
    //     Err(err) => println!("error: {:?}", err),
    // }

    //// create oracle
    // let lamports = client.get_minimum_balance_for_rent_exemption(RateOracle::LEN).unwrap();
    // let authority = &Keypair::from_base58_string(GLOBAL_OWNER);
    // let rate_oracle = &Keypair::new();
    // println!("rate oracle key: {:?}", rate_oracle.pubkey());
    // let (block_hash, _) = client.get_recent_blockhash().unwrap();
    // let transaction = create_rate_oracle(rate_oracle, authority, lamports, block_hash);
    // match client.send_and_confirm_transaction(&transaction) {
    //     Ok(sig) => println!("sig is {:?}", sig),
    //     Err(err) => println!("error: {:?}", err),
    // }

    // create market reserve (no liquidity)
    // let account_lamports = client.get_minimum_balance_for_rent_exemption(Account::LEN).unwrap();
    // let reserve_lamports = client.get_minimum_balance_for_rent_exemption(MarketReserve::LEN).unwrap();
    // let authority = &Keypair::from_base58_string(GLOBAL_OWNER);
    // let (block_hash, _) = client.get_recent_blockhash().unwrap();
    // let transaction = create_market_reserve_without_liquidity(
    //     authority,
    //     &Pubkey::from_str("5nBpNCqkH8aKpUkJjruykZsuSjmLKSzCYEnAb2p8TB13").unwrap(),
    //     &Pubkey::from_str("2weC6fjXrfaCLQpqEzdgBHpz6yVNvmSN133m7LDuZaDb").unwrap(),
    //     &Pubkey::from_str("GwzBgrXb4PG59zjce24SF2b9JXbLEjJJTBkmytuEZj1b").unwrap(),
    //     &Pubkey::from_str("6mhUyoQR5CcHN4RJ5PSfcvTjRuWF742ypZeMwptPgFnK").unwrap(),
    //     CollateralConfig {
    //         liquidate_fee_rate: 25_000_000_000_000_000, 
    //         arbitrary_liquidate_rate: 950_000_000_000_000_000, 
    //         liquidate_limit: 85, 
    //         effective_value_rate: 70,
    //         close_factor: 60,
    //     },
    //     reserve_lamports,
    //     account_lamports, 
    //     block_hash
    // ).unwrap();
    // match client.send_and_confirm_transaction(&transaction) {
    //     Ok(sig) => println!("sig is {:?}", sig),
    //     Err(err) => println!("error: {:?}", err),
    // }

    // create market reserve (with liquidity)
    let account_lamports = client.get_minimum_balance_for_rent_exemption(Account::LEN).unwrap();
    let reserve_lamports = client.get_minimum_balance_for_rent_exemption(MarketReserve::LEN).unwrap();
    let authority = &Keypair::from_base58_string(GLOBAL_OWNER);
    let (block_hash, _) = client.get_recent_blockhash().unwrap();
    let transaction = create_market_reserve_with_liquidity(
        authority,
        &Pubkey::from_str("5nBpNCqkH8aKpUkJjruykZsuSjmLKSzCYEnAb2p8TB13").unwrap(),
        &Pubkey::from_str("2weC6fjXrfaCLQpqEzdgBHpz6yVNvmSN133m7LDuZaDb").unwrap(),
        &Pubkey::from_str("GwzBgrXb4PG59zjce24SF2b9JXbLEjJJTBkmytuEZj1b").unwrap(),
        &Pubkey::from_str("6mhUyoQR5CcHN4RJ5PSfcvTjRuWF742ypZeMwptPgFnK").unwrap(),
        &Pubkey::from_str("")
        CollateralConfig {
            liquidate_fee_rate: 25_000_000_000_000_000, 
            arbitrary_liquidate_rate: 950_000_000_000_000_000, 
            liquidate_limit: 85, 
            effective_value_rate: 70,
            close_factor: 60,
        },
        reserve_lamports,
        account_lamports, 
        block_hash
    ).unwrap();
    match client.send_and_confirm_transaction(&transaction) {
        Ok(sig) => println!("sig is {:?}", sig),
        Err(err) => println!("error: {:?}", err),
    }


























    // let lamports1 = client.get_minimum_balance_for_rent_exemption(Mint::LEN).unwrap();
    // let lamports2 = client.get_minimum_balance_for_rent_exemption(Account::LEN).unwrap();

    // let authority = &Keypair::from_base58_string(GLOBAL_OWNER);

    // let mint = &Keypair::new();
    // let token_account = &Keypair::new();
    // println!("mint key: {:?}, token account pubkey: {:?}", mint.pubkey(), token_account.pubkey());

    // client.request_airdrop(&authority.pubkey(), 10_000_000_000).unwrap();

    // thread::sleep(Duration::from_secs(30));

    // let (block_hash, _) = client.get_recent_blockhash().unwrap();

    // let transaction = create_token(
    //     mint,
    //     authority, 
    //     token_account, 
    //     lamports1, 
    //     lamports2,
    //     1_000_000_000_000_000_000, 
    //     block_hash
    // ).unwrap();

    // match client.send_and_confirm_transaction(&transaction) {
    //     Ok(sig) => println!("sig is {:?}", sig),
    //     Err(err) => println!("error: {:?}", err),
    // }
}

fn get_pyth_quote_currency(data: &[u8]) -> Result<[u8; 32], ProgramError> {
    let pyth_product = pyth::load::<pyth::Product>(data)
        .map_err(|_| ProgramError::InvalidAccountData)?;

    const LEN: usize = 14;
    const KEY: &[u8; LEN] = b"quote_currency";

    let mut start = 0;
    while start < pyth::PROD_ATTR_SIZE {
        let mut length = pyth_product.attr[start] as usize;
        start += 1;

        if length == LEN {
            let mut end = start + length;
            if end > pyth::PROD_ATTR_SIZE {
                println!("Pyth product attribute key length too long");
                return Err(ProgramError::InvalidAccountData);
            }

            let key = &pyth_product.attr[start..end];
            if key == KEY {
                start += length;
                length = pyth_product.attr[start] as usize;
                start += 1;

                end = start + length;
                if length > 32 || end > pyth::PROD_ATTR_SIZE {
                    println!("Pyth product quote currency value too long");
                    return Err(ProgramError::InvalidAccountData);
                }

                let mut value = [0u8; 32];
                value[0..length].copy_from_slice(&pyth_product.attr[start..end]);
                return Ok(value);
            }
        }

        start += length;
        start += 1 + pyth_product.attr[start] as usize;
    }

    Err(ProgramError::InvalidAccountData)
}

#[allow(clippy::too_many_arguments)]
fn create_token(
    mint: &Keypair,
    authority: &Keypair,
    account: &Keypair,
    mint_lamports: u64,
    acnt_lamports: u64,
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
                9,
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
    manager: &Keypair,
    authority: &Keypair,
    lamports: u64,
    recent_blockhash: Hash,
) -> Transaction {
    let manager_pubkey = &manager.pubkey();
    let authority_pubkey = &authority.pubkey();
    let pyth_id = Pubkey::from_str(PYTH_ID).unwrap();

    Transaction::new_signed_with_payer(&[
        create_account(
            authority_pubkey,
            manager_pubkey,
            lamports,
            Manager::LEN as u64,
            &soda_lending_contract::id(),
        ),
        init_manager(
            manager_pubkey.clone(),
            authority_pubkey.clone(),
            pyth_id,
            *QUOTE_CURRENCY,
        )
    ],
    Some(authority_pubkey),
        &[manager, authority],
        recent_blockhash,
    )
}

fn create_rate_oracle(
    rate_oracle: &Keypair,
    authority: &Keypair,
    lamports: u64,
    recent_blockhash: Hash,
) -> Transaction {
    let authority_pubkey = &authority.pubkey();
    let rate_oracle_pubkey = &rate_oracle.pubkey();

    Transaction::new_signed_with_payer(&[
        create_account(
            authority_pubkey,
            rate_oracle_pubkey,
            lamports,
            RateOracle::LEN as u64,
            &soda_lending_contract::id(),
        ),
        init_rate_oracle(
            rate_oracle_pubkey.clone(),
            authority_pubkey.clone(),
        )
    ],
    Some(authority_pubkey),
        &[rate_oracle, authority],
        recent_blockhash,
    )
}

#[allow(clippy::too_many_arguments)]
fn create_market_reserve_without_liquidity(
    authority: &Keypair,
    manager_key: &Pubkey,
    pyth_product_key: &Pubkey,
    pyth_price_key: &Pubkey,
    mint_pubkey: &Pubkey,
    config: CollateralConfig,
    reserve_lamports: u64,
    account_lamports: u64,
    recent_blockhash: Hash,
) -> Result<Transaction, ProgramError> {
    let market_reserve = &Keypair::new();
    let token_account = &Keypair::new();
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
            mint_pubkey,
            manager_authority_key,
        )?,
        init_market_reserve_without_liquidity(
            *manager_key,
            *market_reserve_key,
            *pyth_product_key,
            *pyth_price_key,
            *mint_pubkey,
            *token_account_key,
            *authority_pubkey,
            config,
        )
    ],
    Some(authority_pubkey),
        &[market_reserve, token_account, authority],
        recent_blockhash,
    ))
}

#[allow(clippy::too_many_arguments)]
fn create_market_reserve_with_liquidity(
    authority: &Keypair,
    manager_key: &Pubkey,
    pyth_product_key: &Pubkey,
    pyth_price_key: &Pubkey,
    mint_pubkey: &Pubkey,
    rate_oracle_pubkey: &Pubkey,
    collateral_config: CollateralConfig,
    liquidity_config: LiquidityConfig,
    reserve_lamports: u64,
    account_lamports: u64,
    recent_blockhash: Hash,
) -> Result<Transaction, ProgramError> {
    let market_reserve = &Keypair::new();
    let token_account = &Keypair::new();
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
            mint_pubkey,
            manager_authority_key,
        )?,
        init_market_reserve_with_liquidity(
            *manager_key,
            *market_reserve_key,
            *pyth_product_key,
            *pyth_price_key,
            *mint_pubkey,
            *token_account_key,
            *authority_pubkey,
            *rate_oracle_pubkey,
            collateral_config,
            liquidity_config,
        )
    ],
    Some(authority_pubkey),
        &[market_reserve, token_account, authority],
        recent_blockhash,
    ))
}