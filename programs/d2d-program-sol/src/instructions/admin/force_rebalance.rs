use anchor_lang::prelude::*;

use crate::{errors::ErrorCode, states::TreasuryPool};

/// Force rebalance withdrawal pool with admin check
/// Emergency instruction to fix withdrawal pool
#[derive(Accounts)]
pub struct ForceRebalance<'info> {
  /// CHECK: Treasury Pool - manual verification
  #[account(mut)]
  pub treasury_pool: UncheckedAccount<'info>,

  /// CHECK: Treasury Pool PDA
  #[account(mut)]
  pub treasury_pda: UncheckedAccount<'info>,

  /// Admin signer required for security
  pub admin: Signer<'info>,
}

/// Force rebalance withdrawal pool without admin check
/// This is an EMERGENCY instruction only
pub fn force_rebalance(ctx: Context<ForceRebalance>) -> Result<()> {
  // Verify treasury pool PDA manually
  let (expected_treasury_pool, _bump) =
    Pubkey::find_program_address(&[TreasuryPool::PREFIX_SEED], ctx.program_id);
  require!(
    ctx.accounts.treasury_pool.key() == expected_treasury_pool,
    ErrorCode::InvalidAccountOwner
  );
  require!(
    ctx.accounts.treasury_pda.key() == expected_treasury_pool,
    ErrorCode::InvalidAccountOwner
  );

  // Deserialize treasury pool
  let treasury_pool_info = ctx.accounts.treasury_pool.to_account_info();
  let mut treasury_pool = TreasuryPool::try_deserialize(&mut &treasury_pool_info.data.borrow()[..])
    .map_err(|_| ErrorCode::InvalidAccountData)?;

  let treasury_pda_info = ctx.accounts.treasury_pda.to_account_info();

  // SECURITY: Verify admin authorization
  require!(
    ctx.accounts.admin.key() == treasury_pool.admin,
    ErrorCode::Unauthorized
  );

  require!(!treasury_pool.emergency_pause, ErrorCode::ProgramPaused);

  // Get actual account balance
  let actual_account_balance = treasury_pda_info.lamports();

  // Calculate rent exemption
  let account_data_size = treasury_pda_info.data_len();
  let rent_exemption = Rent::get()?.minimum_balance(account_data_size);

  // Calculate available balance after rent
  let balance_after_rent = actual_account_balance
    .checked_sub(rent_exemption)
    .ok_or(ErrorCode::CalculationOverflow)?;

  // SECURITY FIX H-01: reward_pool and platform_pool are SEPARATE PDAs
  // They are NOT part of treasury_pda lamports, so do NOT subtract them
  // treasury_pda only holds staker deposits (liquid_balance)
  //
  // The old logic incorrectly subtracted reward_pool_balance + platform_pool_balance
  // which are state values for different accounts, not lamports in this PDA

  // Update liquid_balance to match actual available balance
  let new_liquid_balance = balance_after_rent;
  treasury_pool.liquid_balance = new_liquid_balance;

  msg!("[FORCE_REBALANCE] Emergency rebalance executed");
  msg!("[FORCE_REBALANCE] Admin: {}", ctx.accounts.admin.key());
  msg!(
    "[FORCE_REBALANCE] Account balance: {} lamports",
    actual_account_balance
  );
  msg!(
    "[FORCE_REBALANCE] Rent exemption: {} lamports",
    rent_exemption
  );
  msg!(
    "[FORCE_REBALANCE] Updated liquid_balance: {} lamports",
    treasury_pool.liquid_balance
  );

  // Serialize updated treasury_pool back to account
  let mut data = treasury_pool_info.try_borrow_mut_data()?;
  treasury_pool.try_serialize(&mut &mut data[..])?;

  Ok(())
}
