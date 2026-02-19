use anchor_lang::prelude::*;

use crate::{errors::ErrorCode, events::AdminWithdrew, states::TreasuryPool};

#[derive(Accounts)]
pub struct AdminWithdrawRewardPool<'info> {
  #[account(
        mut,
        seeds = [TreasuryPool::PREFIX_SEED],
        bump = treasury_pool.bump
    )]
  pub treasury_pool: Account<'info, TreasuryPool>,

  /// CHECK: Reward Pool PDA
  #[account(
        mut,
        seeds = [TreasuryPool::REWARD_POOL_SEED],
        bump = treasury_pool.reward_pool_bump
    )]
  pub reward_pool: UncheckedAccount<'info>,

  #[account(
        constraint = admin.key() == treasury_pool.admin @ ErrorCode::Unauthorized
    )]
  pub admin: Signer<'info>,

  /// CHECK: Destination wallet
  #[account(mut)]
  pub destination: UncheckedAccount<'info>,

  pub system_program: Program<'info, System>,
}

pub fn admin_withdraw_reward_pool(
  ctx: Context<AdminWithdrawRewardPool>,
  amount: u64,
  reason: String,
) -> Result<()> {
  let treasury_pool = &mut ctx.accounts.treasury_pool;
  let reward_pool_info = ctx.accounts.reward_pool.to_account_info();
  let destination_info = ctx.accounts.destination.to_account_info();

  require!(!treasury_pool.emergency_pause, ErrorCode::ProgramPaused);
  require!(amount > 0, ErrorCode::InvalidAmount);

  require!(
    treasury_pool.reward_pool_balance >= amount,
    ErrorCode::InsufficientTreasuryFunds
  );

  let excess_rewards = treasury_pool.get_excess_rewards();
  require!(
    amount <= excess_rewards,
    ErrorCode::CannotWithdrawProtectedRewards
  );

  require!(
    reward_pool_info.lamports() >= amount,
    ErrorCode::InsufficientTreasuryFunds
  );

  {
    let mut reward_pool_lamports = reward_pool_info.try_borrow_mut_lamports()?;
    let mut destination_lamports = destination_info.try_borrow_mut_lamports()?;

    **reward_pool_lamports = (**reward_pool_lamports)
      .checked_sub(amount)
      .ok_or(ErrorCode::CalculationOverflow)?;
    **destination_lamports = (**destination_lamports)
      .checked_add(amount)
      .ok_or(ErrorCode::CalculationOverflow)?;
  }

  treasury_pool.reward_pool_balance = treasury_pool
    .reward_pool_balance
    .checked_sub(amount)
    .ok_or(ErrorCode::CalculationOverflow)?;

  emit!(AdminWithdrew {
    admin: ctx.accounts.admin.key(),
    amount,
    destination: destination_info.key(),
    reason,
    withdrawn_at: Clock::get()?.unix_timestamp,
  });

  Ok(())
}
