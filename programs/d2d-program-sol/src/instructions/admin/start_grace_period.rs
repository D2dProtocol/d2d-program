use anchor_lang::prelude::*;

use crate::errors::ErrorCode;
use crate::events::GracePeriodStarted;
use crate::states::{DeployRequest, DeployRequestStatus, TreasuryPool};

#[derive(Accounts)]
#[instruction(request_id: [u8; 32])]
pub struct StartGracePeriod<'info> {
    #[account(
        seeds = [TreasuryPool::PREFIX_SEED],
        bump = treasury_pool.bump
    )]
    pub treasury_pool: Account<'info, TreasuryPool>,

    #[account(
        mut,
        seeds = [DeployRequest::PREFIX_SEED, deploy_request.program_hash.as_ref()],
        bump = deploy_request.bump,
        constraint = deploy_request.request_id == request_id @ ErrorCode::InvalidRequestId
    )]
    pub deploy_request: Account<'info, DeployRequest>,

    #[account(
        constraint = treasury_pool.is_admin(&admin.key()) @ ErrorCode::Unauthorized
    )]
    pub admin: Signer<'info>,
}

pub fn start_grace_period(ctx: Context<StartGracePeriod>, request_id: [u8; 32]) -> Result<()> {
    let treasury_pool = &ctx.accounts.treasury_pool;
    let deploy_request = &mut ctx.accounts.deploy_request;

    require!(!treasury_pool.emergency_pause, ErrorCode::ProgramPaused);

    // Verify subscription is expired (not already in grace period)
    require!(
        deploy_request.status == DeployRequestStatus::SubscriptionExpired,
        ErrorCode::InvalidRequestStatus
    );

    // Verify not already in grace period
    require!(
        deploy_request.grace_period_end == 0,
        ErrorCode::AlreadyInGracePeriod
    );

    // Start grace period
    deploy_request.start_grace_period()?;

    let current_time = Clock::get()?.unix_timestamp;

    emit!(GracePeriodStarted {
        request_id,
        developer: deploy_request.developer,
        grace_period_days: deploy_request.grace_period_days,
        grace_period_end: deploy_request.grace_period_end,
        started_at: current_time,
    });

    Ok(())
}
