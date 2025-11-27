use pinocchio::ProgramResult;

pub mod claim;
pub mod contribute;
pub mod initialize;
pub mod refund;

pub use claim::*;
pub use contribute::*;
pub use initialize::*;
pub use refund::*;

pub trait Handler<'a> {
    const DISCRIMINATOR: &'a u8;

    fn process(&mut self) -> ProgramResult;
}
