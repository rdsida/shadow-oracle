//! # Shadow Oracle
//!
//! Mock oracles for LiteSVM testing. Create and manipulate price feeds
//! for Pyth, Switchboard, and Chainlink - instantly without network calls.
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use litesvm::LiteSVM;
//! use shadow_oracle::{ShadowOracle, PriceConf};
//!
//! let mut svm = LiteSVM::default();
//! let mut oracle = ShadowOracle::new(&mut svm);
//!
//! // Create a Pyth price feed at $100
//! let sol_feed = oracle.pyth().create_price_feed(PriceConf::new_usd(100.0, 0.1));
//!
//! // Create a Switchboard feed
//! let btc_feed = oracle.switchboard().create_price_feed(PriceConf::new_usd(43000.0, 10.0));
//!
//! // Create a Chainlink feed
//! let eth_feed = oracle.chainlink().create_price_feed(PriceConf::new_usd(2200.0, 1.0));
//!
//! // Update prices
//! oracle.pyth().set_price_usd(&sol_feed, 150.0, 0.2).unwrap();
//!
//! // Simulate market events
//! oracle.pyth().simulate_crash(&sol_feed, 50.0).unwrap();
//! ```
//!
//! ## Individual Providers
//!
//! You can also use providers directly:
//!
//! ```rust,ignore
//! use litesvm::LiteSVM;
//! use shadow_oracle::{Pyth, Switchboard, Chainlink, PriceConf};
//!
//! let mut svm = LiteSVM::default();
//! let mut pyth = Pyth::new(&mut svm);
//! let feed = pyth.create_price_feed(PriceConf::new_usd(100.0, 0.1));
//! ```

mod error;
mod price;
pub mod providers;

pub use error::*;
pub use price::*;
pub use providers::chainlink::Chainlink;
pub use providers::pyth::Pyth;
pub use providers::switchboard::Switchboard;

use litesvm::LiteSVM;

/// Main entry point for shadow oracles
///
/// Provides access to all oracle providers through a single interface.
pub struct ShadowOracle<'a> {
    svm: &'a mut LiteSVM,
}

impl<'a> ShadowOracle<'a> {
    /// Create a new ShadowOracle instance
    pub fn new(svm: &'a mut LiteSVM) -> Self {
        Self { svm }
    }

    /// Get a Pyth oracle provider
    pub fn pyth(&mut self) -> Pyth<'_> {
        Pyth::new(self.svm)
    }

    /// Get a Switchboard oracle provider
    pub fn switchboard(&mut self) -> Switchboard<'_> {
        Switchboard::new(self.svm)
    }

    /// Get a Chainlink oracle provider
    pub fn chainlink(&mut self) -> Chainlink<'_> {
        Chainlink::new(self.svm)
    }
}

/// Known mainnet price feed addresses
pub mod feeds {
    pub mod pyth {
        use solana_pubkey::Pubkey;
        use std::str::FromStr;

        pub fn sol_usd() -> Pubkey {
            Pubkey::from_str("H6ARHf6YXhGYeQfUzQNGk6rDNnLBQKrenN712K4AQJEG").unwrap()
        }

        pub fn btc_usd() -> Pubkey {
            Pubkey::from_str("GVXRSBjFk6e6J3NbVPXohDJetcTjaeeuykUpbQF8UoMU").unwrap()
        }

        pub fn eth_usd() -> Pubkey {
            Pubkey::from_str("JBu1AL4obBcCMqKBBxhpWCNUt136ijcuMZLFvTP7iWdB").unwrap()
        }

        pub fn usdc_usd() -> Pubkey {
            Pubkey::from_str("Gnt27xtC473ZT2Mw5u8wZ68Z3gULkSTb5DuxJy7eJotD").unwrap()
        }

        pub fn usdt_usd() -> Pubkey {
            Pubkey::from_str("3vxLXJqLqF3JG5TCbYycbKWRBbCJQLxQmBGCkyqEEefL").unwrap()
        }
    }

    pub mod switchboard {
        use solana_pubkey::Pubkey;
        use std::str::FromStr;

        pub fn sol_usd() -> Pubkey {
            Pubkey::from_str("GvDMxPzN1sCj7L26YDK2HnMRXEQmQ2aemov8YBtPS7vR").unwrap()
        }

        pub fn btc_usd() -> Pubkey {
            Pubkey::from_str("8SXvChNYFhRq4EZuZvnhjrB3jJRQCv4k3P4W6hesH3Ee").unwrap()
        }

        pub fn eth_usd() -> Pubkey {
            Pubkey::from_str("HNStfhaLnqwF2ZtJUizaA9uHDAVB976r2AgTUx9LrdEo").unwrap()
        }
    }

    pub mod chainlink {
        use solana_pubkey::Pubkey;
        use std::str::FromStr;

        pub fn sol_usd() -> Pubkey {
            Pubkey::from_str("CcPVS9bqyXbD9cLnTbhhHazLsrua8QMFUHTutPtjyDzq").unwrap()
        }

        pub fn btc_usd() -> Pubkey {
            Pubkey::from_str("CGmWwBNsTRDENT5gmVZzRu38GnNnMm1K5C3sFiUUyYQX").unwrap()
        }

        pub fn eth_usd() -> Pubkey {
            Pubkey::from_str("5JcBbyiwxPxFMvNJHLxLqg5LPZeC4sC3VdWFfaKManYm").unwrap()
        }
    }
}
