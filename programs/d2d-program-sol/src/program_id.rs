use anchor_lang::prelude::{pubkey, Pubkey};

cfg_if::cfg_if! {
  if #[cfg(feature = "staging")] {
    pub const PROGRAM_ID: Pubkey = pubkey!("HDxYgZcTu6snVtCEozCUkhwmmUngWEsYuNKJsvgpyL5k");
  } else if #[cfg(feature = "dev")] {
    pub const PROGRAM_ID: Pubkey = pubkey!("HDxYgZcTu6snVtCEozCUkhwmmUngWEsYuNKJsvgpyL5k");
  } else {
    pub const PROGRAM_ID: Pubkey = pubkey!("HDxYgZcTu6snVtCEozCUkhwmmUngWEsYuNKJsvgpyL5k");
  }
  // Default use for localnet
}
