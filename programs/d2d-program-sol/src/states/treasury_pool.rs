use crate::errors::ErrorCode;
use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct TreasuryPool {
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

    pub fn calculate_reward_fee(deposit_amount: u64) -> Result<u64> {
        let fee = (deposit_amount as u128)
            .checked_mul(Self::REWARD_FEE_BPS as u128)
            .ok_or(ErrorCode::CalculationOverflow)?
            .checked_div(10000)
            .ok_or(ErrorCode::CalculationOverflow)?;
        Ok(fee as u64)
    }

    pub fn calculate_platform_fee(deposit_amount: u64) -> Result<u64> {
        let fee = (deposit_amount as u128)
            .checked_mul(Self::PLATFORM_FEE_BPS as u128)
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
}
