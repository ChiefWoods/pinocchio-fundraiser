use pinocchio::{account_info::AccountInfo, program_error::ProgramError};
use pinocchio_token_2022::state::Mint;

use crate::{
    AccountCheck,
    helpers::{TOKEN_2022_ACCOUNT_DISCRIMINATOR_OFFSET, TOKEN_2022_MINT_DISCRIMINATOR},
};

pub struct MintInterface;

impl AccountCheck for MintInterface {
    fn check(account: &AccountInfo) -> Result<(), ProgramError> {
        if account.owner().ne(&pinocchio_token_2022::ID) {
            if account.owner().ne(&pinocchio_token::ID) {
                return Err(ProgramError::InvalidAccountOwner);
            } else if account.data_len().ne(&Mint::BASE_LEN) {
                return Err(ProgramError::InvalidAccountData);
            }
        } else {
            let data = account.try_borrow_data()?;

            if data.len().ne(&Mint::BASE_LEN)
                && data[TOKEN_2022_ACCOUNT_DISCRIMINATOR_OFFSET].ne(&TOKEN_2022_MINT_DISCRIMINATOR)
            {
                return Err(ProgramError::InvalidAccountData);
            }
        }

        Ok(())
    }
}
