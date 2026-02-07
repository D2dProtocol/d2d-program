use crate::errors::ErrorCode;
use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct TreasuryPool {
    // === EXISTING FIELDS ===
    pub reward_per_share: u128,
    pub total_deposited: u64,
    pub liquid_balance: u64,
    pub reward_pool_balance: u64,
    pub platform_pool_balance: u64,
    pub reward_fee_bps: u64,
    pub platform_fee_bps: u64,
    pub admin: Pubkey,
    pub dev_wallet: Pubkey,
    pub emergency_pause: bool,
    pub guardian: Pubkey,
    pub timelock_duration: i64,
    pub pending_withdrawal_count: u8,
    pub daily_withdrawal_limit: u64,
    pub last_withdrawal_day: i64,
    pub withdrawn_today: u64,
    pub total_credited_rewards: u64,
    pub total_claimed_rewards: u64,
    pub reward_pool_bump: u8,
    pub platform_pool_bump: u8,
    pub bump: u8,

    // === DEBT TRACKING ===
    /// Total outstanding borrowed amount across all active deployments
    pub total_borrowed: u64,
    /// Total recovered from rent reclamation (lifetime)
    pub total_recovered: u64,
    /// Total debt repaid from rent recovery
    pub total_debt_repaid: u64,
    /// Active deployment count
    pub active_deployment_count: u32,

    // === FAIR REWARD DISTRIBUTION ===
    /// Total duration-weighted stake (sum of stake_amount * duration_seconds)
    pub total_stake_duration_weight: u128,
    /// Last time stake_duration_weight was updated
    pub last_weight_update: i64,
    /// Accumulated rewards waiting for stakers (prevents first-depositor arbitrage)
    pub pending_undistributed_rewards: u64,

    // === WITHDRAWAL QUEUE ===
    /// Head of withdrawal queue (oldest pending request)
    pub withdrawal_queue_head: u32,
    /// Tail of withdrawal queue (newest pending request)
    pub withdrawal_queue_tail: u32,
    /// Total amount in withdrawal queue
    pub queued_withdrawal_amount: u64,

    // === DYNAMIC APY ===
    /// Base APY in basis points (e.g., 500 = 5%)
    pub base_apy_bps: u64,
    /// Maximum APY multiplier when utilization is high (e.g., 30000 = 3x)
    pub max_apy_multiplier_bps: u64,
    /// Target utilization for optimal APY (e.g., 6000 = 60%)
    pub target_utilization_bps: u64,
}

impl TreasuryPool {
    pub const PREFIX_SEED: &'static [u8] = b"treasury_pool";
    pub const REWARD_POOL_SEED: &'static [u8] = b"reward_pool";
    pub const PLATFORM_POOL_SEED: &'static [u8] = b"platform_pool";

    pub const REWARD_FEE_BPS: u64 = 100;
    pub const PLATFORM_FEE_BPS: u64 = 10;
    pub const PRECISION: u128 = 1_000_000_000_000;
    pub const MAX_AMOUNT: u128 = 1_000_000_000 * 1_000_000_000;

    pub const DEFAULT_TIMELOCK_DURATION: i64 = 24 * 60 * 60;
    pub const MIN_TIMELOCK_DURATION: i64 = 60 * 60;
    pub const MAX_TIMELOCK_DURATION: i64 = 7 * 24 * 60 * 60;

    pub const SECONDS_PER_DAY: i64 = 24 * 60 * 60;
    pub const DEFAULT_DAILY_LIMIT: u64 = 0;

    // Pool utilization limit - max 80% of liquid_balance can be used for deployments
    pub const MAX_UTILIZATION_BPS: u64 = 8000; // 80% in basis points

    // Dynamic APY defaults
    pub const DEFAULT_BASE_APY_BPS: u64 = 500; // 5% base APY
    pub const DEFAULT_MAX_APY_MULTIPLIER_BPS: u64 = 30000; // 3x max multiplier
    pub const DEFAULT_TARGET_UTILIZATION_BPS: u64 = 6000; // 60% target utilization

    // SECURITY FIX M-06: Add rounding to minimize precision loss in fee calculations
    // Using round-half-up: (numerator + denominator/2) / denominator

