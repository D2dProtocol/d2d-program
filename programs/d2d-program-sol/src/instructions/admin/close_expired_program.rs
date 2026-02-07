use anchor_lang::prelude::*;

use crate::errors::ErrorCode;
use crate::events::{GracePeriodEnded, ProgramClosedAfterGrace};
use crate::states::{DeployRequest, DeployRequestStatus, ManagedProgram, TreasuryPool};

#[derive(Accounts)]
#[instruction(request_id: [u8; 32])]
pub struct CloseExpiredProgram<'info> {
    #[account(
        mut,
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
        mut,
        seeds = [ManagedProgram::PREFIX_SEED, managed_program.program_id.as_ref()],
        bump = managed_program.bump,
        constraint = managed_program.deploy_request == deploy_request.key() @ ErrorCode::InvalidRequestId
    )]
    pub managed_program: Account<'info, ManagedProgram>,

    #[account(
        constraint = treasury_pool.is_admin(&admin.key()) @ ErrorCode::Unauthorized
    )]
    pub admin: Signer<'info>,
}

pub fn close_expired_program(ctx: Context<CloseExpiredProgram>, request_id: [u8; 32]) -> Result<()> {
    let treasury_pool = &mut ctx.accounts.treasury_pool;
    let deploy_request = &mut ctx.accounts.deploy_request;
    let managed_program = &mut ctx.accounts.managed_program;

    require!(!treasury_pool.emergency_pause, ErrorCode::ProgramPaused);

    // Verify program is in grace period
    require!(
        deploy_request.status == DeployRequestStatus::InGracePeriod,
        ErrorCode::NotInGracePeriod
    );

    // Verify grace period has expired
    require!(
        deploy_request.is_grace_period_expired()?,
        ErrorCode::GracePeriodNotExpired
    );

    let current_time = Clock::get()?.unix_timestamp;
    let program_id = managed_program.program_id;

    // Update deploy request status
    deploy_request.status = DeployRequestStatus::Closed;

    // Mark managed program as inactive
    managed_program.is_active = false;

    // Emit grace period ended event
    emit!(GracePeriodEnded {
        request_id,
        developer: deploy_request.developer,
        action: "closed".to_string(),
        ended_at: current_time,
    });

    // Emit program closed event
    emit!(ProgramClosedAfterGrace {
        request_id,
        developer: deploy_request.developer,
        program_id,
        grace_period_days: deploy_request.grace_period_days,
        closed_at: current_time,
    });

    // Note: Actual program rent reclamation is handled by reclaim_program_rent instruction
    // which uses BPF Loader's close_any instruction

    Ok(())
}
