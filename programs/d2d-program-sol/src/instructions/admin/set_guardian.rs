use crate::errors::ErrorCode;
use crate::events::GuardianSet;
use crate::states::TreasuryPool;
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct SetGuardian<'info> {
    #[account(
        mut,
        seeds = [TreasuryPool::PREFIX_SEED],
        bump = treasury_pool.bump
    )]
    pub treasury_pool: Account<'info, TreasuryPool>,

    #[account(
        constraint = admin.key() == treasury_pool.admin @ ErrorCode::Unauthorized
    )]
    pub admin: Signer<'info>,
}

pub fn set_guardian(ctx: Context<SetGuardian>, new_guardian: Pubkey) -> Result<()> {
    let treasury_pool = &mut ctx.accounts.treasury_pool;

    if new_guardian != Pubkey::default() {
        require!(
            new_guardian != treasury_pool.admin,
            ErrorCode::InvalidGuardianAddress
        );
    }

    let old_guardian = treasury_pool.guardian;
    treasury_pool.guardian = new_guardian;

    emit!(GuardianSet {
        admin: ctx.accounts.admin.key(),
        old_guardian,
        new_guardian,
        set_at: Clock::get()?.unix_timestamp,
    });

    Ok(())
}