    pub fn calculate_reward_fee(deposit_amount: u64) -> Result<u64> {
        let numerator = (deposit_amount as u128)
            .checked_mul(Self::REWARD_FEE_BPS as u128)
            .ok_or(ErrorCode::CalculationOverflow)?;

        // Round half up: add 5000 (half of 10000) before dividing
        let fee = numerator
            .checked_add(5000)
            .ok_or(ErrorCode::CalculationOverflow)?
            .checked_div(10000)
            .ok_or(ErrorCode::CalculationOverflow)?;
        Ok(fee as u64)
    }

    pub fn calculate_platform_fee(deposit_amount: u64) -> Result<u64> {
        let numerator = (deposit_amount as u128)
            .checked_mul(Self::PLATFORM_FEE_BPS as u128)
            .ok_or(ErrorCode::CalculationOverflow)?;

        // Round half up: add 5000 (half of 10000) before dividing
        let fee = numerator
            .checked_add(5000)
            .ok_or(ErrorCode::CalculationOverflow)?
            .checked_div(10000)
            .ok_or(ErrorCode::CalculationOverflow)?;
        Ok(fee as u64)
    }

    pub fn credit_fee_to_pool(&mut self, fee_reward: u64, fee_platform: u64) -> Result<()> {
        require!(fee_reward <= Self::MAX_AMOUNT as u64, ErrorCode::FeeAmountTooLarge);
        require!(fee_platform <= Self::MAX_AMOUNT as u64, ErrorCode::FeeAmountTooLarge);

        self.platform_pool_balance = self
            .platform_pool_balance
            .checked_add(fee_platform)
            .ok_or_else(|| ErrorCode::CalculationOverflow)?;

        self.reward_pool_balance = self
            .reward_pool_balance
            .checked_add(fee_reward)
            .ok_or_else(|| ErrorCode::CalculationOverflow)?;

        self.total_credited_rewards = self
            .total_credited_rewards
            .checked_add(fee_reward)
            .ok_or_else(|| ErrorCode::CalculationOverflow)?;

        if self.total_deposited > 0 {
            let delta = (fee_reward as u128)
                .checked_mul(Self::PRECISION)
                .ok_or(ErrorCode::CalculationOverflow)?
                .checked_div(self.total_deposited as u128)
                .ok_or(ErrorCode::CalculationOverflow)?;

            self.reward_per_share = self
                .reward_per_share
                .checked_add(delta)
                .ok_or_else(|| ErrorCode::CalculationOverflow)?;
        }

        Ok(())
    }

    pub fn calculate_claimable_rewards(&self, deposited_amount: u64, reward_debt: u128) -> Result<u64> {
        let accumulated = (deposited_amount as u128)
            .checked_mul(self.reward_per_share)
            .ok_or(ErrorCode::CalculationOverflow)?;

        let claimable = accumulated
            .checked_sub(reward_debt)
            .ok_or(ErrorCode::CalculationOverflow)?
            .checked_div(Self::PRECISION)
            .ok_or(ErrorCode::CalculationOverflow)?;

        Ok(claimable as u64)
    }

    pub fn credit_reward_pool(&mut self, amount: u128) -> Result<()> {
        require!(amount <= Self::MAX_AMOUNT, ErrorCode::FeeAmountTooLarge);
        self.reward_pool_balance = self
            .reward_pool_balance
            .checked_add(amount as u64)
            .ok_or_else(|| ErrorCode::CalculationOverflow)?;
        Ok(())
    }

    pub fn debit_reward_pool(&mut self, amount: u64) -> Result<()> {
        require!(amount <= Self::MAX_AMOUNT as u64, ErrorCode::FeeAmountTooLarge);
        self.reward_pool_balance = self
            .reward_pool_balance
            .checked_sub(amount)
            .ok_or_else(|| ErrorCode::CalculationOverflow)?;
        Ok(())
    }

    pub fn credit_platform_pool(&mut self, amount: u128) -> Result<()> {
        require!(amount <= Self::MAX_AMOUNT, ErrorCode::FeeAmountTooLarge);
        self.platform_pool_balance = self
            .platform_pool_balance
            .checked_add(amount as u64)
            .ok_or_else(|| ErrorCode::CalculationOverflow)?;
        Ok(())
    }

    pub fn has_guardian(&self) -> bool {
        self.guardian != Pubkey::default()
    }

    pub fn is_admin(&self, caller: &Pubkey) -> bool {
        self.admin == *caller
    }

    pub fn is_guardian(&self, caller: &Pubkey) -> bool {
        self.has_guardian() && self.guardian == *caller
    }

