use pinocchio::{ProgramResult, account_info::AccountInfo, program_error::ProgramError, sysvars::{Sysvar, rent::Rent}};
use pinocchio_system::instructions::CreateAccount;
use pinocchio_token_2022::{instructions::InitializeMint2, state::Mint};

use crate::{AccountCheck, MintInit, helpers::{TOKEN_2022_ACCOUNT_DISCRIMINATOR_OFFSET, TOKEN_2022_MINT_DISCRIMINATOR}};

pub struct Mint2022Account;

impl AccountCheck for Mint2022Account {
    fn check(account: &AccountInfo) -> Result<(), ProgramError> {
        if account.owner().ne(&pinocchio_token_2022::ID) {
            return Err(ProgramError::InvalidAccountOwner.into());
        }

        let data = account.try_borrow_data()?;

        if data.len().ne(&Mint::BASE_LEN) {
            if data[TOKEN_2022_ACCOUNT_DISCRIMINATOR_OFFSET].ne(&TOKEN_2022_MINT_DISCRIMINATOR) {
                return Err(ProgramError::InvalidAccountData.into());
            }
        }

        Ok(())
    }
}

impl MintInit for Mint2022Account {
    fn init(
        account: &AccountInfo,
        payer: &AccountInfo,
        decimals: u8,
        mint_authority: &[u8; 32],
        freeze_authority: Option<&[u8; 32]>,
    ) -> ProgramResult {
        let lamports = Rent::get()?.minimum_balance(Mint::BASE_LEN);

        CreateAccount {
            from: payer,
            to: account,
            lamports,
            space: Mint::BASE_LEN as u64,
            owner: &pinocchio_token_2022::ID,
        }
        .invoke()?;

        InitializeMint2 {
            mint: account,
            decimals,
            mint_authority,
            freeze_authority,
            token_program: &pinocchio_token_2022::ID,
        }
        .invoke()
    }

    fn init_if_needed(
        account: &AccountInfo,
        payer: &AccountInfo,
        decimals: u8,
        mint_authority: &[u8; 32],
        freeze_authority: Option<&[u8; 32]>,
    ) -> ProgramResult {
        match Self::check(account) {
            Ok(_) => Ok(()),
            Err(_) => Self::init(account, payer, decimals, mint_authority, freeze_authority),
        }
    }
}