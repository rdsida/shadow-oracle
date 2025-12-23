//! Oracle provider implementations

#[cfg(feature = "pyth")]
pub mod pyth;

#[cfg(feature = "switchboard")]
pub mod switchboard;

#[cfg(feature = "chainlink")]
pub mod chainlink;