    pub fn is_admin_or_guardian(&self, caller: &Pubkey) -> bool {
        self.is_admin(caller) || self.is_guardian(caller)
    }

    pub fn get_day_timestamp(unix_timestamp: i64) -> i64 {
        (unix_timestamp / Self::SECONDS_PER_DAY) * Self::SECONDS_PER_DAY
    }

    pub fn check_and_update_daily_limit(&mut self, amount: u64, current_time: i64) -> Result<()> {
        if self.daily_withdrawal_limit == 0 {
            return Ok(());
        }

        let current_day = Self::get_day_timestamp(current_time);

        if current_day > self.last_withdrawal_day {
            self.last_withdrawal_day = current_day;
            self.withdrawn_today = 0;
        }

        let new_total = self
            .withdrawn_today
            .checked_add(amount)
            .ok_or(ErrorCode::CalculationOverflow)?;

        require!(
            new_total <= self.daily_withdrawal_limit,
            ErrorCode::DailyWithdrawalLimitExceeded
        );

        self.withdrawn_today = new_total;

        Ok(())
    }

    pub fn get_remaining_daily_allowance(&self, current_time: i64) -> u64 {
        if self.daily_withdrawal_limit == 0 {
            return u64::MAX;
        }

        let current_day = Self::get_day_timestamp(current_time);

        if current_day > self.last_withdrawal_day {
            return self.daily_withdrawal_limit;
        }

        self.daily_withdrawal_limit.saturating_sub(self.withdrawn_today)
    }

    pub fn get_protected_rewards(&self) -> u64 {
        self.total_credited_rewards
            .saturating_sub(self.total_claimed_rewards)
    }

    pub fn get_excess_rewards(&self) -> u64 {
        let protected = self.get_protected_rewards();
        self.reward_pool_balance.saturating_sub(protected)
    }

    pub fn can_withdraw_from_reward_pool(&self, amount: u64) -> bool {
        amount <= self.get_excess_rewards()
    }

    pub fn credit_rewards_with_tracking(&mut self, amount: u64) -> Result<()> {
        self.reward_pool_balance = self
            .reward_pool_balance
            .checked_add(amount)
            .ok_or(ErrorCode::CalculationOverflow)?;

        self.total_credited_rewards = self
            .total_credited_rewards
            .checked_add(amount)
            .ok_or(ErrorCode::CalculationOverflow)?;

        Ok(())
    }

    pub fn record_claimed_rewards(&mut self, amount: u64) -> Result<()> {
        self.total_claimed_rewards = self
            .total_claimed_rewards
            .checked_add(amount)
            .ok_or(ErrorCode::CalculationOverflow)?;
        Ok(())
    }

    /// Check if deploying the given amount would exceed 80% utilization limit
    /// Returns true if utilization is within acceptable limits
    pub fn check_utilization_limit(&self, deployment_amount: u64) -> Result<bool> {
        if self.total_deposited == 0 {
            // No deposits means no limit - allow deployments from admin funds only
            return Ok(true);
        }

        // Calculate remaining liquid balance after deployment
        let remaining = self
            .liquid_balance
            .checked_sub(deployment_amount)
            .unwrap_or(0);

        // Calculate what percentage of total_deposited remains liquid
        // remaining >= 20% of total_deposited means utilization <= 80%
        let min_reserve = (self.total_deposited as u128)
            .checked_mul((10000 - Self::MAX_UTILIZATION_BPS) as u128)
            .ok_or(ErrorCode::CalculationOverflow)?
            .checked_div(10000)
            .ok_or(ErrorCode::CalculationOverflow)? as u64;

        Ok(remaining >= min_reserve)
    }

    /// Calculate claimable rewards with enhanced validation
    /// Returns error with descriptive message if reward_debt exceeds accumulated
    pub fn calculate_claimable_rewards_safe(
        &self,
        deposited_amount: u64,
        reward_debt: u128,
    ) -> Result<u64> {
        let accumulated = (deposited_amount as u128)
            .checked_mul(self.reward_per_share)
            .ok_or(ErrorCode::CalculationOverflow)?;

        // SECURITY: Validate reward_debt doesn't exceed accumulated
        require!(
            reward_debt <= accumulated,
            ErrorCode::RewardDebtExceedsAccumulated
        );

        let claimable = accumulated
            .checked_sub(reward_debt)
            .ok_or(ErrorCode::CalculationOverflow)?
            .checked_div(Self::PRECISION)
            .ok_or(ErrorCode::CalculationOverflow)?;

        Ok(claimable as u64)
    }

