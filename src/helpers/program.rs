use pinocchio::{ProgramResult, account_info::AccountInfo, instruction::{Seed, Signer}, program_error::ProgramError, pubkey::{Pubkey, create_program_address}, sysvars::{Sysvar, rent::Rent}};
use pinocchio_system::instructions::CreateAccount;

use crate::{AccountCheck, AccountLoad, FundraiserError, Space};

pub struct ProgramAccount;

pub trait SetInner: Sized {
    type Params;
    
    fn set_inner(&mut self, params: Self::Params);
}

impl AccountCheck for ProgramAccount {
    fn check(account: &AccountInfo) -> Result<(), ProgramError> {
        if account.owner().ne(&crate::ID) {
            return Err(ProgramError::InvalidAccountOwner.into());
        }

        Ok(())
    }
}

impl ProgramAccount {
    pub fn init<'a, T: Sized>(
        payer: &AccountInfo,
        account: &AccountInfo,
        seeds: &[Seed<'a>],
        space: usize,
    ) -> ProgramResult {
        let lamports = Rent::get()?.minimum_balance(space);

        let signer = [Signer::from(seeds)];

        CreateAccount {
            from: payer,
            to: account,
            lamports,
            space: space as u64,
            owner: &crate::ID,
        }
        .invoke_signed(&signer)?;

        Ok(())
    }

    pub fn init_if_needed<T: AccountLoad + Space + SetInner + Sized>(
        seeds: &[Seed<'_>],
        account: &AccountInfo,
        payer: &AccountInfo,
        params: T::Params
    ) -> ProgramResult {
        if Self::check(account).is_err() {
            Self::init::<T>(
                payer,
                account,
                seeds,
                T::LEN
            )?;

            let mut data = account.try_borrow_mut_data()?;
            let account = T::load_mut(data.as_mut())?;

            account.set_inner(params);
        }

        Ok(())
    }

    pub fn close(account: &AccountInfo, destination: &AccountInfo) -> ProgramResult {
        {
            let mut data = account.try_borrow_mut_data()?;
            data[0] = 0xff;
        }

        *destination.try_borrow_mut_lamports()? += *account.try_borrow_lamports()?;
        account.resize(1)?;
        account.close()
    }

    pub fn validate(seeds: &[&[u8]], address: Pubkey) -> ProgramResult {
        let pda = create_program_address(seeds, &crate::ID)?;

        if address != pda {
            return Err(FundraiserError::InvalidAddress.into());
        }

        Ok(())
    }
}