use anchor_lang::prelude::*;

use crate::{
  errors::ErrorCode,
  events::WithdrawalQueueProcessed,
  states::{BackerDeposit, TreasuryPool, WithdrawalQueueEntry},
};

/// Process a single queued withdrawal entry when liquidity is available
/// Called by admin/crank after rent recovery or when liquid_balance increases
/// Processes one entry per call - caller should invoke repeatedly for batch processing
#[derive(Accounts)]
#[instruction(queue_position: u32)]
pub struct ProcessWithdrawalQueue<'info> {
  /// CHECK: Treasury Pool - manual deserialization for migration compatibility
  #[account(
        mut,
        seeds = [TreasuryPool::PREFIX_SEED],
        bump
    )]
  pub treasury_pool: UncheckedAccount<'info>,

  /// CHECK: Treasury Pool PDA (holds deposits) - same PDA, used for lamport transfer
  #[account(
        mut,
        seeds = [TreasuryPool::PREFIX_SEED],
        bump
    )]
  pub treasury_pda: UncheckedAccount<'info>,

  #[account(
        mut,
        seeds = [WithdrawalQueueEntry::PREFIX_SEED, &queue_position.to_le_bytes()],
        bump = queue_entry.bump,
        constraint = !queue_entry.processed @ ErrorCode::WithdrawalAlreadyProcessed,
    )]
  pub queue_entry: Account<'info, WithdrawalQueueEntry>,

  #[account(
        mut,
        seeds = [BackerDeposit::PREFIX_SEED, queue_entry.staker.as_ref()],
        bump = lender_stake.bump,
        constraint = lender_stake.backer == queue_entry.staker @ ErrorCode::Unauthorized,
    )]
  pub lender_stake: Account<'info, BackerDeposit>,

  /// CHECK: Staker receiving the withdrawal - must match queue entry
  #[account(
        mut,
        constraint = staker.key() == queue_entry.staker @ ErrorCode::Unauthorized,
    )]
  pub staker: UncheckedAccount<'info>,

  #[account(mut)]
  pub admin: Signer<'info>,

  pub system_program: Program<'info, System>,
}

pub fn process_withdrawal_queue(
  ctx: Context<ProcessWithdrawalQueue>,
  queue_position: u32,
) -> Result<()> {
  // Verify treasury accounts match
  require!(
    ctx.accounts.treasury_pda.key() == ctx.accounts.treasury_pool.key(),
    ErrorCode::InvalidAccountOwner
  );

  // Deserialize treasury pool manually (migration compatibility)
  let treasury_pool_info = ctx.accounts.treasury_pool.to_account_info();
  let required_space = 8 + TreasuryPool::INIT_SPACE;
  let current_space = treasury_pool_info.data_len();

  if current_space < required_space {
    treasury_pool_info.resize(required_space)?;
  }

  let mut treasury_pool = TreasuryPool::try_deserialize(&mut &treasury_pool_info.data.borrow()[..])
    .map_err(|_| anchor_lang::error!(ErrorCode::InvalidAccountData))?;

  // Admin-only check
  require!(
    treasury_pool.is_admin_or_guardian(&ctx.accounts.admin.key()),
    ErrorCode::Unauthorized
  );

  let treasury_pda_info = ctx.accounts.treasury_pda.to_account_info();
  let queue_entry = &mut ctx.accounts.queue_entry;
  let lender_stake = &mut ctx.accounts.lender_stake;
  let current_time = Clock::get()?.unix_timestamp;

  // Verify queue entry is still pending
  require!(
    queue_entry.is_pending(),
    ErrorCode::WithdrawalAlreadyProcessed
  );

  // Calculate available balance
  let treasury_lamports = treasury_pda_info.lamports();
  let account_data_size = treasury_pda_info.data_len();
  let rent_exemption =
    anchor_lang::solana_program::rent::Rent::get()?.minimum_balance(account_data_size);

  let available_balance = treasury_lamports
    .checked_sub(rent_exemption)
    .ok_or(ErrorCode::CalculationOverflow)?;

  require!(available_balance > 0, ErrorCode::InsufficientLiquidBalance);

  // Process withdrawal (partial or full based on available liquidity)
  let remaining_amount = queue_entry.get_remaining_amount();
  let transfer_amount = available_balance.min(remaining_amount);

  require!(transfer_amount > 0, ErrorCode::InsufficientLiquidBalance);

  // Settle pending rewards before modifying deposit
  lender_stake.settle_pending_rewards(treasury_pool.reward_per_share)?;

  // Update duration weight
  let weight_delta = lender_stake.update_duration_weight(current_time)?;
  if weight_delta > 0 {
    treasury_pool.update_stake_duration_weight(weight_delta)?;
  }

  // Update lender stake - reduce deposited amount
  lender_stake.deposited_amount = lender_stake
    .deposited_amount
    .checked_sub(transfer_amount)
    .ok_or(ErrorCode::CalculationOverflow)?;

  if lender_stake.deposited_amount == 0 {
    lender_stake.is_active = false;
    lender_stake.reward_debt = 0;
  } else {
    lender_stake.update_reward_debt(treasury_pool.reward_per_share)?;
  }

  // Process the queue entry
  let processed_amount = queue_entry.process_withdrawal(transfer_amount, current_time);

  // Update lender stake queue tracking
  lender_stake.process_queued_withdrawal(processed_amount)?;

  // Update treasury pool
  treasury_pool.total_deposited = treasury_pool
    .total_deposited
    .checked_sub(transfer_amount)
    .ok_or(ErrorCode::CalculationOverflow)?;

  treasury_pool.liquid_balance = treasury_pool
    .liquid_balance
    .checked_sub(transfer_amount)
    .ok_or(ErrorCode::CalculationOverflow)?;

  treasury_pool.process_queued_withdrawal(processed_amount)?;

  // Advance queue head if this entry is fully processed
  if queue_entry.processed && queue_position == treasury_pool.withdrawal_queue_head {
    treasury_pool.withdrawal_queue_head = treasury_pool
      .withdrawal_queue_head
      .checked_add(1)
      .ok_or(ErrorCode::CalculationOverflow)?;
  }

  // Transfer SOL from treasury PDA to staker
  {
    let staker_info = ctx.accounts.staker.to_account_info();
    let mut treasury_lamports = treasury_pda_info.try_borrow_mut_lamports()?;
    let mut staker_lamports = staker_info.try_borrow_mut_lamports()?;

    **treasury_lamports = (**treasury_lamports)
      .checked_sub(transfer_amount)
      .ok_or(ErrorCode::CalculationOverflow)?;
    **staker_lamports = (**staker_lamports)
      .checked_add(transfer_amount)
      .ok_or(ErrorCode::CalculationOverflow)?;
  }

  // Serialize treasury pool back
  let mut data = treasury_pool_info.try_borrow_mut_data()?;
  treasury_pool.try_serialize(&mut &mut data[..])?;

  emit!(WithdrawalQueueProcessed {
    entries_processed: 1,
    total_amount: transfer_amount,
    remaining_queued: treasury_pool.queued_withdrawal_amount,
    processed_at: current_time,
  });

  Ok(())
}
