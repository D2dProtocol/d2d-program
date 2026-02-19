use anchor_lang::prelude::*;

use crate::{errors::ErrorCode, events::DailyLimitChanged, states::TreasuryPool};

#[derive(Accounts)]
pub struct SetDailyLimit<'info> {
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

pub fn set_daily_limit(ctx: Context<SetDailyLimit>, new_limit: u64) -> Result<()> {
  let treasury_pool = &mut ctx.accounts.treasury_pool;

  let old_limit = treasury_pool.daily_withdrawal_limit;
  treasury_pool.daily_withdrawal_limit = new_limit;

  emit!(DailyLimitChanged {
    admin: ctx.accounts.admin.key(),
    old_limit,
    new_limit,
    changed_at: Clock::get()?.unix_timestamp,
  });

  Ok(())
}
