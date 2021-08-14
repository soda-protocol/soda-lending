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
    signer::{Signer, keypair::Keypair},
    system_instruction::create_account,
    transaction::Transaction
};
use spl_token::{
    instruction::initialize_mint,
    state::Mint,
};

const dev_net: &str = "https://api.devnet.solana.com";

fn main() {
    let client = RpcClient::new_with_commitment(String::from(dev_net), CommitmentConfig::default());
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

    let mut transaction = Transaction::new_with_payer(&[
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
    );
    transaction.sign(&[mint, authority], recent_blockhash);
    
    Ok(transaction)
}
