use anchor_lang::prelude::*;
use crate::errors::ErrorCode;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq, InitSpace)]
pub enum DeployRequestStatus {
    PendingDeployment,   // Payment made, waiting for deployment
    Active,              // Active with valid subscription
    SubscriptionExpired, // Subscription expired
    InGracePeriod,       // Subscription expired, in grace period before closure
    Suspended,           // Suspended due to non-payment
    Failed,              // Deployment failed
    Cancelled,           // Cancelled by developer
    Closed,              // Program closed, lamports recovered
}

#[account]
#[derive(InitSpace)]
pub struct DeployRequest {
    pub request_id: [u8; 32],                // Unique request identifier
    pub developer: Pubkey,                   // Developer public key
    pub program_hash: [u8; 32],              // Hash of program to deploy
    pub service_fee: u64,                    // One-time service fee
    pub monthly_fee: u64,                    // Monthly subscription fee
    pub deployment_cost: u64,                // Actual deployment cost from treasury
    pub borrowed_amount: u64,                // Amount borrowed from treasury (for fee calculation: 1% monthly)
    pub subscription_paid_until: i64,        // Subscription valid until timestamp
    pub ephemeral_key: Option<Pubkey>,       // Temporary key for deployment
    pub deployed_program_id: Option<Pubkey>, // Deployed program ID
    pub status: DeployRequestStatus,         // Current status
    pub created_at: i64,                     // Creation timestamp
    pub bump: u8,                            // PDA bump

    // Grace period fields
    pub grace_period_days: u8,               // Grace period duration (3, 5, or 7 days)
    pub grace_period_end: i64,               // Grace period end timestamp (0 if not in grace)
    pub total_subscribed_months: u32,        // Total months ever subscribed (for grace calc)
    pub auto_renewal_enabled: bool,          // Per-program auto-renewal toggle
    pub last_renewal_at: i64,                // Last successful renewal timestamp
    pub auto_renewal_failed_count: u8,       // Failed auto-renewal attempts

    // === DEBT REPAYMENT TRACKING ===
    /// Amount already repaid from rent recovery
    pub repaid_amount: u64,
    /// Expected rent recoverable (estimated at deployment)
    pub expected_rent_recovery: u64,
    /// Actual rent recovered on closure
    pub actual_rent_recovered: u64,
    /// Recovery ratio in basis points: (actual_recovered / borrowed_amount) * 10000
    pub recovery_ratio_bps: u64,
    /// Timestamp when debt was fully repaid (0 if not yet repaid)
    pub debt_repaid_at: i64,
}

impl DeployRequest {
    pub const PREFIX_SEED: &'static [u8] = b"deploy_request";
    pub const SECONDS_PER_DAY: i64 = 24 * 60 * 60;
    pub const SECONDS_PER_MONTH: i64 = 30 * Self::SECONDS_PER_DAY;
    pub const MAX_EXTENSION_MONTHS: u32 = 120; // Maximum 10 years extension at once

    pub fn is_subscription_valid(&self) -> Result<bool> {
        let current_time = Clock::get()?.unix_timestamp;
        Ok(current_time <= self.subscription_paid_until)
    }

    /// Extend subscription with overflow protection
    /// Returns error if extension would cause overflow or exceeds maximum
    pub fn extend_subscription(&mut self, months: u32) -> Result<()> {
        // SECURITY: Prevent excessive subscription extensions
        require!(
            months <= Self::MAX_EXTENSION_MONTHS,
            ErrorCode::SubscriptionExtensionTooLarge
        );

        // SECURITY: Use checked arithmetic to prevent overflow
        let extension_seconds = (months as i64)
            .checked_mul(Self::SECONDS_PER_MONTH)
            .ok_or(ErrorCode::SubscriptionExtensionOverflow)?;

        self.subscription_paid_until = self
            .subscription_paid_until
            .checked_add(extension_seconds)
            .ok_or(ErrorCode::SubscriptionExtensionOverflow)?;

        // Update total subscribed months for grace period calculation
        self.total_subscribed_months = self.total_subscribed_months.saturating_add(months);
        self.last_renewal_at = Clock::get().map(|c| c.unix_timestamp).unwrap_or(0);

        // Reset failed count on successful renewal
        self.auto_renewal_failed_count = 0;

        // Exit grace period if in it
        if self.status == DeployRequestStatus::InGracePeriod {
            self.status = DeployRequestStatus::Active;
            self.grace_period_end = 0;
        }

        Ok(())
    }

    /// Calculate grace period days based on total subscribed months
    /// 1-2 months = 3 days, 3-5 months = 5 days, 6+ months = 7 days
    pub fn calculate_grace_period_days(&self) -> u8 {
        if self.total_subscribed_months >= 6 {
            7
        } else if self.total_subscribed_months >= 3 {
            5
        } else {
            3
        }
    }

    /// Start grace period
    pub fn start_grace_period(&mut self) -> Result<()> {
        let current_time = Clock::get()?.unix_timestamp;

        self.grace_period_days = self.calculate_grace_period_days();
        self.grace_period_end = current_time + (self.grace_period_days as i64 * Self::SECONDS_PER_DAY);
        self.status = DeployRequestStatus::InGracePeriod;

        Ok(())
    }

