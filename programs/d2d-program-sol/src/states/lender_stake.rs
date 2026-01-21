use crate::errors::ErrorCode;
use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct BackerDeposit {
    pub backer: Pubkey,
    pub deposited_amount: u64,
    pub reward_debt: u128,
    pub pending_rewards: u64,
    pub claimed_total: u64,
    pub is_active: bool,
    pub bump: u8,
}

pub type LenderStake = BackerDeposit;

impl BackerDeposit {
    pub const PREFIX_SEED: &'static [u8] = b"lender_stake";

    pub fn calculate_claimable_rewards(&self, reward_per_share: u128) -> Result<u64> {
        use crate::states::TreasuryPool;

        let accumulated = (self.deposited_amount as u128)
            .checked_mul(reward_per_share)
            .ok_or(ErrorCode::CalculationOverflow)?;

        let from_reward_per_share = accumulated
            .checked_sub(self.reward_debt)
            .ok_or(ErrorCode::CalculationOverflow)?
            .checked_div(TreasuryPool::PRECISION)
            .ok_or(ErrorCode::CalculationOverflow)?;

        let total_claimable = self.pending_rewards
            .checked_add(from_reward_per_share as u64)
            .ok_or(ErrorCode::CalculationOverflow)?;

        Ok(total_claimable)
    }

    pub fn settle_pending_rewards(&mut self, reward_per_share: u128) -> Result<()> {
        use crate::states::TreasuryPool;

        let accumulated = (self.deposited_amount as u128)
            .checked_mul(reward_per_share)
            .ok_or(ErrorCode::CalculationOverflow)?;

        let new_rewards = accumulated
            .checked_sub(self.reward_debt)
            .ok_or(ErrorCode::CalculationOverflow)?
            .checked_div(TreasuryPool::PRECISION)
            .ok_or(ErrorCode::CalculationOverflow)?;

        self.pending_rewards = self.pending_rewards
            .checked_add(new_rewards as u64)
            .ok_or(ErrorCode::CalculationOverflow)?;

        Ok(())
    }

    pub fn update_reward_debt(&mut self, reward_per_share: u128) -> Result<()> {
        self.reward_debt = (self.deposited_amount as u128)
            .checked_mul(reward_per_share)
            .ok_or(ErrorCode::CalculationOverflow)?;
        Ok(())
    }
}
