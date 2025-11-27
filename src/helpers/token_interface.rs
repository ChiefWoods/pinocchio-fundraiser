use pinocchio::{account_info::AccountInfo, program_error::ProgramError};
use pinocchio_token_2022::state::TokenAccount as TokenAccountState;

use crate::AccountCheck;

pub struct TokenAccountInterface;

impl AccountCheck for TokenAccountInterface {
    fn check(account: &AccountInfo) -> Result<(), ProgramError> {
        if account.owner().ne(&pinocchio_token_2022::ID) {
            if account.owner().ne(&pinocchio_token::ID) {
                return Err(ProgramError::InvalidAccountOwner);
            } else if account.data_len().ne(&TokenAccountState::BASE_LEN) {
                return Err(ProgramError::InvalidAccountData);
            }
        } else {
            let data = account.try_borrow_data()?;

            if data.len().ne(&TokenAccountState::BASE_LEN) {
                return Err(ProgramError::InvalidAccountData);
            }
        }

        Ok(())
    }
}
