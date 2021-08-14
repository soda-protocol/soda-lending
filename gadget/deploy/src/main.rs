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
    instruction::{initialize_mint, initialize_account, mint_to},
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
    
    let lamports1 = client.get_minimum_balance_for_rent_exemption(Mint::LEN).unwrap();
    let lamports2 = client.get_minimum_balance_for_rent_exemption(Account::LEN).unwrap();

    let authority = &Keypair::new();
    println!("authority keypair: {:?}, pubkey: {:?}", authority.to_base58_string(), authority.pubkey());

    let mint = &Keypair::new();
    let token_account = &Keypair::new();
    println!("mint key: {:?}, token account pubkey: {:?}", mint.pubkey(), token_account.pubkey());

    client.request_airdrop(&authority.pubkey(), 10_000_000_000).unwrap();

    thread::sleep(Duration::from_secs(30));

    let (block_hash, _) = client.get_recent_blockhash().unwrap();

    let transaction = create_token(
        mint,
        authority, 
        token_account, 
        lamports1, 
        lamports2,
        1_000_000_000_000_000_000, 
        block_hash
    ).unwrap();

    match client.send_and_confirm_transaction(&transaction) {
        Ok(sig) => println!("sig is {:?}", sig),
        Err(err) => println!("error: {:?}", err),
    }
}

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

// fn create_token_account(
//     account: &Keypair,
//     owner: &Keypair,
//     mint_pubkey: &Pubkey,
//     lamports: u64,
//     recent_blockhash: Hash,
// ) -> Result<Transaction, ProgramError> {
//     let program_id = spl_token::id();
//     let account_pubkey = &account.pubkey();
//     let owner_pubkey = &owner.pubkey();

//     Ok(Transaction::new_signed_with_payer(&[
//             create_account(
//                 owner_pubkey,
//                 account_pubkey,
//                 lamports,
//                 Account::LEN as u64,
//                 &program_id,
//             ),
//             initialize_account(
//                 &program_id,
//                 account_pubkey,
//                 mint_pubkey,
//                 owner_pubkey,
//             )?,
//         ],
//         Some(owner_pubkey),
//         &[account, owner],
//         recent_blockhash,
//     ))
// }

// fn token_mint_to(
//     account: &Keypair,
//     owner: &Keypair,
//     mint_pubkey: &Pubkey,
//     lamports: u64,
//     recent_blockhash: Hash,
// ) -> Result<Transaction, ProgramError> {
//     let program_id = spl_token::id();
//     let account_pubkey = &account.pubkey();
//     let owner_pubkey = &owner.pubkey();

//     Ok(Transaction::new_signed_with_payer(&[
//             initialize_account(
//                 &program_id,
//                 account_pubkey,
//                 mint_pubkey,
//                 owner_pubkey,
//             )?,
//         ],
//         Some(owner_pubkey),
//         &[account, owner],
//         recent_blockhash,
//     ))
// }