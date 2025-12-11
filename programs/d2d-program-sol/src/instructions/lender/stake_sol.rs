use crate::errors::ErrorCode;
use crate::events::SolStaked;
use crate::states::{BackerDeposit, TreasuryPool};
use anchor_lang::prelude::*;
use anchor_lang::system_program;
use anchor_lang::solana_program::rent::Rent;

/// Deposit SOL into the program (reward-per-share model)
/// 
/// Flow:
/// 1. Settle pending rewards (update reward_debt)
/// 2. Calculate fees: 1% reward, 0.1% platform
/// 3. Transfer net deposit to Treasury PDA
/// 4. Transfer fees to respective pools
/// 5. Update total_deposited and liquid_balance
/// 6. Update backer's deposited_amount and reward_debt
#[derive(Accounts)]
pub struct StakeSol<'info> {
    /// CHECK: Treasury Pool - will be migrated if needed
    /// We use UncheckedAccount to handle old layout migration
    /// Note: We can't use Account constraint because old layout can't deserialize
    /// We verify the PDA manually in the function
    #[account(mut)]
    pub treasury_pool: UncheckedAccount<'info>,
    
    /// CHECK: Treasury Pool PDA (receives 100% of deposit)
    /// Same as treasury_pool, just for lamport transfers
    #[account(mut)]
    pub treasury_pda: UncheckedAccount<'info>,
    
    /// CHECK: Lender stake account - will be initialized/resized if needed
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

