use pinocchio::{ProgramResult, account_info::AccountInfo, program_error::ProgramError, sysvars::{Sysvar, rent::Rent}};
use pinocchio_system::instructions::CreateAccount;
use pinocchio_token_2022::{instructions::InitializeAccount3, state::TokenAccount as TokenAccountState};

use crate::{AccountCheck, TokenInit, helpers::{TOKEN_2022_ACCOUNT_DISCRIMINATOR_OFFSET, TOKEN_2022_TOKEN_ACCOUNT_DISCRIMINATOR}};

pub struct TokenAccount2022Account;

impl AccountCheck for TokenAccount2022Account {
    fn check(account: &AccountInfo) -> Result<(), ProgramError> {
        if account.owner().ne(&pinocchio_token_2022::ID) {
            return Err(ProgramError::InvalidAccountOwner.into());
        }

        let data = account.try_borrow_data()?;

        if data.len().ne(&TokenAccountState::BASE_LEN) {
            if data[TOKEN_2022_ACCOUNT_DISCRIMINATOR_OFFSET]
                .ne(&TOKEN_2022_TOKEN_ACCOUNT_DISCRIMINATOR)
            {
                return Err(ProgramError::InvalidAccountData.into());
            }
        }

        Ok(())
    }
}

impl TokenInit for TokenAccount2022Account {
    fn init(
        account: &AccountInfo,
        mint: &AccountInfo,
        payer: &AccountInfo,
        owner: &[u8; 32],
    ) -> ProgramResult {
        let lamports = Rent::get()?.minimum_balance(TokenAccountState::BASE_LEN);

        CreateAccount {
            from: payer,
            to: account,
            lamports,
            space: TokenAccountState::BASE_LEN as u64,
            owner: &pinocchio_token_2022::ID,
        }
        .invoke()?;

        InitializeAccount3 {
            account,
            mint,
            owner,
            token_program: &pinocchio_token_2022::ID,
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