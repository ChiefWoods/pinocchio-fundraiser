use pinocchio::{account_info::AccountInfo, program_error::ProgramError};

use crate::AccountCheck;

pub struct SystemAccount;

impl AccountCheck for SystemAccount {
    fn check(account: &AccountInfo) -> Result<(), ProgramError> {
        if account.owner().ne(&pinocchio_system::ID) {
            return Err(ProgramError::InvalidAccountOwner.into());
        }

        Ok(())
    }
}