    // === DEBT TRACKING METHODS ===

    /// Record a new deployment borrowing funds from treasury
    pub fn record_deployment_borrow(&mut self, amount: u64) -> Result<()> {
        self.total_borrowed = self
            .total_borrowed
            .checked_add(amount)
            .ok_or(ErrorCode::CalculationOverflow)?;
        self.active_deployment_count = self
            .active_deployment_count
            .checked_add(1)
            .ok_or(ErrorCode::CalculationOverflow)?;
        Ok(())
    }

    /// Record debt repayment from rent recovery
    /// Returns (debt_repayment, excess_to_rewards)
    pub fn record_debt_repayment(
        &mut self,
        recovered_amount: u64,
        remaining_debt: u64,
    ) -> Result<(u64, u64)> {
        // Calculate how much goes to debt repayment vs rewards
        let debt_repayment = recovered_amount.min(remaining_debt);
        let excess_to_rewards = recovered_amount.saturating_sub(debt_repayment);

        // Update global debt tracking
        self.total_recovered = self
            .total_recovered
            .checked_add(recovered_amount)
            .ok_or(ErrorCode::CalculationOverflow)?;
        self.total_debt_repaid = self
            .total_debt_repaid
            .checked_add(debt_repayment)
            .ok_or(ErrorCode::CalculationOverflow)?;
        self.total_borrowed = self
            .total_borrowed
            .saturating_sub(debt_repayment);
        self.active_deployment_count = self
            .active_deployment_count
            .saturating_sub(1);

        // Debt repayment restores liquid_balance for withdrawals
        self.liquid_balance = self
            .liquid_balance
            .checked_add(debt_repayment)
            .ok_or(ErrorCode::CalculationOverflow)?;

        Ok((debt_repayment, excess_to_rewards))
    }

    /// Get current utilization rate in basis points
    pub fn get_utilization_bps(&self) -> u64 {
        if self.total_deposited == 0 {
            return 0;
        }
        ((self.total_borrowed as u128) * 10000 / (self.total_deposited as u128)) as u64
    }

    /// Get global recovery ratio in basis points
    pub fn get_recovery_ratio_bps(&self) -> u64 {
        let total_ever_borrowed = self.total_borrowed
            .saturating_add(self.total_debt_repaid);
        if total_ever_borrowed == 0 {
            return 10000; // 100% if no borrowing
        }
        ((self.total_recovered as u128) * 10000 / (total_ever_borrowed as u128)) as u64
    }

    // === DYNAMIC APY METHODS ===

    /// Calculate current APY based on utilization rate
    /// Higher utilization = higher APY to attract more deposits
    pub fn calculate_current_apy(&self) -> Result<u64> {
        if self.base_apy_bps == 0 {
            return Ok(0);
        }

        let utilization_bps = self.get_utilization_bps();

        // APY multiplier curve:
        // - At 0% utilization: base_apy (1x)
        // - At target_utilization (60%): base_apy * 1.5x
        // - At 80%+ utilization: base_apy * max_multiplier (3x)
        let multiplier_bps = if utilization_bps >= Self::MAX_UTILIZATION_BPS {
            self.max_apy_multiplier_bps
        } else if utilization_bps >= self.target_utilization_bps {
            // Linear interpolation between target (1.5x) and max (3x)
            let utilization_above_target = utilization_bps
                .saturating_sub(self.target_utilization_bps);
            let range = Self::MAX_UTILIZATION_BPS
                .saturating_sub(self.target_utilization_bps);
            let multiplier_range = self.max_apy_multiplier_bps
                .saturating_sub(15000); // 1.5x to max

            if range == 0 {
                15000
            } else {
                15000 + ((utilization_above_target as u128)
                    .checked_mul(multiplier_range as u128)
                    .ok_or(ErrorCode::CalculationOverflow)?
                    .checked_div(range as u128)
                    .ok_or(ErrorCode::CalculationOverflow)?) as u64
            }
        } else {
            // Below target: 1x to 1.5x
            let multiplier_range = 5000u64; // 1x to 1.5x = 0.5x range

            if self.target_utilization_bps == 0 {
                10000
            } else {
                10000 + ((utilization_bps as u128)
                    .checked_mul(multiplier_range as u128)
                    .ok_or(ErrorCode::CalculationOverflow)?
                    .checked_div(self.target_utilization_bps as u128)
                    .ok_or(ErrorCode::CalculationOverflow)?) as u64
            }
        };

        // Final APY = base_apy * multiplier / 10000
        let current_apy = (self.base_apy_bps as u128)
            .checked_mul(multiplier_bps as u128)
            .ok_or(ErrorCode::CalculationOverflow)?
            .checked_div(10000)
            .ok_or(ErrorCode::CalculationOverflow)? as u64;

        Ok(current_apy)
    }

