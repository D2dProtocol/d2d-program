use crate::errors::ErrorCode;
use crate::events::GuardianPaused;
use crate::states::TreasuryPool;
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct GuardianPause<'info> {
    #[account(
        mut,
        seeds = [TreasuryPool::PREFIX_SEED],
        bump = treasury_pool.bump
    )]
    pub treasury_pool: Account<'info, TreasuryPool>,

    #[account(mut)]
    pub guardian: Signer<'info>,
}

pub fn guardian_pause(ctx: Context<GuardianPause>) -> Result<()> {
    let treasury_pool = &mut ctx.accounts.treasury_pool;

    require!(treasury_pool.has_guardian(), ErrorCode::GuardianNotSet);
    require!(
        ctx.accounts.guardian.key() == treasury_pool.guardian,
        ErrorCode::OnlyGuardian
    );

    if treasury_pool.emergency_pause {
        return Ok(());
    }

    treasury_pool.emergency_pause = true;

    emit!(GuardianPaused {
        guardian: ctx.accounts.guardian.key(),
        paused_at: Clock::get()?.unix_timestamp,
    });

    Ok(())
}
