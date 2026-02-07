use crate::errors::ErrorCode;
use crate::events::DeploymentFailed;
use crate::states::{DeployRequest, DeployRequestStatus, TreasuryPool};
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct ForceResetDeployment<'info> {
    #[account(
        mut,
        seeds = [TreasuryPool::PREFIX_SEED],
        bump = treasury_pool.bump
    )]
    pub treasury_pool: Account<'info, TreasuryPool>,
    
    #[account(
        mut,
        seeds = [DeployRequest::PREFIX_SEED, deploy_request.program_hash.as_ref()],
        bump = deploy_request.bump
    )]
    pub deploy_request: Account<'info, DeployRequest>,
    
    #[account(
        mut,
        constraint = admin.key() == treasury_pool.admin @ ErrorCode::Unauthorized
    )]
    pub admin: Signer<'info>,
}

pub fn force_reset_deployment(
    ctx: Context<ForceResetDeployment>,
) -> Result<()> {
    let deploy_request = &mut ctx.accounts.deploy_request;
    
    // Status must be resettable or hung
    // We allow force reset for any status if admin deems it necessary
    
    let previous_status = deploy_request.status.clone();
    deploy_request.status = DeployRequestStatus::Failed;
    deploy_request.ephemeral_key = None; // Critical: clear the key that was blocking reset
    
    msg!("[FORCE_RESET] Reset deployment for hash {:?}", deploy_request.program_hash);
    msg!("  Previous status: {:?}", previous_status);
    
    emit!(DeploymentFailed {
        request_id: deploy_request.request_id,
        developer: deploy_request.developer,
        failure_reason: "Force reset by admin".to_string(),
        refund_amount: 0, // No automatic refund in force reset
        deployment_cost_returned: 0, // Admin must manually recover SOL from ephemeral if known
        failed_at: Clock::get()?.unix_timestamp,
    });

    Ok(())
}