    // === WITHDRAWAL QUEUE METHODS ===

    /// Add withdrawal to queue
    pub fn add_to_withdrawal_queue(&mut self, amount: u64) -> Result<u32> {
        let position = self.withdrawal_queue_tail;
        self.withdrawal_queue_tail = self
            .withdrawal_queue_tail
            .checked_add(1)
            .ok_or(ErrorCode::CalculationOverflow)?;
        self.queued_withdrawal_amount = self
            .queued_withdrawal_amount
            .checked_add(amount)
            .ok_or(ErrorCode::CalculationOverflow)?;
        Ok(position)
    }

    /// Process withdrawal from queue
    pub fn process_queued_withdrawal(&mut self, amount: u64) -> Result<()> {
        self.queued_withdrawal_amount = self
            .queued_withdrawal_amount
            .saturating_sub(amount);
        Ok(())
    }

    /// Check if withdrawal queue has pending entries
    pub fn has_pending_withdrawals(&self) -> bool {
        self.withdrawal_queue_tail > self.withdrawal_queue_head
    }

    /// Get number of pending withdrawals
    pub fn get_pending_withdrawal_count(&self) -> u32 {
        self.withdrawal_queue_tail.saturating_sub(self.withdrawal_queue_head)
    }

    // === FAIR REWARD DISTRIBUTION METHODS ===

    /// Move rewards to pending_undistributed (for gradual distribution)
    pub fn move_to_pending_rewards(&mut self, amount: u64) -> Result<()> {
        self.pending_undistributed_rewards = self
            .pending_undistributed_rewards
            .checked_add(amount)
            .ok_or(ErrorCode::CalculationOverflow)?;
        Ok(())
    }

    /// Distribute a portion of pending rewards to reward_per_share
    pub fn distribute_pending_rewards(&mut self, percentage_bps: u64) -> Result<u64> {
        if self.pending_undistributed_rewards == 0 || self.total_deposited == 0 {
            return Ok(0);
        }

        let amount_to_distribute = (self.pending_undistributed_rewards as u128)
            .checked_mul(percentage_bps as u128)
            .ok_or(ErrorCode::CalculationOverflow)?
            .checked_div(10000)
            .ok_or(ErrorCode::CalculationOverflow)? as u64;

        if amount_to_distribute == 0 {
            return Ok(0);
        }

        // Update reward_per_share
        let delta = (amount_to_distribute as u128)
            .checked_mul(Self::PRECISION)
            .ok_or(ErrorCode::CalculationOverflow)?
            .checked_div(self.total_deposited as u128)
            .ok_or(ErrorCode::CalculationOverflow)?;

        self.reward_per_share = self
            .reward_per_share
            .checked_add(delta)
            .ok_or(ErrorCode::CalculationOverflow)?;

        self.pending_undistributed_rewards = self
            .pending_undistributed_rewards
            .saturating_sub(amount_to_distribute);

        Ok(amount_to_distribute)
    }

    /// Update duration weight tracking
    pub fn update_stake_duration_weight(&mut self, weight_delta: u128) -> Result<()> {
        self.total_stake_duration_weight = self
            .total_stake_duration_weight
            .checked_add(weight_delta)
            .ok_or(ErrorCode::CalculationOverflow)?;
        Ok(())
    }

    /// Calculate duration-weighted share of pending rewards for a staker
    pub fn calculate_duration_bonus(
        &self,
        staker_weight: u128,
    ) -> Result<u64> {
        if self.total_stake_duration_weight == 0 || self.pending_undistributed_rewards == 0 {
            return Ok(0);
        }

        let bonus = (self.pending_undistributed_rewards as u128)
            .checked_mul(staker_weight)
            .ok_or(ErrorCode::CalculationOverflow)?
            .checked_div(self.total_stake_duration_weight)
            .ok_or(ErrorCode::CalculationOverflow)? as u64;

        Ok(bonus)
    }
}
