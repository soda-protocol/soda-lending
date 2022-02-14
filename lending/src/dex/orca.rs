use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    program_pack::Pack,
    program_error::ProgramError,
};
use spl_token::state::Account;
use crate::{Data, invoker::process_invoke};

use super::Swapper;

// Orca testnet
const ORCA_PROGRAM: &str = "3xQ8SWv2GaFXXpHZNqkXsdxq5DZciHBz6ZFoPPfbFd7U";
const SUPPORTED_ORCA_SWAP_POOLS: [&str; 4] = [
    "GaCKuVZyo6HxUf6bkcWzDETGHqqViF6H77ax7Uxq3LXU", // orca/usdc
    "B4v9urCKnrdCMWt7rEPyA5xyuEeYQv4aDpCfGFVaCvox", // orca/sol
    "65AsoozQfBedPU3rGCB7CfBbSFhiFGaVQaeoF9mLFM3g", // sol/usdt
    "F9MgdfFEshXCTGbppcVr2DzpVxqkiVowGqd95S4vpC6D", // eth/sol
];

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct OrcaSwapData {
    /// SOURCE amount to transfer, output to DESTINATION is based on the exchange rate
    amount_in: u64,
    /// Minimum amount of DESTINATION token to output, prevents excessive slippage
    minimum_amount_out: u64,
}

impl Data for OrcaSwapData {
    fn to_vec(self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(1 + 8 + 8);
        buf.push(1);
        buf.extend_from_slice(&self.amount_in.to_le_bytes());
        buf.extend_from_slice(&self.minimum_amount_out.to_le_bytes());

        buf
    }
}

#[derive(Debug)]
pub struct OrcaSwapContext<'a, 'b> {
    pub swap_program: &'a AccountInfo<'b>,
    pub token_program: &'a AccountInfo<'b>,
    pub pool_info: &'a AccountInfo<'b>,
    pub pool_authority: &'a AccountInfo<'b>,
    pub pool_lp_token_mint: &'a AccountInfo<'b>,
    pub pool_source_token_account: &'a AccountInfo<'b>,
    pub pool_dest_token_account: &'a AccountInfo<'b>,
    pub pool_fee_account: &'a AccountInfo<'b>,
    pub user_authority: &'a AccountInfo<'b>,
    pub user_source_token_account: &'a AccountInfo<'b>,
    pub user_dest_token_account: &'a AccountInfo<'b>,
    pub signer_seeds: &'a [&'a [u8]],
}

impl<'a, 'b> Swapper<'a, 'b> for OrcaSwapContext<'a, 'b> {
    fn is_supported(&self) -> bool {
        let a = self.swap_program.key.to_string() == ORCA_PROGRAM;
        let b = SUPPORTED_ORCA_SWAP_POOLS
            .iter()
            .any(|&key| key == self.pool_info.key.to_string());

        a && b
    }

    fn get_user_source_token_balance(&self) -> Result<u64, ProgramError> {
        Ok(Account::unpack(&self.user_source_token_account.try_borrow_data()?)?.amount)
    }

    fn get_user_dest_token_balance(&self) -> Result<u64, ProgramError> {
        Ok(Account::unpack(&self.user_dest_token_account.try_borrow_data()?)?.amount)
    }

    fn get_pool_source_token_balance(&self) -> Result<u64, ProgramError> {
        Ok(Account::unpack(&self.pool_source_token_account.try_borrow_data()?)?.amount)
    }

    fn get_pool_dest_token_balance(&self) -> Result<u64, ProgramError> {
        Ok(Account::unpack(&self.pool_dest_token_account.try_borrow_data()?)?.amount)
    }

    fn swap(&self, amount_in: u64, minimum_amount_out: u64) -> ProgramResult {
        let data = OrcaSwapData { amount_in, minimum_amount_out };
        let mut user_authority = self.user_authority.clone();
        user_authority.is_signer = true;

        process_invoke(
            data,
            self.swap_program,
            vec![
                self.pool_info.clone(),
                self.pool_authority.clone(),
                user_authority,
                self.user_source_token_account.clone(),
                self.pool_source_token_account.clone(),
                self.pool_dest_token_account.clone(),
                self.user_dest_token_account.clone(),
                self.pool_lp_token_mint.clone(),
                self.pool_fee_account.clone(),
                self.token_program.clone(),
            ],
            self.signer_seeds,
        )
    }
}