/// Deposit SOL (reward-per-share model)
/// 
/// Before updating deposited_amount, settle pending rewards by updating reward_debt
pub fn stake_sol(ctx: Context<StakeSol>, deposit_amount: u64, _lock_period: i64) -> Result<()> {
    msg!("[STAKE] Starting stake_sol instruction");
    msg!("[STAKE] Deposit amount: {} lamports", deposit_amount);
    
    // Verify treasury pool PDA matches
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
    
    // Handle migration if needed
    let treasury_pool_info = ctx.accounts.treasury_pool.to_account_info();
    let required_space = 8 + TreasuryPool::INIT_SPACE;
    let current_space = treasury_pool_info.data_len();
    
    // Check if account needs migration (resize)
    if current_space < required_space {
        msg!("[STAKE] Account needs resize: {} < {} bytes", current_space, required_space);
        // Resize account - this will preserve existing data
        treasury_pool_info.realloc(required_space, false)?;
    }
    
    // Try to deserialize treasury pool
    // If deserialization fails, it means account has old layout - need admin migration first
    let mut treasury_pool = TreasuryPool::try_deserialize(&mut &treasury_pool_info.data.borrow()[..])
        .map_err(|_| {
            msg!("[STAKE] ERROR: Cannot deserialize TreasuryPool account");
            msg!("[STAKE] Account size: {} bytes, required: {} bytes", current_space, required_space);
            msg!("[STAKE] Please call migrate_treasury_pool() instruction first");
            anchor_lang::error!(crate::errors::ErrorCode::InvalidAccountData)
        })?;
    
    let lender_stake = &mut ctx.accounts.lender_stake;

    msg!("[STAKE] Treasury Pool loaded - reward_per_share: {}, total_deposited: {}", 
         treasury_pool.reward_per_share, treasury_pool.total_deposited);
    msg!("[STAKE] Lender: {}", ctx.accounts.lender.key());

    require!(!treasury_pool.emergency_pause, ErrorCode::ProgramPaused);
    require!(deposit_amount > 0, ErrorCode::InvalidAmount);

    // Check lender has sufficient lamports
    // Need to account for:
    // 1. deposit_amount (the amount to stake)
    // 2. Transaction fees (~5000 lamports)
    // 3. Rent exemption for lender_stake account if it's new (~1.4M lamports)
    let lender_lamports = ctx.accounts.lender.lamports();
    msg!("[STAKE] Lender balance: {} lamports", lender_lamports);
    msg!("[STAKE] Deposit amount: {} lamports", deposit_amount);
    
    // Check if lender_stake account is new (needs rent exemption)
    let is_new_account = lender_stake.backer == Pubkey::default();
    msg!("[STAKE] Is new account: {}", is_new_account);
    
    let rent_exemption_needed = if is_new_account {
        // Rent exemption for BackerDeposit account (8 + INIT_SPACE)
        let rent = Rent::get()?;
        let min_balance = rent.minimum_balance(8 + BackerDeposit::INIT_SPACE);
        msg!("[STAKE] Rent exemption needed: {} lamports", min_balance);
        min_balance
    } else {
        msg!("[STAKE] Rent exemption needed: 0 (existing account)");
        0
    };
    
    // Transaction fee estimate (~5000 lamports, but we use 10000 for safety)
    const TRANSACTION_FEE_ESTIMATE: u64 = 10_000;
    
    let total_required = deposit_amount
        .checked_add(rent_exemption_needed)
        .and_then(|x| x.checked_add(TRANSACTION_FEE_ESTIMATE))
        .ok_or(ErrorCode::CalculationOverflow)?;
    
    msg!("[STAKE] Total required: {} lamports (deposit: {} + rent: {} + fee: {})", 
         total_required, deposit_amount, rent_exemption_needed, TRANSACTION_FEE_ESTIMATE);
    msg!("[STAKE] Available: {} lamports", lender_lamports);
    
    require!(
        lender_lamports >= total_required,
        ErrorCode::InsufficientDeposit
    );

    // Initialize backer deposit if first time (init_if_needed handles this)
    let is_new_deposit = lender_stake.backer == Pubkey::default();

    if is_new_deposit {
        // Initialize new deposit
        lender_stake.backer = ctx.accounts.lender.key();
        lender_stake.deposited_amount = 0;
        lender_stake.reward_debt = 0;
        lender_stake.pending_rewards = 0;
        lender_stake.claimed_total = 0;
        lender_stake.is_active = true;
        lender_stake.bump = ctx.bumps.lender_stake;
    } else {
        // If account exists but is inactive (e.g., after full unstake), reactivate it
        // This allows users to stake again after unstaking all their SOL
        if !lender_stake.is_active {
            msg!("[STAKE] Reactivating inactive stake account");
            lender_stake.is_active = true;
        }

        // CRITICAL: Settle pending rewards before adding new deposit
        // This preserves rewards that would be lost when reward_debt is recalculated
        msg!("[STAKE] Settling pending rewards before adding new deposit");
        lender_stake.settle_pending_rewards(treasury_pool.reward_per_share)?;
        msg!("[STAKE] Pending rewards after settle: {} lamports", lender_stake.pending_rewards);
    }

    // NO FEES TAKEN FROM BACKER - 100% goes to TreasuryPool
    // Fees come from developers when they pay for deployments (borrowed_amount * 1% monthly)

    // Handle excess rewards: If fees were credited before any deposits,
    // we need to distribute those excess rewards proportionally to all backers
    // This ensures backers receive 1-1.2% returns when their SOL is fully utilized
    let total_deposited_before = treasury_pool.total_deposited;
    if total_deposited_before == 0 && treasury_pool.reward_pool_balance > 0 {
        // There are excess rewards (fees credited before any deposits)
        // Distribute them proportionally based on the new total deposits after this stake
        let excess_rewards = treasury_pool.reward_pool_balance;
        let new_total_deposited = deposit_amount;
        
        // reward_per_share = excess_rewards * PRECISION / new_total_deposited
        // This ensures the first backer(s) receive excess rewards proportionally
        let excess_reward_per_share = (excess_rewards as u128)
            .checked_mul(TreasuryPool::PRECISION)
            .ok_or(ErrorCode::CalculationOverflow)?
            .checked_div(new_total_deposited as u128)
            .ok_or(ErrorCode::CalculationOverflow)?;
        
        msg!("[STAKE] Excess rewards detected: {} lamports", excess_rewards);
        msg!("[STAKE] Calculating reward_per_share from excess: {}", excess_reward_per_share);
        msg!("[STAKE] New total deposited: {} lamports", new_total_deposited);
        
        // Add excess reward_per_share to current reward_per_share
        treasury_pool.reward_per_share = treasury_pool
            .reward_per_share
            .checked_add(excess_reward_per_share)
            .ok_or(ErrorCode::CalculationOverflow)?;
        
        msg!("[STAKE] Updated reward_per_share to: {}", treasury_pool.reward_per_share);
    } else if total_deposited_before > 0 && treasury_pool.reward_pool_balance > 0 {
        // Check if there are still excess rewards (reward_pool_balance > total claimable)
        // This can happen if fees were credited when total_deposited was lower
        // For now, we let the normal credit_fee_to_pool logic handle this
        // Future deposits will benefit from accumulated reward_per_share
    }

    // Update deposit amount (100% of deposit_amount)
    lender_stake.deposited_amount = lender_stake
        .deposited_amount
        .checked_add(deposit_amount)
        .ok_or(ErrorCode::CalculationOverflow)?;

    // Update treasury pool state
    treasury_pool.total_deposited = treasury_pool
        .total_deposited
        .checked_add(deposit_amount)
        .ok_or(ErrorCode::CalculationOverflow)?;
    
    treasury_pool.liquid_balance = treasury_pool
        .liquid_balance
        .checked_add(deposit_amount)
        .ok_or(ErrorCode::CalculationOverflow)?;

    // Transfer 100% of deposit to Treasury PDA
    let deposit_cpi = CpiContext::new(
        ctx.accounts.system_program.to_account_info(),
        system_program::Transfer {
            from: ctx.accounts.lender.to_account_info(),
            to: ctx.accounts.treasury_pda.to_account_info(),
        },
    );
    system_program::transfer(deposit_cpi, deposit_amount)?;

    // Update reward_debt after deposit
    // This captures the current reward_per_share for the new total deposited_amount
    // pending_rewards already settled above (if not new deposit), safe to update debt
    lender_stake.update_reward_debt(treasury_pool.reward_per_share)?;

    // Serialize updated treasury_pool back to account
    let mut data = treasury_pool_info.try_borrow_mut_data()?;
    treasury_pool.try_serialize(&mut &mut data[..])?;

    emit!(SolStaked {
        lender: lender_stake.backer,
        amount: deposit_amount, // 100% of deposit (no fees)
        total_staked: lender_stake.deposited_amount,
        lock_period: 0, // Not used in new model
    });
    
    // Emit detailed deposit event
    emit!(crate::events::DepositMade {
        backer: lender_stake.backer,
        deposit_amount,
        net_deposit: deposit_amount, // No fees deducted
        reward_fee: 0, // No fees from backer
        platform_fee: 0, // No fees from backer
        total_deposited: treasury_pool.total_deposited,
        liquid_balance: treasury_pool.liquid_balance,
        deposited_at: Clock::get()?.unix_timestamp,
    });

    Ok(())
}
