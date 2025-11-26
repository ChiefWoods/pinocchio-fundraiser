use pinocchio::{
    ProgramResult,
    account_info::AccountInfo,
    instruction::{Seed, Signer},
    program_error::ProgramError,
    sysvars::{Sysvar, clock::Clock},
};
use pinocchio_token_2022::instructions::{CloseAccount, Transfer};

use crate::{
    AccountCheck, AccountLoad, AssociatedTokenAccount, Contributor, Fundraise, FundraiserError,
    Handler, MintInterface, Prefix, ProgramAccount, SECONDS_TO_DAYS, SignerAccount,
};

pub struct RefundAccounts<'a> {
    pub authority: &'a AccountInfo,
    pub maker: &'a AccountInfo,
    pub mint_to_raise: &'a AccountInfo,
    pub fundraise: &'a AccountInfo,
    pub contributor: &'a AccountInfo,
    pub authority_token_account: &'a AccountInfo,
    pub vault: &'a AccountInfo,
    pub system_program: &'a AccountInfo,
    pub token_program: &'a AccountInfo,
}

impl<'a> TryFrom<&'a [AccountInfo]> for RefundAccounts<'a> {
    type Error = ProgramError;

    fn try_from(accounts: &'a [AccountInfo]) -> Result<Self, Self::Error> {
        let [
            authority,
            maker,
            mint_to_raise,
            fundraise,
            contributor,
            authority_token_account,
            vault,
            system_program,
            token_program,
        ] = accounts
        else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };

        SignerAccount::check(authority)?;
        MintInterface::check(mint_to_raise)?;
        ProgramAccount::check(fundraise)?;
        ProgramAccount::check(contributor)?;
        AssociatedTokenAccount::check(
            authority_token_account,
            authority,
            mint_to_raise,
            token_program,
        )?;
        AssociatedTokenAccount::check(vault, fundraise, mint_to_raise, token_program)?;

        Ok(Self {
            authority,
            maker,
            mint_to_raise,
            fundraise,
            contributor,
            authority_token_account,
            vault,
            system_program,
            token_program,
        })
    }
}

pub struct Refund<'a> {
    pub accounts: RefundAccounts<'a>,
}

impl<'a> TryFrom<&'a [AccountInfo]> for Refund<'a> {
    type Error = ProgramError;

    fn try_from(accounts: &'a [AccountInfo]) -> Result<Self, Self::Error> {
        let accounts = RefundAccounts::try_from(accounts)?;

        Ok(Self { accounts })
    }
}