    /// Check if grace period has expired
    pub fn is_grace_period_expired(&self) -> Result<bool> {
        if self.status != DeployRequestStatus::InGracePeriod {
            return Ok(false);
        }

        let current_time = Clock::get()?.unix_timestamp;
        Ok(current_time > self.grace_period_end)
    }

    /// Check if currently in grace period
    pub fn is_in_grace_period(&self) -> bool {
        self.status == DeployRequestStatus::InGracePeriod && self.grace_period_end > 0
    }

    /// Increment auto-renewal failed count
    pub fn increment_auto_renewal_failed(&mut self) {
        self.auto_renewal_failed_count = self.auto_renewal_failed_count.saturating_add(1);
    }

    /// Calculate the 1% monthly borrow fee on borrowed_amount
    /// This fee is charged monthly for using treasury funds for deployment
    pub fn calculate_monthly_borrow_fee(&self) -> Result<u64> {
        // 1% = 100 basis points
        const MONTHLY_FEE_BPS: u64 = 100;

        let fee = (self.borrowed_amount as u128)
            .checked_mul(MONTHLY_FEE_BPS as u128)
            .ok_or(ErrorCode::CalculationOverflow)?
            .checked_div(10000)
            .ok_or(ErrorCode::CalculationOverflow)?;

        Ok(fee as u64)
    }

    /// Calculate total borrow fees based on months elapsed since deployment
    pub fn calculate_total_borrow_fees(&self) -> Result<u64> {
        let current_time = Clock::get()?.unix_timestamp;
        let elapsed_seconds = current_time
            .checked_sub(self.created_at)
            .unwrap_or(0);

        // Calculate months elapsed (rounded up to next month)
        let months_elapsed = (elapsed_seconds as u64)
            .checked_add(Self::SECONDS_PER_MONTH as u64 - 1)
            .ok_or(ErrorCode::CalculationOverflow)?
            .checked_div(Self::SECONDS_PER_MONTH as u64)
            .ok_or(ErrorCode::CalculationOverflow)?;

        let monthly_fee = self.calculate_monthly_borrow_fee()?;
        let total_fee = monthly_fee
            .checked_mul(months_elapsed)
            .ok_or(ErrorCode::CalculationOverflow)?;

        Ok(total_fee)
    }

    // === DEBT REPAYMENT METHODS ===

    /// Get remaining debt (borrowed_amount - repaid_amount)
    pub fn get_remaining_debt(&self) -> u64 {
        self.borrowed_amount.saturating_sub(self.repaid_amount)
    }

    /// Check if debt is fully repaid
    pub fn is_debt_repaid(&self) -> bool {
        self.repaid_amount >= self.borrowed_amount
    }

    /// Record debt repayment from rent recovery
    /// Returns (debt_repayment, excess_to_rewards)
    pub fn record_rent_recovery(&mut self, recovered_amount: u64) -> Result<(u64, u64)> {
        let remaining_debt = self.get_remaining_debt();

        // Calculate how much goes to debt repayment vs excess rewards
        let debt_repayment = recovered_amount.min(remaining_debt);
        let excess_to_rewards = recovered_amount.saturating_sub(debt_repayment);

        // Update repaid amount
        self.repaid_amount = self
            .repaid_amount
            .checked_add(debt_repayment)
            .ok_or(ErrorCode::CalculationOverflow)?;

        // Update actual rent recovered
        self.actual_rent_recovered = self
            .actual_rent_recovered
            .checked_add(recovered_amount)
            .ok_or(ErrorCode::CalculationOverflow)?;

        // Calculate recovery ratio
        if self.borrowed_amount > 0 {
            self.recovery_ratio_bps = ((self.actual_rent_recovered as u128)
                .checked_mul(10000)
                .ok_or(ErrorCode::CalculationOverflow)?
                .checked_div(self.borrowed_amount as u128)
                .ok_or(ErrorCode::CalculationOverflow)?) as u64;
        }

        // Mark debt as fully repaid if applicable
        if self.is_debt_repaid() && self.debt_repaid_at == 0 {
            self.debt_repaid_at = Clock::get()?.unix_timestamp;
        }

        Ok((debt_repayment, excess_to_rewards))
    }

    /// Set expected rent recovery estimate (called during deployment funding)
    pub fn set_expected_rent_recovery(&mut self, deployment_cost: u64) {
        // Typically ~80% of deployment cost is recoverable as rent
        self.expected_rent_recovery = deployment_cost
            .checked_mul(80)
            .and_then(|x| x.checked_div(100))
            .unwrap_or(0);
    }

    /// Get debt repayment status as a percentage (0-100)
    pub fn get_repayment_percentage(&self) -> u8 {
        if self.borrowed_amount == 0 {
            return 100;
        }
        ((self.repaid_amount as u128) * 100 / (self.borrowed_amount as u128)) as u8
    }
}
