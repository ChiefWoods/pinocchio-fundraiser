use pinocchio::{
    ProgramResult,
    account_info::AccountInfo,
    program_error::ProgramError,
    sysvars::{Sysvar, rent::Rent},
};
use pinocchio_system::instructions::CreateAccount;
use pinocchio_token::{instructions::InitializeAccount3, state::TokenAccount as TokenAccountState};

use crate::{AccountCheck, TokenInit};

pub struct TokenAccount;

impl AccountCheck for TokenAccount {
    fn check(account: &AccountInfo) -> Result<(), ProgramError> {
        if account.owner().ne(&pinocchio_token::ID) {
            return Err(ProgramError::InvalidAccountOwner);
        }

        if account.data_len().ne(&TokenAccountState::LEN) {
            return Err(ProgramError::InvalidAccountData);
        }

        Ok(())
    }
}

impl TokenInit for TokenAccount {
    fn init(
        account: &AccountInfo,
        mint: &AccountInfo,
        payer: &AccountInfo,
        owner: &[u8; 32],
    ) -> ProgramResult {
        let lamports = Rent::get()?.minimum_balance(TokenAccountState::LEN);

        CreateAccount {
            from: payer,
            to: account,
            lamports,
            space: TokenAccountState::LEN as u64,
            owner: &pinocchio_token::ID,
        }
        .invoke()?;

        InitializeAccount3 {
            account,
            mint,
            owner,
        }
        .invoke()
    }

    fn init_if_needed(
        account: &AccountInfo,
        mint: &AccountInfo,
        payer: &AccountInfo,
        owner: &[u8; 32],
    ) -> ProgramResult {
        match Self::check(account) {
            Ok(_) => Ok(()),
            Err(_) => Self::init(account, mint, payer, owner),
        }
    }
}
