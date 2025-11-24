use pinocchio::pubkey::Pubkey;

use crate::{AccountLoad, Prefix, SetInner, Space};
use core::mem::size_of;

#[repr(C)]
pub struct Contributor {
    pub fundraise: Pubkey,
    pub authority: Pubkey,
    amount: [u8; 8],
    pub bump: u8,
}

impl Prefix for Contributor {
    const PREFIX: &'static [u8] = b"contributor";
}

impl Space for Contributor {
    const LEN: usize = size_of::<Self>();
}

impl AccountLoad for Contributor {}

impl Contributor {
    #[inline(always)]
    pub fn get_amount(&self) -> u64 {
        u64::from_le_bytes(self.amount)
    }

    #[inline(always)]
    pub fn set_amount(&mut self, amount: u64) {
        self.amount = amount.to_le_bytes();
    }
}

pub struct ContributorParams {
    pub fundraise: Pubkey,
    pub authority: Pubkey,
    pub bump: u8,
}

impl ContributorParams {
    pub fn new(fundraise: Pubkey, authority: Pubkey, bump: u8) -> Self {
        Self {
            fundraise,
            authority,
            bump,
        }
    }
}

impl SetInner for Contributor {
    type Params = ContributorParams;
    
    fn set_inner(&mut self, params: Self::Params) {
        self.fundraise = params.fundraise;
        self.authority = params.authority;
        self.set_amount(0);
        self.bump = params.bump;
    }
}
