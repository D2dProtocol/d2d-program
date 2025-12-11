use crate::errors::ErrorCode;
use crate::states::TreasuryPool;
use anchor_lang::prelude::*;

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
    let (expected_treasury_pool, _bump) = Pubkey::find_program_address(
        &[TreasuryPool::PREFIX_SEED],
        ctx.program_id,
    );
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

    // Account for other pools (reward_pool and platform_pool are separate)
    let other_pools = treasury_pool
        .reward_pool_balance
        .checked_add(treasury_pool.platform_pool_balance)
        .ok_or(ErrorCode::CalculationOverflow)?;

    // Available for liquid_balance (shared between deployments and withdrawals)
    let new_liquid_balance = balance_after_rent
        .checked_sub(other_pools)
        .ok_or(ErrorCode::CalculationOverflow)?;

    // Update liquid_balance
    treasury_pool.liquid_balance = new_liquid_balance;

    msg!("[FORCE_REBALANCE] Emergency rebalance executed");
    msg!("[FORCE_REBALANCE] Admin: {}", ctx.accounts.admin.key());
    msg!("[FORCE_REBALANCE] Account balance: {} lamports", actual_account_balance);
    msg!("[FORCE_REBALANCE] Rent exemption: {} lamports", rent_exemption);
    msg!("[FORCE_REBALANCE] Updated liquid_balance: {} lamports", treasury_pool.liquid_balance);

    // Serialize updated treasury_pool back to account
    let mut data = treasury_pool_info.try_borrow_mut_data()?;
    treasury_pool.try_serialize(&mut &mut data[..])?;

    Ok(())
}