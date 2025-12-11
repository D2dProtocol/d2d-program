use crate::errors::ErrorCode;
use crate::events::SolUnstaked;
use crate::states::{BackerDeposit, TreasuryPool};
use anchor_lang::prelude::*;
use anchor_lang::system_program;

/// Unstake SOL (withdraw deposit)
/// 
/// Reward-per-share model:
/// - If liquid_balance >= amount: withdraw immediately
/// - Else: create withdraw_request (to be implemented)
#[derive(Accounts)]
pub struct UnstakeSol<'info> {
    /// CHECK: Treasury Pool - will be migrated if needed
    /// We use UncheckedAccount to handle old layout migration
    /// Note: We can't use Account constraint because old layout can't deserialize
    /// We add seeds constraint so Anchor can resolve PDA, but don't deserialize
    #[account(
        mut,
        seeds = [TreasuryPool::PREFIX_SEED],
        bump
    )]
    pub treasury_pool: UncheckedAccount<'info>,
    
    /// CHECK: Treasury Pool PDA (holds deposits)
    /// Same as treasury_pool, just for lamport transfers
    #[account(
        mut,
        seeds = [TreasuryPool::PREFIX_SEED],
        bump
    )]
    pub treasury_pda: UncheckedAccount<'info>,
    
    #[account(
        mut,
        seeds = [BackerDeposit::PREFIX_SEED, lender.key().as_ref()],
        bump = lender_stake.bump
    )]
    pub lender_stake: Account<'info, BackerDeposit>,
    
    #[account(mut)]
    pub lender: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

