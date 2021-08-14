use std::{str::FromStr, time::Duration, error::Error, thread};

use solana_client::{
    blockhash_query::BlockhashQuery, 
    rpc_client::RpcClient, 
    rpc_request::TokenAccountsFilter,
};
use solana_sdk::{
    commitment_config::CommitmentConfig, 
    hash::Hash, 
    program_error::ProgramError, 
    program_pack::Pack, 
    pubkey::Pubkey, 
    signer::{Signer, keypair::Keypair}, 
    system_instruction::create_account, 
    transaction::Transaction,
};
use spl_token::{
    instruction::{initialize_mint, initialize_account},
    state::{Mint, Account},
};
use soda_lending_contract::{
    state::{},
    instruction::{
        init_manager, init_rate_oracle, init_market_reserve_without_liquidity,
        init_market_reserve_with_liquidity, init_user_obligation,
        init_user_asset, deposit_liquidity, withdraw_liquidity,
        deposit_collateral, borrow_liquidity, repay_loan,
        redeem_collateral, liquidate, feed_rate_oracle, pause_rate_oracle,
        add_liquidity_to_market_reserve, withdraw_fee,
    }
};

const DEV_NET: &str = "https://api.devnet.solana.com";

fn main() {
    let client = RpcClient::new_with_commitment(String::from(DEV_NET), CommitmentConfig::default());
    let lamports = client.get_minimum_balance_for_rent_exemption(Mint::LEN).unwrap();
    let authority = &Keypair::new();
    println!("authority keypair: {:?}, pubkey: {:?}", authority.to_base58_string(), authority.pubkey());

    let mint = &Keypair::new();
    println!("mint key: {:?}", mint.pubkey());
    client.request_airdrop(&authority.pubkey(), lamports * 3).unwrap();

    thread::sleep(Duration::from_secs(30));

    let (block_hash, _) = client.get_recent_blockhash().unwrap();

    let transaction = create_token_mint(mint, authority, lamports, block_hash).unwrap();

    match client.send_and_confirm_transaction(&transaction) {
        Ok(sig) => println!("sig is {:?}", sig),
        Err(err) => println!("error: {:?}", err),
    }
}

fn create_token_mint(
    mint: &Keypair,
    authority: &Keypair,
    lamports: u64,
    recent_blockhash: Hash,
) -> Result<Transaction, ProgramError> {
    let program_id = spl_token::id();
    let mint_pubkey = &mint.pubkey();
    let authority_pubkey = &authority.pubkey();

    Ok(Transaction::new_signed_with_payer(&[
            create_account(
                authority_pubkey,
                mint_pubkey,
                lamports,
                Mint::LEN as u64,
                &program_id,
            ),
            initialize_mint(
                &program_id,
                mint_pubkey,
                authority_pubkey,
                None,
                9,
            )?,
        ],
        Some(authority_pubkey),
        &[mint, authority],
        recent_blockhash,
    ))
}

fn create_token_account(
    account: &Keypair,
    owner: &Keypair,
    mint_pubkey: &Pubkey,
    lamports: u64,
    recent_blockhash: Hash,
) -> Result<Transaction, ProgramError> {
    let program_id = spl_token::id();
    let account_pubkey = &account.pubkey();
    let owner_pubkey = &owner.pubkey();

    Ok(Transaction::new_signed_with_payer(&[
            create_account(
                owner_pubkey,
                account_pubkey,
                lamports,
                Account::LEN as u64,
                &program_id,
            ),
            initialize_account(
                &program_id,
                account_pubkey,
                mint_pubkey,
                owner_pubkey,
            )?,
        ],
        Some(owner_pubkey),
        &[account, owner],
        recent_blockhash,
    ))
}

fn 
