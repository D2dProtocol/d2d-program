use anchor_lang::prelude::*;

use crate::{
  errors::ErrorCode,
  events::StakerWithdrawalQueued,
  states::{BackerDeposit, TreasuryPool, WithdrawalQueueEntry},
};

/// Queue a withdrawal request when liquid_balance is insufficient
/// This creates a queue entry that will be processed when funds become available
/// (e.g., after rent recovery from closed programs)
#[derive(Accounts)]
pub struct QueueWithdrawal<'info> {
  #[account(
        mut,
        seeds = [TreasuryPool::PREFIX_SEED],
        bump = treasury_pool.bump
    )]
  pub treasury_pool: Account<'info, TreasuryPool>,

  #[account(
        mut,
        seeds = [BackerDeposit::PREFIX_SEED, staker.key().as_ref()],
        bump = lender_stake.bump,
        constraint = lender_stake.backer == staker.key() @ ErrorCode::Unauthorized,
        constraint = lender_stake.is_active @ ErrorCode::InsufficientStake,
    )]
  pub lender_stake: Account<'info, BackerDeposit>,

  #[account(
        init,
        payer = staker,
        space = 8 + WithdrawalQueueEntry::INIT_SPACE,
        seeds = [WithdrawalQueueEntry::PREFIX_SEED, &treasury_pool.withdrawal_queue_tail.to_le_bytes()],
        bump
    )]
  pub queue_entry: Account<'info, WithdrawalQueueEntry>,

  #[account(mut)]
  pub staker: Signer<'info>,

  pub system_program: Program<'info, System>,
}

pub fn queue_withdrawal(ctx: Context<QueueWithdrawal>, amount: u64) -> Result<()> {
  let treasury_pool = &mut ctx.accounts.treasury_pool;
  let lender_stake = &mut ctx.accounts.lender_stake;
  let queue_entry = &mut ctx.accounts.queue_entry;
  let current_time = Clock::get()?.unix_timestamp;

  require!(!treasury_pool.emergency_pause, ErrorCode::ProgramPaused);
  require!(amount > 0, ErrorCode::InvalidAmount);
  require!(
    amount <= lender_stake.deposited_amount,
    ErrorCode::InsufficientStake
  );

  // Check if staker already has a queued withdrawal
  require!(
    !lender_stake.has_queued_withdrawal(),
    ErrorCode::WithdrawalAlreadyQueued
  );

  // Get the queue position
  let position = treasury_pool.withdrawal_queue_tail;

  // Initialize queue entry
  queue_entry.position = position;
  queue_entry.staker = ctx.accounts.staker.key();
  queue_entry.amount = amount;
  queue_entry.queued_at = current_time;
  queue_entry.processed = false;
  queue_entry.amount_withdrawn = 0;
  queue_entry.processed_at = 0;
  queue_entry.bump = ctx.bumps.queue_entry;

  // Update lender stake
  lender_stake.queue_withdrawal(amount, position, current_time)?;

  // Update treasury pool queue tracking
  treasury_pool.add_to_withdrawal_queue(amount)?;

  emit!(StakerWithdrawalQueued {
    staker: ctx.accounts.staker.key(),
    amount,
    queue_position: position,
    queued_withdrawal_total: treasury_pool.queued_withdrawal_amount,
    queued_at: current_time,
  });

  Ok(())
}
