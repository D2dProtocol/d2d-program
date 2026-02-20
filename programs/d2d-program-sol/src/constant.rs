pub mod admin_mod {
  #[cfg(feature = "staging")]
  pub const ADMIN_SYSTEM_PUBKEY: &str = "opty8HWBKX3wW8c9qMPkmB4xnrCpMWWmQwqq7yGzmr4";

  #[cfg(feature = "dev")]
  pub const ADMIN_SYSTEM_PUBKEY: &str = "opty8HWBKX3wW8c9qMPkmB4xnrCpMWWmQwqq7yGzmr4";

  #[cfg(all(not(feature = "staging"), not(feature = "dev")))]
  pub const ADMIN_SYSTEM_PUBKEY: &str = "";
}
