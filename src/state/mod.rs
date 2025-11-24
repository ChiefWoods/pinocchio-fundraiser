use std::mem::transmute;

use pinocchio::program_error::ProgramError;

pub mod contributor;
pub mod fundraise;

pub use contributor::*;
pub use fundraise::*;

pub trait Prefix {
    const PREFIX: &'static [u8];
}

pub trait Space {
    const LEN: usize;
}

pub trait AccountLoad: Sized + Space {
    #[inline(always)]
    fn load(bytes: &[u8]) -> Result<&Self, ProgramError> {
        if bytes.len() != Self::LEN {
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(unsafe { &*transmute::<*const u8, *const Self>(bytes.as_ptr()) })
    }

    #[inline(always)]
    fn load_mut(bytes: &mut [u8]) -> Result<&mut Self, ProgramError> {
        if bytes.len() != Self::LEN {
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(unsafe { &mut *transmute::<*mut u8, *mut Self>(bytes.as_mut_ptr()) })
    }
}