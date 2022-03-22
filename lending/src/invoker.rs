#![allow(missing_docs)]
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    pubkey::Pubkey,
    sysvar::{rent::Rent, Sysvar},
    system_instruction,
    program_error::ProgramError, instruction::{Instruction, AccountMeta}, program::{invoke, invoke_signed},
};
use crate::{error::LendingError, Data};

#[inline(always)]
pub fn process_transfer<'a>(
    source: &AccountInfo<'a>,
    destination: &AccountInfo<'a>,
    lamports: u64,
    signer_seeds: &[&[u8]],
) -> ProgramResult {
    let result = invoke_optionally_signed(
        &system_instruction::transfer(
            &source.key,
            &destination.key,
            lamports,
        ),
        &[source.clone(), destination.clone()],
        signer_seeds,
    );
    result.map_err(|_| ProgramError::InsufficientFunds.into())
}

#[inline(always)]
pub fn process_token_transfer<'a>(
    token_program: &AccountInfo<'a>,
    source: &AccountInfo<'a>,
    destination: &AccountInfo<'a>,
    authority: &AccountInfo<'a>,
    amount: u64,
    signer_seeds: &[&[u8]],
) -> ProgramResult {
    let result = invoke_optionally_signed(
        &spl_token::instruction::transfer(
            token_program.key,
            source.key,
            destination.key,
            authority.key,
            &[],
            amount,
        )?,
        &[
            source.clone(),
            destination.clone(),
            authority.clone(),
            token_program.clone(),
        ],
        signer_seeds,
    );
    result.map_err(|_| LendingError::TokenTransferFailed.into())
}

#[inline(always)]
pub fn process_token_approve<'a>(
    token_program: &AccountInfo<'a>,
    source: &AccountInfo<'a>,
    delegate: &AccountInfo<'a>,
    authority: &AccountInfo<'a>,
    amount: u64,
    signer_seeds: &[&[u8]],
) -> ProgramResult {
    let result = invoke_optionally_signed(
        &spl_token::instruction::approve(
            token_program.key,
            source.key,
            delegate.key,
            authority.key,
            &[],
            amount,
        )?,
        &[
            source.clone(),
            delegate.clone(),
            authority.clone(),
            token_program.clone(),
        ],
        signer_seeds,
    );
    result.map_err(|_| LendingError::TokenApproveFailed.into())
}

#[inline(always)]
pub fn process_token_revoke<'a>(
    token_program: &AccountInfo<'a>,
    source: &AccountInfo<'a>,
    authority: &AccountInfo<'a>,
    signer_seeds: &[&[u8]],
) -> ProgramResult {
    let result = invoke_optionally_signed(
        &spl_token::instruction::revoke(
            token_program.key,
            source.key,
            authority.key,
            &[],
        )?,
        &[source.clone(), authority.clone(), token_program.clone()],
        signer_seeds,
    );
    result.map_err(|_| LendingError::TokenRevokeFailed.into())
}

#[inline(always)]
pub fn process_token_burn<'a>(
    token_program: &AccountInfo<'a>,
    source: &AccountInfo<'a>,
    mint: &AccountInfo<'a>,
    authority: &AccountInfo<'a>,
    amount: u64,
    signer_seeds: &[&[u8]],
) -> ProgramResult {
    let result = invoke_optionally_signed(
        &spl_token::instruction::burn(
            token_program.key,
            source.key,
            mint.key,
            authority.key,
            &[],
            amount,
        )?,
        &[source.clone(), mint.clone(), authority.clone(), token_program.clone()],
        signer_seeds,
    );
    result.map_err(|_| LendingError::TokenBurnFailed.into())
}

