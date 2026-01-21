use crate::errors::ErrorCode;
use crate::events::TimelockDurationChanged;
use crate::states::TreasuryPool;
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct SetTimelockDuration<'info> {
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

pub fn set_timelock_duration(
    ctx: Context<SetTimelockDuration>,
    new_duration: i64,
) -> Result<()> {
    let treasury_pool = &mut ctx.accounts.treasury_pool;

    require!(
        new_duration >= TreasuryPool::MIN_TIMELOCK_DURATION,
        ErrorCode::InvalidTimelockDuration
    );
    require!(
        new_duration <= TreasuryPool::MAX_TIMELOCK_DURATION,
        ErrorCode::InvalidTimelockDuration
    );

    let old_duration = treasury_pool.timelock_duration;
    treasury_pool.timelock_duration = new_duration;

    emit!(TimelockDurationChanged {
        admin: ctx.accounts.admin.key(),
        old_duration,
        new_duration,
        changed_at: Clock::get()?.unix_timestamp,
    });

    Ok(())
}
