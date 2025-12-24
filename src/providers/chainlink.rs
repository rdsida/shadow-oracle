//! Chainlink Oracle Provider
//!
//! Mock Chainlink data feeds for LiteSVM testing.
//! Based on the Chainlink Solana feeds program.

use crate::{PriceConf, ShadowOracleError, StandardFeeds};
use litesvm::LiteSVM;
use solana_account::Account;
use solana_clock::Clock;
use solana_keypair::Keypair;
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use std::collections::HashMap;
use std::str::FromStr;

/// Chainlink Solana Program ID (mainnet)
pub const CHAINLINK_PROGRAM_ID: &str = "HEvSKofvBgfaexv23kMabbYqxasxU3mQ4ibBMEmJWHny";

/// Chainlink Store Program ID
pub const CHAINLINK_STORE_PROGRAM_ID: &str = "CaH12fwNTKJAG8PxEvo9R96Zc2j8Jq3Q5K9B7tTFQ2by";

/// Chainlink feed data - manually serialized
#[derive(Debug, Clone)]
struct ChainlinkFeed {
    price: f64,
    decimals: u8,
    slot: u64,
    timestamp: u32,
    round_id: u32,
}

impl ChainlinkFeed {
    fn from_conf(conf: &PriceConf, clock: &Clock) -> Self {
        let now = conf.publish_time.unwrap_or(clock.unix_timestamp);

        Self {
            price: conf.price_usd(),
            decimals: conf.decimals,
            slot: clock.slot,
            timestamp: now as u32,
            round_id: 1,
        }
    }

    fn set_price(&mut self, price: f64, clock: &Clock) {
        self.price = price;
        self.slot = clock.slot;
        self.round_id += 1;
        self.timestamp = clock.unix_timestamp as u32;
    }

    fn get_answer(&self) -> i128 {
        let scale = 10i128.pow(self.decimals as u32);
        (self.price * scale as f64) as i128
    }

    /// Serialize to Chainlink-compatible format
    fn to_bytes(&self) -> Vec<u8> {
        // Simplified Chainlink feed account structure
        // Based on chainlink-solana transmissions account
        const HEADER_SIZE: usize = 192;
        const TRANSMISSION_SIZE: usize = 48;
        const NUM_TRANSMISSIONS: usize = 16;
        let account_size = HEADER_SIZE + (TRANSMISSION_SIZE * NUM_TRANSMISSIONS);

        let mut data = vec![0u8; account_size];

        // Header
        // version (1 byte)
        data[0] = 1;
        // state (1 byte) - 1 = initialized
        data[1] = 1;

        // owner (32 bytes) at offset 2
        // proposed_owner (32 bytes) at offset 34
        // writer (32 bytes) at offset 66
        // description (32 bytes) at offset 98

        // decimals (1 byte) at offset 130
        data[130] = self.decimals;

        // flagging_threshold (4 bytes) at offset 131
        data[131..135].copy_from_slice(&1000u32.to_le_bytes());

        // latest_round_id (4 bytes) at offset 135
        data[135..139].copy_from_slice(&self.round_id.to_le_bytes());

        // granularity (1 byte) at offset 141
        data[141] = 1;

        // live_length (4 bytes) at offset 142
        data[142..146].copy_from_slice(&(NUM_TRANSMISSIONS as u32).to_le_bytes());

        // live_cursor (4 bytes) at offset 150
        let cursor = (self.round_id - 1) % NUM_TRANSMISSIONS as u32;
        data[150..154].copy_from_slice(&cursor.to_le_bytes());

        // Transmissions start at offset HEADER_SIZE
        // Each transmission: slot (8), timestamp (4), padding (4), answer (16), obs_count (1), observer_count (1), padding (14)
        let tx_offset = HEADER_SIZE + (cursor as usize * TRANSMISSION_SIZE);

        // slot
        data[tx_offset..tx_offset + 8].copy_from_slice(&self.slot.to_le_bytes());
        // timestamp
        data[tx_offset + 8..tx_offset + 12].copy_from_slice(&self.timestamp.to_le_bytes());
        // answer (i128)
        let answer = self.get_answer();
        data[tx_offset + 16..tx_offset + 32].copy_from_slice(&answer.to_le_bytes());
        // observations_count
        data[tx_offset + 32] = 3;
        // observer_count
        data[tx_offset + 33] = 3;

        data
    }
}

