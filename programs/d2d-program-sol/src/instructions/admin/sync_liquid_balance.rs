use anchor_lang::prelude::*;

use crate::{errors::ErrorCode, states::TreasuryPool};

/// Sync liquid_balance with actual account balance
/// Admin-only instruction to fix liquid_balance when it's out of sync
///
/// This is useful when:
/// - Account balance is higher than liquid_balance (e.g., from direct transfers)
/// - liquid_balance needs to be updated to match actual account balance
#[derive(Accounts)]
pub struct SyncLiquidBalance<'info> {
  /// CHECK: Treasury Pool - manual verification of PDA
  /// We can't use seeds constraint because old accounts may have incorrect bump
  #[account(mut)]
  pub treasury_pool: UncheckedAccount<'info>,

  /// CHECK: Treasury Pool PDA (to get actual account balance)
  #[account(mut)]
  pub treasury_pda: UncheckedAccount<'info>,

  pub admin: Signer<'info>,
}

/// Sync liquid_balance with actual account balance
///
/// This instruction:
/// 1. Gets the actual account balance (lamports) from treasury_pda
/// 2. Calculates rent exemption
/// 3. Updates liquid_balance to match (account_balance - rent_exemption)
///
/// This ensures liquid_balance reflects the actual available SOL in the account
pub fn sync_liquid_balance(ctx: Context<SyncLiquidBalance>) -> Result<()> {
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

  // Check admin authorization
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

  // Available balance = actual balance - rent exemption
  let available_balance = actual_account_balance
    .checked_sub(rent_exemption)
    .ok_or(ErrorCode::CalculationOverflow)?;

  // Update liquid_balance to match available balance
  treasury_pool.liquid_balance = available_balance;

  msg!("[SYNC] Synced liquid_balance with account balance");
  msg!(
    "[SYNC] Account balance: {} lamports",
    actual_account_balance
  );
  msg!("[SYNC] Rent exemption: {} lamports", rent_exemption);
  msg!("[SYNC] Available balance: {} lamports", available_balance);
  msg!(
    "[SYNC] Updated liquid_balance: {} lamports",
    treasury_pool.liquid_balance
  );

  // Serialize updated treasury_pool back to account
  let mut data = treasury_pool_info.try_borrow_mut_data()?;
  treasury_pool.try_serialize(&mut &mut data[..])?;

  Ok(())
}
