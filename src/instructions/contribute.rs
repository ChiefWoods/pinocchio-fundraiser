use core::mem::size_of;
use pinocchio::{
    ProgramResult,
    account_info::AccountInfo,
    instruction::Seed,
    program_error::ProgramError,
    pubkey::find_program_address,
    sysvars::{Sysvar, clock::Clock},
};
use pinocchio_token_2022::instructions::Transfer;

use crate::{
    AccountCheck, AccountLoad, AssociatedTokenAccount, Contributor, ContributorParams, Fundraise,
    FundraiserError, Handler, MAX_BPS, MAX_CONTRIBUTION_PERCENTAGE_BPS, MintInterface, Prefix,
    ProgramAccount, SECONDS_TO_DAYS, SignerAccount
};

pub struct ContributeAccounts<'a> {
    pub authority: &'a AccountInfo,
    pub mint_to_raise: &'a AccountInfo,
    pub fundraise: &'a AccountInfo,
    pub contributor: &'a AccountInfo,
    pub authority_token_account: &'a AccountInfo,
    pub vault: &'a AccountInfo,
    pub system_program: &'a AccountInfo,
    pub token_program: &'a AccountInfo,
}

impl<'a> TryFrom<&'a [AccountInfo]> for ContributeAccounts<'a> {
    type Error = ProgramError;

    fn try_from(accounts: &'a [AccountInfo]) -> Result<Self, Self::Error> {
        let [
            authority,
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
        AssociatedTokenAccount::check(
            authority_token_account,
            authority,
            mint_to_raise,
            token_program,
        )?;
        AssociatedTokenAccount::check(vault, fundraise, mint_to_raise, token_program)?;

        Ok(Self {
            authority,
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

pub struct ContributeInstructionData {
    pub amount: u64,
}

impl<'a> TryFrom<&'a [u8]> for ContributeInstructionData {
    type Error = ProgramError;

    fn try_from(data: &'a [u8]) -> Result<Self, Self::Error> {
        if data.len() != size_of::<u64>() {
            return Err(ProgramError::InvalidInstructionData);
        }

        let offset = size_of::<u64>();
        let amount = u64::from_le_bytes(data[0..offset].try_into().unwrap());

        Ok(Self { amount })
    }
}

pub struct Contribute<'a> {
    pub accounts: ContributeAccounts<'a>,
    pub data: ContributeInstructionData,
    pub bump: u8,
}

impl<'a> TryFrom<(&'a [u8], &'a [AccountInfo])> for Contribute<'a> {
    type Error = ProgramError;

    fn try_from((data, accounts): (&'a [u8], &'a [AccountInfo])) -> Result<Self, Self::Error> {
        let accounts = ContributeAccounts::try_from(accounts)?;
        let data = ContributeInstructionData::try_from(data)?;

        let (pda, bump) = find_program_address(
            &[
                Contributor::PREFIX,
                accounts.fundraise.key(),
                accounts.authority.key(),
            ],
            &crate::ID,
        );

        if pda != *accounts.contributor.key() {
            return Err(FundraiserError::InvalidAddress.into());
        }

        let bump_binding = [bump];
        let contributor_seeds = [
            Seed::from(Contributor::PREFIX),
            Seed::from(accounts.fundraise.key().as_ref()),
            Seed::from(accounts.authority.key().as_ref()),
            Seed::from(&bump_binding),
        ];
        let params =
            ContributorParams::new(*accounts.fundraise.key(), *accounts.authority.key(), bump);

        ProgramAccount::init_if_needed::<Contributor>(
            &contributor_seeds,
            accounts.contributor,
            accounts.authority,
            params,
        )?;

        Ok(Self {
            accounts,
            data,
            bump,
        })
    }
}

impl<'a> Handler<'a> for Contribute<'a> {
    const DISCRIMINATOR: &'a u8 = &1;

    fn process(&mut self) -> ProgramResult {
        let decimals = match *self.accounts.mint_to_raise.owner() {
            pinocchio_token::ID => {
                let mint = unsafe {
                    pinocchio_token::state::Mint::from_account_info_unchecked(
                        self.accounts.mint_to_raise,
                    )?
                };
                mint.decimals()
            }
            pinocchio_token_2022::ID => {
                let mint = unsafe {
                    pinocchio_token_2022::state::Mint::from_account_info_unchecked(
                        self.accounts.mint_to_raise,
                    )?
                };
                mint.decimals()
            }
            _ => return Err(ProgramError::IncorrectProgramId),
        };

        if self.data.amount <= 1u64.pow(decimals as u32) {
            return Err(FundraiserError::ContributionTooSmall.into());
        }

        let mut fundraise_data = self.accounts.fundraise.try_borrow_mut_data()?;
        let fundraise = Fundraise::load_mut(fundraise_data.as_mut())?;

        let fundraise_seeds = &[Fundraise::PREFIX, &fundraise.maker, &[fundraise.bump]];

        ProgramAccount::validate(fundraise_seeds, *self.accounts.fundraise.key())?;
        fundraise.check_mint_to_raise(self.accounts.mint_to_raise.key())?;

        let amount_to_raise = fundraise.get_amount_to_raise();

        if self.data.amount
            > amount_to_raise * u64::from(MAX_CONTRIBUTION_PERCENTAGE_BPS) / u64::from(MAX_BPS)
        {
            return Err(FundraiserError::ContributionTooBig.into());
        }

        let now = Clock::get()?.unix_timestamp;

        let duration = fundraise.get_duration();
        let time_started = fundraise.get_time_started();

        if now != time_started
            && duration as i64 > (now - time_started) / i64::from(SECONDS_TO_DAYS)
        {
            return Err(FundraiserError::FundraiserEnded.into());
        }

        let mut contributor_data = self.accounts.contributor.try_borrow_mut_data()?;
        let contributor = Contributor::load_mut(contributor_data.as_mut())?;

        let contributor_amount = contributor.get_amount();

        if (contributor_amount
            > amount_to_raise * u64::from(MAX_CONTRIBUTION_PERCENTAGE_BPS) / u64::from(MAX_BPS))
            || (contributor_amount + self.data.amount
                > amount_to_raise * u64::from(MAX_CONTRIBUTION_PERCENTAGE_BPS) / u64::from(MAX_BPS))
        {
            return Err(FundraiserError::MaximumContributionsReached.into());
        }

        let current_amount = fundraise.get_current_amount();
        fundraise.set_current_amount(current_amount + self.data.amount);

        let contributor_amount = contributor.get_amount();
        contributor.set_amount(contributor_amount + self.data.amount);

        Transfer {
            amount: self.data.amount,
            authority: self.accounts.authority,
            from: self.accounts.authority_token_account,
            to: self.accounts.vault,
            token_program: self.accounts.token_program.key(),
        }
        .invoke()?;

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
    use spl_token_2022::state::Account;

    use crate::{
        AccountLoad, Contributor, Fundraise,
        tests::{
            constants::{
                ASSOCIATED_TOKEN_PROGRAM_ID, MINT_DECIMALS, PROGRAM_ID, SYSTEM_PROGRAM_ID,
                TOKEN_PROGRAM_ID,
            },
            pda::{get_contributor_pda, get_fundraise_pda},
            utils::{
                build_and_send_transaction, fetch_account, init_ata, init_mint,
                init_wallet, setup,
            },
        },
    };

    #[test]
    fn contribute() {
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

        let fundraise_acc = litesvm.get_account(&fundraise_pda).unwrap();
        let fundraise = Fundraise::load(&fundraise_acc.data.as_ref()).unwrap();
        let pre_fundraise_current_amount = fundraise.get_current_amount();

        let pre_authority_ata_bal = fetch_account::<Account>(litesvm, &authority_ata).amount;
        let pre_vault_bal = fetch_account::<Account>(litesvm, &vault).amount;

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

        let contributor_acc = litesvm.get_account(&contributor_pda).unwrap();
        let contributor = Contributor::load(&contributor_acc.data.as_ref()).unwrap();

        assert_eq!(contributor.fundraise, fundraise_pda.to_bytes());
        assert_eq!(contributor.authority, authority.pubkey().to_bytes());
        assert_eq!(contributor.get_amount(), contribute_amount);

        let fundraise_acc = litesvm.get_account(&fundraise_pda).unwrap();
        let fundraise = Fundraise::load(&fundraise_acc.data.as_ref()).unwrap();
        let post_fundraise_current_amount = fundraise.get_current_amount();

        assert_eq!(
            pre_fundraise_current_amount,
            post_fundraise_current_amount - contribute_amount
        );

        let post_authority_ata_bal = fetch_account::<Account>(litesvm, &authority_ata).amount;
        let post_vault_bal = fetch_account::<Account>(litesvm, &vault).amount;

        assert_eq!(
            pre_authority_ata_bal,
            post_authority_ata_bal + contribute_amount
        );
        assert_eq!(pre_vault_bal, post_vault_bal - contribute_amount);
    }
}