/// Unstake SOL (withdraw principal)
/// 
/// Withdraws directly from liquid_balance based on deposited_amount in BackerDeposit
pub fn unstake_sol(ctx: Context<UnstakeSol>, amount: u64) -> Result<()> {
    
    // Verify treasury_pda is the same as treasury_pool
    require!(
        ctx.accounts.treasury_pda.key() == ctx.accounts.treasury_pool.key(),
        ErrorCode::InvalidAccountOwner
    );
    
    // Handle migration if needed
    let treasury_pool_info = ctx.accounts.treasury_pool.to_account_info();
    let required_space = 8 + TreasuryPool::INIT_SPACE;
    let current_space = treasury_pool_info.data_len();
    
    // Check if account needs migration (resize)
    if current_space < required_space {
        msg!("[UNSTAKE] Account needs resize: {} < {} bytes", current_space, required_space);
        // Resize account - this will preserve existing data
        treasury_pool_info.realloc(required_space, false)?;
        // Note: realloc is deprecated but resize() requires different signature
        // This works correctly for our use case
    }
    
    // Try to deserialize treasury pool
    // If deserialization fails, it means account has old layout - need admin migration first
    let mut treasury_pool = TreasuryPool::try_deserialize(&mut &treasury_pool_info.data.borrow()[..])
        .map_err(|_| {
            msg!("[UNSTAKE] ERROR: Cannot deserialize TreasuryPool account");
            msg!("[UNSTAKE] Account size: {} bytes, required: {} bytes", current_space, required_space);
            msg!("[UNSTAKE] Please call migrate_treasury_pool() instruction first");
            anchor_lang::error!(crate::errors::ErrorCode::InvalidAccountData)
        })?;
    
    // Get account info and bump before mutable borrows
    let treasury_pda_info = ctx.accounts.treasury_pda.to_account_info();
    
    let lender_stake = &mut ctx.accounts.lender_stake;

    require!(!treasury_pool.emergency_pause, ErrorCode::ProgramPaused);
    require!(amount > 0, ErrorCode::InvalidAmount);
    require!(
        amount <= lender_stake.deposited_amount,
        ErrorCode::InsufficientStake
    );

    // Allow unstake if deposited_amount > 0, even if is_active = false
    // This handles cases where is_active was incorrectly set to false
    // If user has deposited_amount > 0, they should be able to withdraw
    if lender_stake.deposited_amount == 0 {
        return Err(ErrorCode::InsufficientStake.into());
    }

    // CRITICAL: Settle pending rewards BEFORE updating deposited_amount
    // This preserves rewards that would be lost when reward_debt is recalculated
    msg!("[UNSTAKE] Settling pending rewards before unstake");
    lender_stake.settle_pending_rewards(treasury_pool.reward_per_share)?;
    msg!("[UNSTAKE] Pending rewards after settle: {} lamports", lender_stake.pending_rewards);

    // Get actual account balance (source of truth)
    let treasury_lamports = treasury_pda_info.lamports();
    
    // Calculate rent exemption
    let account_data_size = treasury_pda_info.data_len();
    let rent_exemption = anchor_lang::solana_program::rent::Rent::get()?
        .minimum_balance(account_data_size);
    
    // Available balance = actual balance - rent exemption
    let available_balance = treasury_lamports
        .checked_sub(rent_exemption)
        .ok_or(ErrorCode::CalculationOverflow)?;
    
    msg!("[UNSTAKE] Treasury PDA balance: {} lamports", treasury_lamports);
    msg!("[UNSTAKE] Rent exemption: {} lamports", rent_exemption);
    msg!("[UNSTAKE] Available balance: {} lamports", available_balance);
    msg!("[UNSTAKE] liquid_balance (from struct): {} lamports", treasury_pool.liquid_balance);
    
    // Check if available balance is sufficient for withdrawal
    // Use actual account balance as source of truth (may be out of sync with liquid_balance)
    if available_balance < amount {
        msg!("[UNSTAKE] ERROR: Insufficient available balance. Available: {} lamports, Requested: {} lamports", available_balance, amount);
        return Err(ErrorCode::InsufficientLiquidBalance.into());
    }
    
    // Sync liquid_balance with actual account balance if they differ significantly
    // This handles cases where liquid_balance is out of sync after deployments
    let balance_diff = available_balance.abs_diff(treasury_pool.liquid_balance);
    if balance_diff > 1_000_000 { // More than 0.001 SOL difference
        msg!("[UNSTAKE] WARNING: liquid_balance out of sync. Syncing to actual balance...");
        msg!("[UNSTAKE]   liquid_balance (old): {} lamports", treasury_pool.liquid_balance);
        msg!("[UNSTAKE]   available_balance (new): {} lamports", available_balance);
        treasury_pool.liquid_balance = available_balance;
    }

    // Update backer deposit
    lender_stake.deposited_amount = lender_stake
        .deposited_amount
        .checked_sub(amount)
        .ok_or(ErrorCode::CalculationOverflow)?;

    // If fully withdrawn, deactivate
    if lender_stake.deposited_amount == 0 {
        lender_stake.is_active = false;
        lender_stake.reward_debt = 0;
        // Keep pending_rewards intact - user can still claim them later
        msg!("[UNSTAKE] Fully withdrawn. Pending rewards preserved: {} lamports", lender_stake.pending_rewards);
    } else {
        // If there's remaining deposit, ensure is_active = true
        // This reactivates accounts that were incorrectly marked as inactive
        lender_stake.is_active = true;
        // Update reward_debt for remaining deposit
        // pending_rewards already settled above, safe to update debt
        lender_stake.update_reward_debt(treasury_pool.reward_per_share)?;
    }

    // Update treasury pool state
    treasury_pool.total_deposited = treasury_pool
        .total_deposited
        .checked_sub(amount)
        .ok_or(ErrorCode::CalculationOverflow)?;
    
    // Deduct from liquid_balance (shared between deployments and withdrawals)
    treasury_pool.liquid_balance = treasury_pool
        .liquid_balance
        .checked_sub(amount)
        .ok_or(ErrorCode::CalculationOverflow)?;

    // Transfer principal from Treasury PDA -> lender via lamport mutation
    // CRITICAL: Use lamport mutation for program-owned accounts (not CPI System transfer)
    // Treasury PDA has data (TreasuryPool struct), so we cannot use System Program transfer
    {
        let lender_info = ctx.accounts.lender.to_account_info();
        let mut treasury_lamports = treasury_pda_info.try_borrow_mut_lamports()?;
        let mut lender_lamports = lender_info.try_borrow_mut_lamports()?;

        let new_treasury_balance = (**treasury_lamports)
            .checked_sub(amount)
            .ok_or(ErrorCode::CalculationOverflow)?;
        let new_lender_balance = (**lender_lamports)
            .checked_add(amount)
            .ok_or(ErrorCode::CalculationOverflow)?;

        **treasury_lamports = new_treasury_balance;
        **lender_lamports = new_lender_balance;
    }
    
    // Serialize updated treasury_pool back to account
    let mut data = treasury_pool_info.try_borrow_mut_data()?;
    treasury_pool.try_serialize(&mut &mut data[..])?;

    emit!(SolUnstaked {
        lender: lender_stake.backer,
        amount, // Only principal, no rewards
        remaining_staked: lender_stake.deposited_amount,
    });

    Ok(())
}