/// Chainlink oracle provider for LiteSVM
pub struct Chainlink<'a> {
    svm: &'a mut LiteSVM,
    price_feeds: HashMap<Pubkey, ChainlinkFeed>,
    program_id: Pubkey,
}

impl<'a> Chainlink<'a> {
    /// Create a new Chainlink provider
    pub fn new(svm: &'a mut LiteSVM) -> Self {
        Self {
            svm,
            price_feeds: HashMap::new(),
            program_id: Pubkey::from_str(CHAINLINK_PROGRAM_ID).unwrap(),
        }
    }

    /// Create with a custom program ID
    pub fn with_program_id(svm: &'a mut LiteSVM, program_id: Pubkey) -> Self {
        Self {
            svm,
            price_feeds: HashMap::new(),
            program_id,
        }
    }

    /// Create a new price feed account
    pub fn create_price_feed(&mut self, conf: PriceConf) -> Pubkey {
        let keypair = Keypair::new();
        let pubkey = keypair.pubkey();

        let clock = self.svm.get_sysvar::<Clock>();
        let feed = ChainlinkFeed::from_conf(&conf, &clock);
        self.set_account(&pubkey, &feed);
        self.price_feeds.insert(pubkey, feed);

        pubkey
    }

    /// Create a price feed at a specific address
    pub fn create_price_feed_at(&mut self, address: Pubkey, conf: PriceConf) -> Pubkey {
        let clock = self.svm.get_sysvar::<Clock>();
        let feed = ChainlinkFeed::from_conf(&conf, &clock);
        self.set_account(&address, &feed);
        self.price_feeds.insert(address, feed);
        address
    }

    /// Update the price of an existing feed
    pub fn set_price(&mut self, feed: &Pubkey, price: f64) -> Result<(), ShadowOracleError> {
        let clock = self.svm.get_sysvar::<Clock>();
        let account = self
            .price_feeds
            .get_mut(feed)
            .ok_or_else(|| ShadowOracleError::PriceFeedNotFound(feed.to_string()))?;

        account.set_price(price, &clock);
        let account_clone = account.clone();
        self.set_account(feed, &account_clone);
        Ok(())
    }

    /// Alias for set_price with USD naming convention (Chainlink doesn't have confidence)
    pub fn set_price_usd(
        &mut self,
        feed: &Pubkey,
        price: f64,
        _confidence: f64, // ignored, Chainlink doesn't use confidence
    ) -> Result<(), ShadowOracleError> {
        self.set_price(feed, price)
    }

    /// Get the current price from a feed
    pub fn get_price(&self, feed: &Pubkey) -> Option<f64> {
        self.price_feeds.get(feed).map(|a| a.price)
    }

    /// Get price in USD format (returns (price, 0.0) for API compatibility)
    pub fn get_price_usd(&self, feed: &Pubkey) -> Option<(f64, f64)> {
        self.get_price(feed).map(|p| (p, 0.0))
    }

    /// Get the raw answer (scaled integer)
    pub fn get_latest_answer(&self, feed: &Pubkey) -> Option<i128> {
        self.price_feeds.get(feed).map(|a| a.get_answer())
    }

    /// Get decimals for a feed
    pub fn get_decimals(&self, feed: &Pubkey) -> Option<u8> {
        self.price_feeds.get(feed).map(|a| a.decimals)
    }

    /// Get the latest round ID
    pub fn get_latest_round(&self, feed: &Pubkey) -> Option<u32> {
        self.price_feeds.get(feed).map(|a| a.round_id)
    }

    /// Create standard price feeds for common assets
    pub fn create_standard_feeds(&mut self) -> StandardFeeds {
        StandardFeeds {
            sol: self.create_price_feed(PriceConf::new_usd(100.0, 0.1)),
            btc: self.create_price_feed(PriceConf::new_usd(43000.0, 10.0)),
            eth: self.create_price_feed(PriceConf::new_usd(2200.0, 1.0)),
            usdc: self.create_price_feed(PriceConf::stablecoin()),
            usdt: self.create_price_feed(PriceConf::stablecoin()),
        }
    }

