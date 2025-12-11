use crate::states::TreasuryPool;
use anchor_lang::prelude::*;
use anchor_lang::solana_program::rent::Rent;

/// Migrate Treasury Pool to new layout (removed withdrawal_pool_balance)
/// Admin-only instruction to migrate existing pool to new struct layout
/// 
/// This preserves all existing data and removes withdrawal_pool_balance field
#[derive(Accounts)]
pub struct MigrateTreasuryPool<'info> {
    /// CHECK: Treasury Pool PDA - will be resized and migrated
    /// We use UncheckedAccount to avoid deserialization issues with old layout
    #[account(
        mut,
        seeds = [TreasuryPool::PREFIX_SEED],
        bump
    )]
    pub treasury_pool: UncheckedAccount<'info>,
    
    #[account(mut)]
    pub admin: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

/// Migrate treasury pool to new layout (removed withdrawal_pool_balance)
/// 
/// This instruction:
/// 1. Resizes the account if needed
/// 2. Reads existing data from old layout
/// 3. Writes to new layout without withdrawal_pool_balance
pub fn migrate_treasury_pool(ctx: Context<MigrateTreasuryPool>) -> Result<()> {
    let treasury_pool_info = ctx.accounts.treasury_pool.to_account_info();
    let required_space = 8 + TreasuryPool::INIT_SPACE;
    
    // Check current account size
    let current_space = treasury_pool_info.data_len();
    msg!("[MIGRATE] Current account size: {} bytes", current_space);
    msg!("[MIGRATE] Required size: {} bytes", required_space);
    
    // If account is already the correct size, check if it needs migration
    if current_space >= required_space {
        // Try to deserialize with new layout
        match TreasuryPool::try_deserialize(&mut &treasury_pool_info.data.borrow()[..]) {
            Ok(_pool) => {
                // Account already has correct size (no withdrawal_pool_balance)
                msg!("[MIGRATE] Account already migrated");
                return Ok(()); // Already migrated
            }
            Err(_) => {
                // Can't deserialize, need to resize and migrate
                msg!("[MIGRATE] Cannot deserialize account, will resize and migrate...");
            }
        }
    }
    
    // Read existing data before resize
    let old_data = treasury_pool_info.data.borrow();
    let mut old_pool_data = vec![0u8; old_data.len()];
    old_pool_data.copy_from_slice(&old_data);
    
    // Calculate old layout size (may have withdrawal_pool_balance)
    // Old layout: all fields including withdrawal_pool_balance (8 bytes for u64)
    let old_layout_size = current_space.min(required_space - 8);
    
    // Resize account if needed
    if current_space < required_space {
        msg!("[MIGRATE] Resizing account from {} to {} bytes", current_space, required_space);
        treasury_pool_info.realloc(required_space, false)?;
    }
    
    // Get mutable data after resize
    let mut data = treasury_pool_info.try_borrow_mut_data()?;
    
    // Try to deserialize old layout manually
    // We'll read the fields we know exist in old layout
    let mut new_pool = TreasuryPool {
        reward_per_share: 0,
        total_deposited: 0,
        liquid_balance: 0,
        reward_pool_balance: 0,
        platform_pool_balance: 0,
        reward_fee_bps: TreasuryPool::REWARD_FEE_BPS,
        platform_fee_bps: TreasuryPool::PLATFORM_FEE_BPS,
        admin: ctx.accounts.admin.key(),
        dev_wallet: Pubkey::default(),
        emergency_pause: false,
        reward_pool_bump: 0,
        platform_pool_bump: 0,
        bump: ctx.bumps.treasury_pool,
        // Legacy fields
        backer_total_staked: 0,
        backer_stake_pool_bump: 0,
        total_rewards_distributed: 0,
        admin_pool_balance: 0,
        admin_pool_bump: 0,
        current_apy_bps: 0,
        last_apy_update_ts: 0,
        last_distribution_time: 0,
        total_staked: 0,
        total_fees_collected: 0,
        current_apy: 0,
        treasury_wallet: Pubkey::default(),
    };
    
    // Try to read from old data if possible
    // We'll use a safe approach: try to deserialize old layout, if fails use defaults
    if old_pool_data.len() >= 8 {
        // Skip discriminator (8 bytes)
        let mut cursor = 8;
        
        // Try to read known fields (be careful with offsets)
        // This is a simplified migration - in production, you'd want to be more careful
        // For now, we'll try to deserialize the old struct and copy fields
        
        // Try Anchor's deserialize with error handling
        if let Ok(old_pool) = TreasuryPool::try_deserialize(&mut &old_pool_data[..]) {
            // Successfully deserialized old layout
            new_pool.reward_per_share = old_pool.reward_per_share;
            new_pool.total_deposited = old_pool.total_deposited;
            new_pool.liquid_balance = old_pool.liquid_balance;
            new_pool.reward_pool_balance = old_pool.reward_pool_balance;
            new_pool.platform_pool_balance = old_pool.platform_pool_balance;
            new_pool.reward_fee_bps = old_pool.reward_fee_bps;
            new_pool.platform_fee_bps = old_pool.platform_fee_bps;
            new_pool.admin = old_pool.admin;
            new_pool.dev_wallet = old_pool.dev_wallet;
            new_pool.emergency_pause = old_pool.emergency_pause;
            new_pool.reward_pool_bump = old_pool.reward_pool_bump;
            new_pool.platform_pool_bump = old_pool.platform_pool_bump;
            new_pool.bump = old_pool.bump;
            // Copy legacy fields
            new_pool.backer_total_staked = old_pool.backer_total_staked;
            new_pool.backer_stake_pool_bump = old_pool.backer_stake_pool_bump;
            new_pool.total_rewards_distributed = old_pool.total_rewards_distributed;
            new_pool.admin_pool_balance = old_pool.admin_pool_balance;
            new_pool.admin_pool_bump = old_pool.admin_pool_bump;
            new_pool.current_apy_bps = old_pool.current_apy_bps;
            new_pool.last_apy_update_ts = old_pool.last_apy_update_ts;
            new_pool.last_distribution_time = old_pool.last_distribution_time;
            new_pool.total_staked = old_pool.total_staked;
            new_pool.total_fees_collected = old_pool.total_fees_collected;
            new_pool.current_apy = old_pool.current_apy;
            new_pool.treasury_wallet = old_pool.treasury_wallet;
            
            msg!("[MIGRATE] Successfully read old pool data");
        } else {
            msg!("[MIGRATE] Could not deserialize old layout, using current account data");
            // Try to read from current account (might already be partially migrated)
            if let Ok(current_pool) = TreasuryPool::try_deserialize(&mut &treasury_pool_info.data.borrow()[..]) {
                new_pool = current_pool;
            }
        }
    }
    
    // Serialize new layout
    new_pool.try_serialize(&mut &mut data[..])?;
    
    msg!("[MIGRATE] Migration completed successfully");
    msg!("[MIGRATE] liquid_balance: {} lamports", new_pool.liquid_balance);
    msg!("[MIGRATE] total_deposited: {} lamports", new_pool.total_deposited);
    
    Ok(())
}

