use crate::states::TreasuryPool;
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct MigrateTreasuryPool<'info> {
    /// CHECK: Treasury Pool PDA - will be resized and migrated
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

pub fn migrate_treasury_pool(ctx: Context<MigrateTreasuryPool>) -> Result<()> {
    let treasury_pool_info = ctx.accounts.treasury_pool.to_account_info();
    let required_space = 8 + TreasuryPool::INIT_SPACE;
    let current_space = treasury_pool_info.data_len();

    if current_space == required_space {
        if TreasuryPool::try_deserialize(&mut &treasury_pool_info.data.borrow()[..]).is_ok() {
            return Ok(());
        }
    }

    let old_data = treasury_pool_info.data.borrow();
    let mut old_pool_data = vec![0u8; old_data.len()];
    old_pool_data.copy_from_slice(&old_data);
    drop(old_data);

    if current_space != required_space {
        treasury_pool_info.resize(required_space)?;
    }

    let mut data = treasury_pool_info.try_borrow_mut_data()?;

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
        guardian: Pubkey::default(),
        timelock_duration: TreasuryPool::DEFAULT_TIMELOCK_DURATION,
        pending_withdrawal_count: 0,
        daily_withdrawal_limit: TreasuryPool::DEFAULT_DAILY_LIMIT,
        last_withdrawal_day: 0,
        withdrawn_today: 0,
        total_credited_rewards: 0,
        total_claimed_rewards: 0,
        reward_pool_bump: 0,
        platform_pool_bump: 0,
        bump: ctx.bumps.treasury_pool,
        // Debt tracking fields
        total_borrowed: 0,
        total_recovered: 0,
        total_debt_repaid: 0,
        active_deployment_count: 0,
        // Fair reward distribution fields
        total_stake_duration_weight: 0,
        last_weight_update: 0,
        pending_undistributed_rewards: 0,
        // Withdrawal queue fields
        withdrawal_queue_head: 0,
        withdrawal_queue_tail: 0,
        queued_withdrawal_amount: 0,
        // Dynamic APY fields
        base_apy_bps: TreasuryPool::DEFAULT_BASE_APY_BPS,
        max_apy_multiplier_bps: TreasuryPool::DEFAULT_MAX_APY_MULTIPLIER_BPS,
        target_utilization_bps: TreasuryPool::DEFAULT_TARGET_UTILIZATION_BPS,
    };

    if old_pool_data.len() >= 8 {
        if let Ok(old_pool) = TreasuryPool::try_deserialize(&mut &old_pool_data[..]) {
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
            new_pool.guardian = old_pool.guardian;
            new_pool.timelock_duration = old_pool.timelock_duration;
            new_pool.pending_withdrawal_count = old_pool.pending_withdrawal_count;
            new_pool.daily_withdrawal_limit = old_pool.daily_withdrawal_limit;
            new_pool.last_withdrawal_day = old_pool.last_withdrawal_day;
            new_pool.withdrawn_today = old_pool.withdrawn_today;
            new_pool.total_credited_rewards = old_pool.total_credited_rewards;
            new_pool.total_claimed_rewards = old_pool.total_claimed_rewards;
            new_pool.reward_pool_bump = old_pool.reward_pool_bump;
            new_pool.platform_pool_bump = old_pool.platform_pool_bump;
            new_pool.bump = old_pool.bump;
        }
    }

    new_pool.try_serialize(&mut &mut data[..])?;

    Ok(())
}
