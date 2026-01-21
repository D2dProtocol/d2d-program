use anchor_lang::prelude::*;

pub mod errors;
pub mod events;
pub mod instructions;
pub mod states;

pub use events::*;
use instructions::*;
pub use states::*;

declare_id!("HDxYgZcTu6snVtCEozCUkhwmmUngWEsYuNKJsvgpyL5k");

#[program]
pub mod d2d_program_sol {
    use super::*;

    pub fn initialize(
        ctx: Context<Initialize>,
        initial_apy: u64,
        dev_wallet: Pubkey,
    ) -> Result<()> {
        instructions::initialize(ctx, initial_apy, dev_wallet)
    }

    pub fn stake_sol(ctx: Context<StakeSol>, amount: u64, lock_period: i64) -> Result<()> {
        instructions::stake_sol(ctx, amount, lock_period)
    }

    pub fn unstake_sol(ctx: Context<UnstakeSol>, amount: u64) -> Result<()> {
        instructions::unstake_sol(ctx, amount)
    }

    pub fn claim_rewards(ctx: Context<ClaimRewards>) -> Result<()> {
        instructions::claim_rewards(ctx)
    }

    pub fn emergency_unstake_sol(ctx: Context<EmergencyUnstakeSol>, amount: u64) -> Result<()> {
        instructions::emergency_unstake_sol(ctx, amount)
    }

    pub fn request_deployment_funds(
        ctx: Context<RequestDeploymentFunds>,
        program_hash: [u8; 32],
        service_fee: u64,
        monthly_fee: u64,
        initial_months: u32,
        deployment_cost: u64,
    ) -> Result<()> {
        instructions::request_deployment_funds(ctx, program_hash, service_fee, monthly_fee, initial_months, deployment_cost)
    }

    pub fn pay_subscription(
        ctx: Context<PaySubscription>,
        request_id: [u8; 32],
        months: u32,
    ) -> Result<()> {
        instructions::pay_subscription(ctx, request_id, months)
    }

    pub fn emergency_pause(ctx: Context<EmergencyPause>, pause: bool) -> Result<()> {
        instructions::emergency_pause(ctx, pause)
    }

    pub fn confirm_deployment_success(
        ctx: Context<ConfirmDeployment>,
        request_id: [u8; 32],
        deployed_program_id: Pubkey,
        recovered_funds: u64,
    ) -> Result<()> {
        instructions::confirm_deployment_success(ctx, request_id, deployed_program_id, recovered_funds)
    }

    pub fn confirm_deployment_failure(
        ctx: Context<ConfirmDeployment>,
        request_id: [u8; 32],
        failure_reason: String,
    ) -> Result<()> {
        instructions::confirm_deployment_failure(ctx, request_id, failure_reason)
    }

    pub fn close_program_and_refund(
        ctx: Context<CloseProgramAndRefund>,
        request_id: [u8; 32],
        recovered_lamports: u64,
    ) -> Result<()> {
        instructions::close_program_and_refund(ctx, request_id, recovered_lamports)
    }

    pub fn fund_temporary_wallet(
        ctx: Context<FundTemporaryWallet>,
        request_id: [u8; 32],
        amount: u64,
        use_admin_pool: bool,
    ) -> Result<()> {
        instructions::fund_temporary_wallet(ctx, request_id, amount, use_admin_pool)
    }

    pub fn create_deploy_request(
        ctx: Context<CreateDeployRequest>,
        program_hash: [u8; 32],
        service_fee: u64,
        monthly_fee: u64,
        initial_months: u32,
        deployment_cost: u64,
    ) -> Result<()> {
        instructions::create_deploy_request(ctx, program_hash, service_fee, monthly_fee, initial_months, deployment_cost)
    }

    pub fn admin_withdraw(
        ctx: Context<AdminWithdraw>,
        amount: u64,
        reason: String,
    ) -> Result<()> {
        instructions::admin_withdraw(ctx, amount, reason)
    }

    pub fn admin_withdraw_reward_pool(
        ctx: Context<AdminWithdrawRewardPool>,
        amount: u64,
        reason: String,
    ) -> Result<()> {
        instructions::admin_withdraw_reward_pool(ctx, amount, reason)
    }

    pub fn close_treasury_pool(ctx: Context<CloseTreasuryPool>) -> Result<()> {
        instructions::close_treasury_pool(ctx)
    }

    pub fn reinitialize_treasury_pool(
        ctx: Context<ReinitializeTreasuryPool>,
        initial_apy: u64,
        dev_wallet: Pubkey,
    ) -> Result<()> {
        instructions::reinitialize_treasury_pool(ctx, initial_apy, dev_wallet)
    }

    pub fn credit_fee_to_pool(
        ctx: Context<CreditFeeToPool>,
        fee_reward: u64,
        fee_platform: u64,
    ) -> Result<()> {
        instructions::credit_fee_to_pool(ctx, fee_reward, fee_platform)
    }

    pub fn sync_liquid_balance(ctx: Context<SyncLiquidBalance>) -> Result<()> {
        instructions::sync_liquid_balance(ctx)
    }

    pub fn force_rebalance(ctx: Context<ForceRebalance>) -> Result<()> {
        instructions::force_rebalance(ctx)
    }

    pub fn migrate_treasury_pool(ctx: Context<MigrateTreasuryPool>) -> Result<()> {
        instructions::migrate_treasury_pool(ctx)
    }

    pub fn set_guardian(ctx: Context<SetGuardian>, new_guardian: Pubkey) -> Result<()> {
        instructions::set_guardian(ctx, new_guardian)
    }

    pub fn guardian_pause(ctx: Context<GuardianPause>) -> Result<()> {
        instructions::guardian_pause(ctx)
    }

    pub fn set_timelock_duration(ctx: Context<SetTimelockDuration>, new_duration: i64) -> Result<()> {
        instructions::set_timelock_duration(ctx, new_duration)
    }

    pub fn set_daily_limit(ctx: Context<SetDailyLimit>, new_limit: u64) -> Result<()> {
        instructions::set_daily_limit(ctx, new_limit)
    }

    pub fn initiate_withdrawal(
        ctx: Context<InitiateWithdrawal>,
        withdrawal_type: states::WithdrawalType,
        amount: u64,
        destination: Pubkey,
        reason: String,
    ) -> Result<()> {
        instructions::initiate_withdrawal(ctx, withdrawal_type, amount, destination, reason)
    }

    pub fn execute_withdrawal(ctx: Context<ExecuteWithdrawal>) -> Result<()> {
        instructions::execute_withdrawal(ctx)
    }

    pub fn guardian_veto(ctx: Context<GuardianVeto>) -> Result<()> {
        instructions::guardian_veto(ctx)
    }

    pub fn cancel_withdrawal(ctx: Context<CancelWithdrawal>) -> Result<()> {
        instructions::cancel_withdrawal(ctx)
    }
}
