use anchor_lang::prelude::*;

/// State to track programs managed by D2D Protocol
/// This enables PDA-based authority proxy for trustless upgrades
#[account]
#[derive(InitSpace)]
pub struct ManagedProgram {
    /// The deployed program ID being managed
    pub program_id: Pubkey,
    
    /// Developer who owns the program (can upgrade)
    pub developer: Pubkey,
    
    /// Link to the associated DeployRequest
    pub deploy_request: Pubkey,
    
    /// PDA that holds the upgrade authority for this program
    /// Derived as: [b"program_authority", program_id]
    pub authority_pda: Pubkey,
    
    /// When authority was transferred to PDA
    pub created_at: i64,
    
    /// Last time program was upgraded via proxy
    pub last_upgraded_at: i64,
    
    /// Total number of upgrades performed
    pub upgrade_count: u32,
    
    /// Whether this managed program is still active
    pub is_active: bool,
    
    /// PDA bump seed
    pub bump: u8,
}

impl ManagedProgram {
    pub const PREFIX_SEED: &'static [u8] = b"managed_program";
    pub const AUTHORITY_SEED: &'static [u8] = b"program_authority";
    
    /// Check if program can be upgraded (developer owns it and it's active)
    pub fn can_upgrade(&self, developer: &Pubkey) -> bool {
        self.is_active && self.developer == *developer
    }
}
