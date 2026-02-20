use anchor_lang::{prelude::*, solana_program::rent::Rent, system_program};

use crate::{
  errors::ErrorCode,
  events::{RewardsMovedToPending, SolStaked},
  states::{BackerDeposit, TreasuryPool},
};

#[derive(Accounts)]
pub struct StakeSol<'info> {
  /// CHECK: Treasury Pool PDA - holds both data and SOL
  #[account(mut)]
  pub treasury_pool: UncheckedAccount<'info>,

  #[account(
    init_if_needed,
    payer = lender,
    space = 8 + BackerDeposit::INIT_SPACE,
    seeds = [BackerDeposit::PREFIX_SEED, lender.key().as_ref()],
    bump
  )]
  pub lender_stake: Account<'info, BackerDeposit>,

  #[account(mut)]
  pub lender: Signer<'info>,

  pub system_program: Program<'info, System>,
}

pub fn stake_sol(ctx: Context<StakeSol>, deposit_amount: u64, _lock_period: i64) -> Result<()> {
  let (expected_treasury_pool, _bump) =
    Pubkey::find_program_address(&[TreasuryPool::PREFIX_SEED], ctx.program_id);
  require!(
    ctx.accounts.treasury_pool.key() == expected_treasury_pool,
    ErrorCode::InvalidAccountOwner
  );

  let treasury_pool_info = ctx.accounts.treasury_pool.to_account_info();
  let required_space = 8 + TreasuryPool::INIT_SPACE;
  let current_space = treasury_pool_info.data_len();

  if current_space < required_space {
    treasury_pool_info.resize(required_space)?;
  }

  let mut treasury_pool = TreasuryPool::try_deserialize(&mut &treasury_pool_info.data.borrow()[..])
    .map_err(|_| anchor_lang::error!(ErrorCode::InvalidAccountData))?;

  let lender_stake = &mut ctx.accounts.lender_stake;

  treasury_pool.is_emergency_stop()?;
  require!(deposit_amount > 0, ErrorCode::InvalidAmount);

  let is_new_deposit = lender_stake.backer == Pubkey::default();

  let rent_exemption_needed = if is_new_deposit {
    Rent::get()?.minimum_balance(8 + BackerDeposit::INIT_SPACE)
  } else {
    0
  };

  const TRANSACTION_FEE_ESTIMATE: u64 = 10_000;

  let total_required = deposit_amount
    .checked_add(rent_exemption_needed)
    .and_then(|x| x.checked_add(TRANSACTION_FEE_ESTIMATE))
    .ok_or(ErrorCode::CalculationOverflow)?;

  require!(
    ctx.accounts.lender.lamports() >= total_required,
    ErrorCode::InsufficientDeposit
  );

  let current_time = Clock::get()?.unix_timestamp;

  if is_new_deposit {
    lender_stake.init(ctx.accounts.lender.key(), ctx.bumps.lender_stake, current_time);
  } else {
    if !lender_stake.is_active {
      lender_stake.is_active = true;
    }
    lender_stake.settle_pending_rewards(treasury_pool.reward_per_share)?;

    // Update duration weight for existing staker before adding more
    let weight_delta = lender_stake.update_duration_weight(current_time)?;
    if weight_delta > 0 {
      treasury_pool.update_stake_duration_weight(weight_delta)?;
    }
  }

  // === FIX: FIRST DEPOSITOR ARBITRAGE ===
  // Instead of giving all accumulated rewards to the first depositor,
  // move them to pending_undistributed_rewards for gradual distribution
  let total_deposited_before = treasury_pool.total_deposited;
  if total_deposited_before == 0 && treasury_pool.reward_pool_balance > 0 {
    let excess_rewards = treasury_pool.reward_pool_balance;

    // FIXED: Move excess rewards to pending for gradual distribution
    // This prevents the first depositor from claiming all rewards instantly
    treasury_pool.move_to_pending_rewards(excess_rewards)?;

    // Emit event for transparency
    emit!(RewardsMovedToPending {
      amount: excess_rewards,
      reason: "First depositor protection - rewards moved to pending".to_string(),
      moved_at: current_time,
    });

    // DO NOT update reward_per_share here!
    // Rewards will be distributed gradually via distribute_pending_rewards instruction
  }

  lender_stake.deposited_amount = lender_stake
    .deposited_amount
    .checked_add(deposit_amount)
    .ok_or(ErrorCode::CalculationOverflow)?;

  treasury_pool.total_deposited = treasury_pool
    .total_deposited
    .checked_add(deposit_amount)
    .ok_or(ErrorCode::CalculationOverflow)?;

  treasury_pool.liquid_balance = treasury_pool
    .liquid_balance
    .checked_add(deposit_amount)
    .ok_or(ErrorCode::CalculationOverflow)?;

  let deposit_cpi = CpiContext::new(
    ctx.accounts.system_program.to_account_info(),
    system_program::Transfer {
      from: ctx.accounts.lender.to_account_info(),
      to: ctx.accounts.treasury_pool.to_account_info(),
    },
  );
  system_program::transfer(deposit_cpi, deposit_amount)?;

  lender_stake.update_reward_debt(treasury_pool.reward_per_share)?;

  let mut data = treasury_pool_info.try_borrow_mut_data()?;
  treasury_pool.try_serialize(&mut &mut data[..])?;

  emit!(SolStaked {
    lender: lender_stake.backer,
    amount: deposit_amount,
    total_staked: lender_stake.deposited_amount,
    lock_period: 0,
  });

  emit!(crate::events::DepositMade {
    backer: lender_stake.backer,
    deposit_amount,
    net_deposit: deposit_amount,
    reward_fee: 0,
    platform_fee: 0,
    total_deposited: treasury_pool.total_deposited,
    liquid_balance: treasury_pool.liquid_balance,
    deposited_at: Clock::get()?.unix_timestamp,
  });

  Ok(())
}