#[inline(always)]
pub fn process_token_mint_to<'a>(
    token_program: &AccountInfo<'a>,
    mint: &AccountInfo<'a>,
    destination: &AccountInfo<'a>,
    authority: &AccountInfo<'a>,
    amount: u64,
    signer_seeds: &[&[u8]],
) -> ProgramResult {
    let result = invoke_optionally_signed(
        &spl_token::instruction::mint_to(
            token_program.key,
            mint.key,
            destination.key,
            authority.key,
            &[],
            amount,
        )?,
        &[mint.clone(), destination.clone(), authority.clone(), token_program.clone()],
        signer_seeds,
    );
    result.map_err(|_| LendingError::TokenMintToFailed.into())
}

#[inline(always)]
pub fn process_token_init_mint<'a>(
    token_program: &AccountInfo<'a>,
    mint: &AccountInfo<'a>,
    rent: &AccountInfo<'a>,
    authority: &Pubkey,
    decimals: u8,
) -> ProgramResult {
    let result = invoke(
        &spl_token::instruction::initialize_mint(
            token_program.key,
            mint.key,
            authority,
            None,
            decimals,
        )?,
        &[mint.clone(), rent.clone(), token_program.clone()],
    );
    result.map_err(|_| LendingError::TokenInitializeMintFailed.into())
}

#[inline(always)]
pub fn process_token_init_account<'a>(
    token_program: &AccountInfo<'a>,
    account: &AccountInfo<'a>,
    mint: &AccountInfo<'a>,
    rent: &AccountInfo<'a>,
    owner: &AccountInfo<'a>,
) -> ProgramResult {
    let ix = spl_token::instruction::initialize_account(
        token_program.key,
        account.key,
        mint.key,
        owner.key,
    )?;
    let result = invoke(
        &ix,
        &[
            account.clone(),
            mint.clone(),
            owner.clone(),
            rent.clone(),
            token_program.clone(),
        ],
    );
    result.map_err(|_| LendingError::TokenInitializeAccountFailed.into())
}

#[inline(never)]
#[allow(clippy::too_many_arguments)]
pub fn process_optimal_create_account<'a>(
    rent_info: &AccountInfo<'a>,
    target_account_info: &AccountInfo<'a>,
    authority_info: &AccountInfo<'a>,
    system_program_info: &AccountInfo<'a>,
    owner: &Pubkey,
    data_len: usize,
    signer_seeds: &[&[u8]],
    target_signer_seeds: &[&[u8]],
) -> ProgramResult {
    if target_account_info.owner == owner {
        return Ok(());
    } else if target_account_info.owner != system_program_info.key {
        return Err(ProgramError::IllegalOwner);
    }

    let required_lamports = Rent::from_account_info(rent_info)?
        .minimum_balance(data_len)
        .saturating_sub(target_account_info.lamports());

    if required_lamports > 0 {
        invoke_optionally_signed(
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
            signer_seeds,
        )?;
    }

    invoke_optionally_signed(
        &system_instruction::allocate(target_account_info.key, data_len as u64),
        &[target_account_info.clone(), system_program_info.clone()],
        target_signer_seeds,
    )?;

    invoke_optionally_signed(
        &system_instruction::assign(target_account_info.key, owner),
        &[target_account_info.clone(), system_program_info.clone()],
        target_signer_seeds,
    )
}

/// Invoke signed unless signers seeds are empty
#[inline(always)]
pub fn invoke_optionally_signed(
    instruction: &Instruction,
    account_infos: &[AccountInfo],
    signer_seeds: &[&[u8]],
) -> ProgramResult {
    if signer_seeds.is_empty() {
        invoke(instruction, account_infos)
    } else {
        invoke_signed(instruction, account_infos, &[signer_seeds])
    }
}

pub fn process_invoke<'a, D: Data>(
    data: D,
    program_info: &AccountInfo<'a>,
    mut account_infos: Vec<AccountInfo<'a>>,
    signer_seeds: &[&[u8]],
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
    account_infos.push(program_info.clone());

    invoke_optionally_signed(
        &Instruction {
            program_id: *program_info.key,
            accounts: instruction_accounts,
            data: data.to_vec(),
        },
        &account_infos,
        signer_seeds,
    )
}