    /// Simulate a price crash
    pub fn simulate_crash(
        &mut self,
        feed: &Pubkey,
        crash_percent: f64,
    ) -> Result<(), ShadowOracleError> {
        let current_price = self
            .get_price(feed)
            .ok_or_else(|| ShadowOracleError::PriceFeedNotFound(feed.to_string()))?;

        let new_price = current_price * (1.0 - crash_percent / 100.0);
        self.set_price(feed, new_price)
    }

    /// Simulate a depeg for stablecoins
    pub fn simulate_depeg(
        &mut self,
        feed: &Pubkey,
        new_price: f64,
    ) -> Result<(), ShadowOracleError> {
        self.set_price(feed, new_price)
    }

    fn set_account(&mut self, pubkey: &Pubkey, account: &ChainlinkFeed) {
        let data = account.to_bytes();

        self.svm
            .set_account(
                *pubkey,
                Account {
                    lamports: 1_000_000_000,
                    data,
                    owner: self.program_id,
                    executable: false,
                    rent_epoch: 0,
                },
            )
            .expect("Failed to set account");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_price_feed() {
        let mut svm = LiteSVM::new().with_sysvars();
        let mut cl = Chainlink::new(&mut svm);

        let feed = cl.create_price_feed(PriceConf::new_usd(100.0, 0.1));

        let price = cl.get_price(&feed).unwrap();
        assert!((price - 100.0).abs() < 0.001);
    }

    #[test]
    fn test_update_price() {
        let mut svm = LiteSVM::new().with_sysvars();
        let mut cl = Chainlink::new(&mut svm);

        let feed = cl.create_price_feed(PriceConf::new_usd(100.0, 0.1));
        cl.set_price(&feed, 150.0).unwrap();

        let price = cl.get_price(&feed).unwrap();
        assert!((price - 150.0).abs() < 0.001);
    }

    #[test]
    fn test_round_increment() {
        let mut svm = LiteSVM::new().with_sysvars();
        let mut cl = Chainlink::new(&mut svm);

        let feed = cl.create_price_feed(PriceConf::new_usd(100.0, 0.1));
        assert_eq!(cl.get_latest_round(&feed), Some(1));

        cl.set_price(&feed, 110.0).unwrap();
        assert_eq!(cl.get_latest_round(&feed), Some(2));

        cl.set_price(&feed, 120.0).unwrap();
        assert_eq!(cl.get_latest_round(&feed), Some(3));
    }

    #[test]
    fn test_standard_feeds() {
        let mut svm = LiteSVM::new().with_sysvars();
        let mut cl = Chainlink::new(&mut svm);

        let feeds = cl.create_standard_feeds();

        let sol_price = cl.get_price(&feeds.sol).unwrap();
        assert!((sol_price - 100.0).abs() < 0.001);

        let usdc_price = cl.get_price(&feeds.usdc).unwrap();
        assert!((usdc_price - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_simulate_crash() {
        let mut svm = LiteSVM::new().with_sysvars();
        let mut cl = Chainlink::new(&mut svm);

        let feed = cl.create_price_feed(PriceConf::new_usd(100.0, 0.1));
        cl.simulate_crash(&feed, 50.0).unwrap();

        let price = cl.get_price(&feed).unwrap();
        assert!((price - 50.0).abs() < 0.001);
    }

    #[test]
    fn test_decimals() {
        let mut svm = LiteSVM::new().with_sysvars();
        let mut cl = Chainlink::new(&mut svm);

        let conf = PriceConf::new_usd(100.0, 0.1).with_decimals(6);
        let feed = cl.create_price_feed(conf);

        assert_eq!(cl.get_decimals(&feed), Some(6));

        let answer = cl.get_latest_answer(&feed).unwrap();
        assert_eq!(answer, 100_000_000); // 100 * 10^6
    }
}
