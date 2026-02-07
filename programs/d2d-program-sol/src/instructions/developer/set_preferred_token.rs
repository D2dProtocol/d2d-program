use anchor_lang::prelude::*;

use crate::errors::ErrorCode;
use crate::events::AutoRenewSettingsChanged;
use crate::states::{DeveloperEscrow, TokenType, TreasuryPool};

#[derive(Accounts)]
pub struct SetPreferredToken<'info> {
    #[account(
        seeds = [TreasuryPool::PREFIX_SEED],
        bump = treasury_pool.bump
    )]
    pub treasury_pool: Account<'info, TreasuryPool>,

    #[account(
        mut,
        seeds = [DeveloperEscrow::PREFIX_SEED, developer.key().as_ref()],
        bump = developer_escrow.bump,
        constraint = developer_escrow.developer == developer.key() @ ErrorCode::Unauthorized
    )]
    pub developer_escrow: Account<'info, DeveloperEscrow>,

    #[account(mut)]
    pub developer: Signer<'info>,
}

pub fn set_preferred_token(ctx: Context<SetPreferredToken>, token_type: u8) -> Result<()> {
    let treasury_pool = &ctx.accounts.treasury_pool;
    let developer_escrow = &mut ctx.accounts.developer_escrow;
    let developer = &ctx.accounts.developer;

    require!(!treasury_pool.emergency_pause, ErrorCode::ProgramPaused);
    require!(token_type <= 2, ErrorCode::InvalidTokenType); // 0=SOL, 1=USDC, 2=USDT

    let preferred_token = match token_type {
        0 => TokenType::SOL,
        1 => TokenType::USDC,
        2 => TokenType::USDT,
        _ => return Err(ErrorCode::InvalidTokenType.into()),
    };

    developer_escrow.preferred_token = preferred_token;

    emit!(AutoRenewSettingsChanged {
        developer: developer.key(),
        auto_renew_enabled: developer_escrow.auto_renew_enabled,
        preferred_token: token_type,
        changed_at: Clock::get()?.unix_timestamp,
    });

    Ok(())
}
