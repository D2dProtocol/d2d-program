use anchor_lang::prelude::*;

use crate::{
  errors::ErrorCode,
  events::StakerWithdrawalCancelled,
  states::{BackerDeposit, TreasuryPool, WithdrawalQueueEntry},
};

/// Cancel a queued withdrawal request
/// This allows a staker to cancel their pending withdrawal and keep funds staked
#[derive(Accounts)]
pub struct CancelQueuedWithdrawal<'info> {
  #[account(
        mut,
        seeds = [TreasuryPool::PREFIX_SEED],
        bump = treasury_pool.bump
    )]
  pub treasury_pool: Account<'info, TreasuryPool>,

  #[account(
        mut,
        seeds = [WithdrawalQueueEntry::PREFIX_SEED, &lender_stake.queue_position.to_le_bytes()],
        bump = queue_entry.bump,
        constraint = queue_entry.staker == staker.key() @ ErrorCode::Unauthorized,
        constraint = !queue_entry.processed @ ErrorCode::WithdrawalAlreadyProcessed,
    )]
  pub queue_entry: Account<'info, WithdrawalQueueEntry>,

  #[account(
        mut,
        seeds = [BackerDeposit::PREFIX_SEED, staker.key().as_ref()],
        bump = lender_stake.bump,
        constraint = lender_stake.backer == staker.key() @ ErrorCode::Unauthorized,
        constraint = lender_stake.has_queued_withdrawal() @ ErrorCode::NoQueuedWithdrawal,
    )]
  pub lender_stake: Account<'info, BackerDeposit>,

  #[account(mut)]
  pub staker: Signer<'info>,
}

pub fn cancel_queued_withdrawal(ctx: Context<CancelQueuedWithdrawal>) -> Result<()> {
  let treasury_pool = &mut ctx.accounts.treasury_pool;
  let queue_entry = &mut ctx.accounts.queue_entry;
  let lender_stake = &mut ctx.accounts.lender_stake;
  let current_time = Clock::get()?.unix_timestamp;

  // Calculate remaining amount to cancel
  let amount_to_cancel = queue_entry.get_remaining_amount();

  // Update treasury pool queue tracking
  treasury_pool.process_queued_withdrawal(amount_to_cancel)?;

  // Mark queue entry as processed (cancelled)
  queue_entry.cancel(current_time);

  // Update lender stake - cancel the queued withdrawal
  let cancelled_amount = lender_stake.cancel_queued_withdrawal()?;

  emit!(StakerWithdrawalCancelled {
    staker: ctx.accounts.staker.key(),
    amount_cancelled: cancelled_amount,
    cancelled_at: current_time,
  });

  Ok(())
}
