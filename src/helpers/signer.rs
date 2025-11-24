use pinocchio::{account_info::AccountInfo, program_error::ProgramError};

use crate::{AccountCheck, FundraiserError};

pub struct SignerAccount;

impl AccountCheck for SignerAccount {
    fn check(account: &AccountInfo) -> Result<(), ProgramError> {
        if !account.is_signer() {
            return Err(FundraiserError::NotSigner.into());
        }
        Ok(())
    }
}
