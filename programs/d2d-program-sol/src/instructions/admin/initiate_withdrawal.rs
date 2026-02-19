use anchor_lang::prelude::*;

use crate::{
  errors::ErrorCode,
  events::WithdrawalInitiated,
  states::{PendingWithdrawal, TreasuryPool, WithdrawalType},
};

#[derive(Accounts)]
#[instruction(withdrawal_type: WithdrawalType, amount: u64)]
pub struct InitiateWithdrawal<'info> {
  #[account(
        mut,
        seeds = [TreasuryPool::PREFIX_SEED],
        bump = treasury_pool.bump
    )]
  pub treasury_pool: Account<'info, TreasuryPool>,

  #[account(
        init,
        payer = admin,
        space = 8 + PendingWithdrawal::INIT_SPACE,
        seeds = [PendingWithdrawal::PREFIX_SEED, treasury_pool.key().as_ref()],
        bump
    )]
  pub pending_withdrawal: Account<'info, PendingWithdrawal>,

  #[account(
        mut,
        constraint = admin.key() == treasury_pool.admin @ ErrorCode::Unauthorized
    )]
  pub admin: Signer<'info>,

  pub system_program: Program<'info, System>,
}

pub fn initiate_withdrawal(
  ctx: Context<InitiateWithdrawal>,
  withdrawal_type: WithdrawalType,
  amount: u64,
  destination: Pubkey,
  reason: String,
) -> Result<()> {
  let treasury_pool = &mut ctx.accounts.treasury_pool;
  let pending_withdrawal = &mut ctx.accounts.pending_withdrawal;

  require!(!treasury_pool.emergency_pause, ErrorCode::ProgramPaused);
  require!(amount > 0, ErrorCode::InvalidAmount);
  require!(
    treasury_pool.pending_withdrawal_count == 0,
    ErrorCode::PendingWithdrawalExists
  );

  match withdrawal_type {
    WithdrawalType::PlatformPool => {
      require!(
        treasury_pool.platform_pool_balance >= amount,
        ErrorCode::InsufficientTreasuryFunds
      );
    }
    WithdrawalType::RewardPool => {
      require!(
        treasury_pool.reward_pool_balance >= amount,
        ErrorCode::InsufficientTreasuryFunds
      );
    }
  }

  let current_time = Clock::get()?.unix_timestamp;
  let remaining_allowance = treasury_pool.get_remaining_daily_allowance(current_time);
  if treasury_pool.daily_withdrawal_limit > 0 {
    require!(
      amount <= remaining_allowance,
      ErrorCode::DailyWithdrawalLimitExceeded
    );
  }

  let execute_after = current_time
    .checked_add(treasury_pool.timelock_duration)
    .ok_or(ErrorCode::CalculationOverflow)?;
  let expires_at = execute_after
    .checked_add(PendingWithdrawal::VALIDITY_PERIOD)
    .ok_or(ErrorCode::CalculationOverflow)?;

  pending_withdrawal.withdrawal_type = withdrawal_type.clone();
  pending_withdrawal.amount = amount;
  pending_withdrawal.destination = destination;
  pending_withdrawal.initiator = ctx.accounts.admin.key();
  pending_withdrawal.initiated_at = current_time;
  pending_withdrawal.execute_after = execute_after;
  pending_withdrawal.expires_at = expires_at;
  pending_withdrawal.reason = reason.clone();
  pending_withdrawal.executed = false;
  pending_withdrawal.vetoed = false;
  pending_withdrawal.bump = ctx.bumps.pending_withdrawal;

  treasury_pool.pending_withdrawal_count = 1;

  let withdrawal_type_str = match withdrawal_type {
    WithdrawalType::PlatformPool => "PlatformPool",
    WithdrawalType::RewardPool => "RewardPool",
  };

  emit!(WithdrawalInitiated {
    initiator: ctx.accounts.admin.key(),
    withdrawal_type: withdrawal_type_str.to_string(),
    amount,
    destination,
    execute_after,
    expires_at,
    reason,
    initiated_at: current_time,
  });

  Ok(())
}
