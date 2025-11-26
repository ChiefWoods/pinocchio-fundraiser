use solana_pubkey::Pubkey;

use crate::{Contributor, Fundraise, Prefix, tests::constants::PROGRAM_ID};

pub fn get_fundraise_pda(maker: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[Fundraise::PREFIX, maker.as_ref()], &PROGRAM_ID).0
}

pub fn get_contributor_pda(fundraise: &Pubkey, authority: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[Contributor::PREFIX, fundraise.as_ref(), authority.as_ref()],
        &PROGRAM_ID,
    )
    .0
}