impl<'a> Handler<'a> for Refund<'a> {
    const DISCRIMINATOR: &'a u8 = &2;

    fn process(&mut self) -> ProgramResult {
        let mut fundraise_data = self.accounts.fundraise.try_borrow_mut_data()?;
        let fundraise = Fundraise::load_mut(fundraise_data.as_mut())?;

        let fundraise_maker = fundraise.maker;
        let fundraise_bump = [fundraise.bump];
        let fundraise_seeds = &[Fundraise::PREFIX, &fundraise_maker, &fundraise_bump];

        ProgramAccount::validate(fundraise_seeds, *self.accounts.fundraise.key())?;
        fundraise.check_mint_to_raise(self.accounts.mint_to_raise.key())?;

        let contributor_data = self.accounts.contributor.try_borrow_data()?;
        let contributor = Contributor::load(&contributor_data)?;

        let contributor_seeds = &[
            Contributor::PREFIX,
            &contributor.fundraise,
            &contributor.authority,
            &[contributor.bump],
        ];

        ProgramAccount::validate(contributor_seeds, *self.accounts.contributor.key())?;

        let now = Clock::get()?.unix_timestamp;

        let duration = fundraise.get_duration();
        let time_started = fundraise.get_time_started();

        if (duration as i64) < (now - time_started) / i64::from(SECONDS_TO_DAYS) {
            return Err(FundraiserError::FundraiserEnded.into());
        }

        let vault_amount = match *self.accounts.vault.owner() {
            pinocchio_token::ID => {
                let vault = unsafe {
                    pinocchio_token::state::TokenAccount::from_account_info_unchecked(
                        self.accounts.vault,
                    )?
                };
                vault.amount()
            }
            pinocchio_token_2022::ID => {
                let vault = unsafe {
                    pinocchio_token_2022::state::TokenAccount::from_account_info_unchecked(
                        self.accounts.vault,
                    )?
                };
                vault.amount()
            }
            _ => return Err(ProgramError::IncorrectProgramId),
        };

        let amount_to_raise = fundraise.get_amount_to_raise();

        if vault_amount >= amount_to_raise {
            return Err(FundraiserError::TargetMet.into());
        }

        let current_amount = fundraise.get_current_amount();
        let contributor_amount = contributor.get_amount();
        fundraise.set_current_amount(current_amount - contributor_amount);

        let fundraise_seeds = [
            Seed::from(Fundraise::PREFIX),
            Seed::from(fundraise_maker.as_ref()),
            Seed::from(&fundraise_bump),
        ];
        let fundraise_signer = Signer::from(&fundraise_seeds);

        drop(fundraise_data);
        Transfer {
            amount: contributor_amount,
            authority: self.accounts.fundraise,
            from: self.accounts.vault,
            to: self.accounts.authority_token_account,
            token_program: self.accounts.token_program.key(),
        }
        .invoke_signed(&[fundraise_signer.clone()])?;

        if vault_amount - contributor_amount == 0 {
            CloseAccount {
                account: self.accounts.vault,
                destination: self.accounts.maker,
                authority: self.accounts.fundraise,
                token_program: self.accounts.token_program.key(),
            }
            .invoke_signed(&[fundraise_signer])?;
        }
        
        drop(contributor_data);
        ProgramAccount::close(self.accounts.contributor, self.accounts.authority)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use solana_instruction::{AccountMeta, Instruction};
    use solana_signer::Signer;
    use spl_associated_token_account::{
        get_associated_token_address_with_program_id,
        solana_program::{clock::SECONDS_PER_DAY, native_token::LAMPORTS_PER_SOL},
    };

    use crate::{
        AccountLoad, Contributor, Fundraise,
        tests::{
            constants::{
                ASSOCIATED_TOKEN_PROGRAM_ID, MINT_DECIMALS, PROGRAM_ID, SYSTEM_PROGRAM_ID,
                TOKEN_PROGRAM_ID,
            },
            pda::{get_contributor_pda, get_fundraise_pda},
            utils::{
                build_and_send_transaction, forward_time, init_ata, init_mint,
                init_wallet, setup,
            },
        },
    };

    #[test]
    fn refund() {
        let (litesvm, _default_payer) = &mut setup();
        let maker = init_wallet(litesvm, LAMPORTS_PER_SOL);
        let authority = init_wallet(litesvm, LAMPORTS_PER_SOL);
        let mint_to_raise = init_mint(litesvm, TOKEN_PROGRAM_ID, MINT_DECIMALS, 1_000_000_000);
        let authority_ata = init_ata(litesvm, mint_to_raise, authority.pubkey(), 1_000_000_000);

        let amount_to_raise: u64 = 5_000_000;
        let duration: u64 = SECONDS_PER_DAY; // 1 day
        let fundraise_pda = get_fundraise_pda(&maker.pubkey());
        let vault = get_associated_token_address_with_program_id(
            &fundraise_pda,
            &mint_to_raise,
            &TOKEN_PROGRAM_ID,
        );

        let data = [
            vec![0u8],
            amount_to_raise.to_le_bytes().to_vec(),
            duration.to_le_bytes().to_vec(),
        ]
        .concat();
        let ix = Instruction {
            program_id: PROGRAM_ID,
            accounts: vec![
                AccountMeta::new(maker.pubkey(), true),
                AccountMeta::new_readonly(mint_to_raise, false),
                AccountMeta::new(fundraise_pda, false),
                AccountMeta::new(vault, false),
                AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
                AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
                AccountMeta::new_readonly(ASSOCIATED_TOKEN_PROGRAM_ID, false),
            ],
            data,
        };

        let _ = build_and_send_transaction(litesvm, &[&maker], &maker.pubkey(), &[ix]);

        let contribute_amount: u64 = 500_000;
        let contributor_pda = get_contributor_pda(&fundraise_pda, &authority.pubkey());

        let data = [vec![1u8], contribute_amount.to_le_bytes().to_vec()].concat();
        let ix = Instruction {
            program_id: PROGRAM_ID,
            accounts: vec![
                AccountMeta::new(authority.pubkey(), true),
                AccountMeta::new_readonly(mint_to_raise, false),
                AccountMeta::new(fundraise_pda, false),
                AccountMeta::new(contributor_pda, false),
                AccountMeta::new(authority_ata, false),
                AccountMeta::new(vault, false),
                AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
                AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
            ],
            data,
        };

        let _ = build_and_send_transaction(litesvm, &[&authority], &authority.pubkey(), &[ix]);

        forward_time(litesvm, 3600); // jump forward 1 hour

        let fundraise_acc = litesvm.get_account(&fundraise_pda).unwrap();
        let fundraise = Fundraise::load(&fundraise_acc.data.as_ref()).unwrap();

        let pre_fundraise_current_amount = fundraise.get_current_amount();

        let contributor_acc = litesvm.get_account(&contributor_pda).unwrap();
        let contributor = Contributor::load(&contributor_acc.data.as_ref()).unwrap();

        let contributor_amount = contributor.get_amount();

        let data = vec![2u8];
        let ix = Instruction {
            program_id: PROGRAM_ID,
            accounts: vec![
                AccountMeta::new(authority.pubkey(), true),
                AccountMeta::new(maker.pubkey(), false),
                AccountMeta::new_readonly(mint_to_raise, false),
                AccountMeta::new(fundraise_pda, false),
                AccountMeta::new(contributor_pda, false),
                AccountMeta::new(authority_ata, false),
                AccountMeta::new(vault, false),
                AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
                AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
            ],
            data,
        };

        let _ = build_and_send_transaction(litesvm, &[&authority], &authority.pubkey(), &[ix]);

        let fundraise_acc = litesvm.get_account(&fundraise_pda).unwrap();
        let fundraise = Fundraise::load(&fundraise_acc.data.as_ref()).unwrap();

        let post_fundraise_current_amount = fundraise.get_current_amount();

        assert_eq!(
            pre_fundraise_current_amount,
            post_fundraise_current_amount + contributor_amount
        );

        let contributor_acc = litesvm.get_account(&contributor_pda);

        assert!(contributor_acc.is_none());

        let vault = litesvm.get_account(&vault);

        assert!(vault.is_none());
    }
}
