//! Switchboard Oracle Provider
//!
//! Mock Switchboard V2 aggregator feeds for LiteSVM testing.

use crate::{PriceConf, ShadowOracleError, StandardFeeds};
use litesvm::LiteSVM;
use solana_account::Account;
use solana_clock::Clock;
use solana_keypair::Keypair;
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use std::collections::HashMap;
use std::str::FromStr;

/// Switchboard V2 Program ID (mainnet)
pub const SWITCHBOARD_PROGRAM_ID: &str = "SW1TCH7qEPTdLsDHRgPuMQjbQxKdH2aBStViMFnt64f";

/// Switchboard On-Demand Program ID
pub const SWITCHBOARD_ON_DEMAND_PROGRAM_ID: &str = "SBondMDrcV3K4kxZR1HNVT7osZxAHVHgYXL5Ze1oMUv";

/// Discriminator for AggregatorAccountData
const AGGREGATOR_DISCRIMINATOR: [u8; 8] = [217, 230, 65, 101, 201, 162, 27, 125];

/// Switchboard aggregator data - manually serialized to avoid Pod issues
#[derive(Debug, Clone)]
struct SwitchboardAggregator {
    price: f64,
    std_deviation: f64,
    decimals: u8,
    slot: u64,
    timestamp: i64,
    round_id: u32,
}

impl SwitchboardAggregator {
    fn from_conf(conf: &PriceConf, clock: &Clock) -> Self {
        let now = conf.publish_time.unwrap_or(clock.unix_timestamp);

        Self {
            price: conf.price_usd(),
            std_deviation: conf.conf_usd(),
            decimals: conf.decimals,
            slot: clock.slot,
            timestamp: now,
            round_id: 1,
        }
    }

    fn set_price(&mut self, price: f64, std_dev: f64, clock: &Clock) {
        self.price = price;
        self.std_deviation = std_dev;
        self.slot = clock.slot;
        self.round_id += 1;
        self.timestamp = clock.unix_timestamp;
    }

    /// Serialize to Switchboard-compatible format
    /// We create a minimal account that Switchboard SDK can read
    fn to_bytes(&self) -> Vec<u8> {
        // Account size based on Switchboard V2 AggregatorAccountData
        // We only populate the fields needed for price reading
        const ACCOUNT_SIZE: usize = 3851; // Actual Switchboard aggregator size

        let mut data = vec![0u8; ACCOUNT_SIZE];

        // Discriminator (offset 0)
        data[0..8].copy_from_slice(&AGGREGATOR_DISCRIMINATOR);

        // Skip to latest_confirmed_round (offset varies, ~1144)
        // AggregatorRound starts with: num_success (4), num_error (4), is_closed (1)
        // then round_open_slot (8), round_open_timestamp (8)
        // then result as SwitchboardDecimal (mantissa i128 = 16, scale u32 = 4, padding = 12)
        // Total SwitchboardDecimal = 32 bytes
        let round_offset = 1144;

        // num_success
        data[round_offset..round_offset + 4].copy_from_slice(&3u32.to_le_bytes());
        // num_error
        data[round_offset + 4..round_offset + 8].copy_from_slice(&0u32.to_le_bytes());
        // is_closed
        data[round_offset + 8] = 1;
        // round_open_slot
        data[round_offset + 9..round_offset + 17].copy_from_slice(&self.slot.to_le_bytes());
        // round_open_timestamp
        data[round_offset + 17..round_offset + 25].copy_from_slice(&self.timestamp.to_le_bytes());

        // Result as SwitchboardDecimal
        // mantissa = price * 10^scale
        let scale = self.decimals as u32;
        let multiplier = 10f64.powi(scale as i32);
        let mantissa = (self.price * multiplier) as i128;

        let result_offset = round_offset + 25;
        data[result_offset..result_offset + 16].copy_from_slice(&mantissa.to_le_bytes());
        data[result_offset + 16..result_offset + 20].copy_from_slice(&scale.to_le_bytes());

        // std_deviation as SwitchboardDecimal
        let std_mantissa = (self.std_deviation * multiplier) as i128;
        let std_offset = result_offset + 32;
        data[std_offset..std_offset + 16].copy_from_slice(&std_mantissa.to_le_bytes());
        data[std_offset + 16..std_offset + 20].copy_from_slice(&scale.to_le_bytes());

        data
    }
}

/// Switchboard oracle provider for LiteSVM
pub struct Switchboard<'a> {
    svm: &'a mut LiteSVM,
    price_feeds: HashMap<Pubkey, SwitchboardAggregator>,
    program_id: Pubkey,
}

impl<'a> Switchboard<'a> {
    /// Create a new Switchboard provider
    pub fn new(svm: &'a mut LiteSVM) -> Self {
        Self {
            svm,
            price_feeds: HashMap::new(),
            program_id: Pubkey::from_str(SWITCHBOARD_PROGRAM_ID).unwrap(),
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

    /// Create a new price feed (aggregator) account
    pub fn create_price_feed(&mut self, conf: PriceConf) -> Pubkey {
        let keypair = Keypair::new();
        let pubkey = keypair.pubkey();

        let clock = self.svm.get_sysvar::<Clock>();
        let aggregator = SwitchboardAggregator::from_conf(&conf, &clock);
        self.set_account(&pubkey, &aggregator);
        self.price_feeds.insert(pubkey, aggregator);

        pubkey
    }

    /// Create a price feed at a specific address
    pub fn create_price_feed_at(&mut self, address: Pubkey, conf: PriceConf) -> Pubkey {
        let clock = self.svm.get_sysvar::<Clock>();
        let aggregator = SwitchboardAggregator::from_conf(&conf, &clock);
        self.set_account(&address, &aggregator);
        self.price_feeds.insert(address, aggregator);
        address
    }

    /// Update the price of an existing feed
    pub fn set_price(
        &mut self,
        feed: &Pubkey,
        price: f64,
        std_dev: f64,
    ) -> Result<(), ShadowOracleError> {
        let clock = self.svm.get_sysvar::<Clock>();
        let account = self
            .price_feeds
            .get_mut(feed)
            .ok_or_else(|| ShadowOracleError::PriceFeedNotFound(feed.to_string()))?;

        account.set_price(price, std_dev, &clock);
        let account_clone = account.clone();
        self.set_account(feed, &account_clone);
        Ok(())
    }

    /// Alias for set_price with USD naming convention
    pub fn set_price_usd(
        &mut self,
        feed: &Pubkey,
        price: f64,
        std_dev: f64,
    ) -> Result<(), ShadowOracleError> {
        self.set_price(feed, price, std_dev)
    }

    /// Get the current price from a feed
    pub fn get_price(&self, feed: &Pubkey) -> Option<(f64, f64)> {
        self.price_feeds
            .get(feed)
            .map(|a| (a.price, a.std_deviation))
    }

    /// Alias for get_price (already in USD)
    pub fn get_price_usd(&self, feed: &Pubkey) -> Option<(f64, f64)> {
        self.get_price(feed)
    }

    /// Get the timestamp of the last price update
    pub fn get_timestamp(&self, feed: &Pubkey) -> Option<i64> {
        self.price_feeds.get(feed).map(|a| a.timestamp)
    }

    /// Get the slot of the last price update
    pub fn get_slot(&self, feed: &Pubkey) -> Option<u64> {
        self.price_feeds.get(feed).map(|a| a.slot)
    }

    /// Make an existing feed stale by setting its timestamp to `seconds_ago` in the past
    ///
    /// This is useful for testing staleness checks without changing the price.
    pub fn make_stale(&mut self, feed: &Pubkey, seconds_ago: i64) -> Result<(), ShadowOracleError> {
        let clock = self.svm.get_sysvar::<Clock>();
        let stale_timestamp = clock.unix_timestamp - seconds_ago;

        let account = self
            .price_feeds
            .get_mut(feed)
            .ok_or_else(|| ShadowOracleError::PriceFeedNotFound(feed.to_string()))?;

        account.timestamp = stale_timestamp;

        let account_clone = account.clone();
        self.set_account(feed, &account_clone);
        Ok(())
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
        let (current_price, current_std) = self
            .get_price(feed)
            .ok_or_else(|| ShadowOracleError::PriceFeedNotFound(feed.to_string()))?;

        let new_price = current_price * (1.0 - crash_percent / 100.0);
        let new_std = current_std * 5.0;

        self.set_price(feed, new_price, new_std)
    }

    /// Simulate a depeg for stablecoins
    pub fn simulate_depeg(
        &mut self,
        feed: &Pubkey,
        new_price: f64,
    ) -> Result<(), ShadowOracleError> {
        self.set_price(feed, new_price, (1.0 - new_price).abs() * 0.1 + 0.001)
    }

    fn set_account(&mut self, pubkey: &Pubkey, account: &SwitchboardAggregator) {
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
        let mut sb = Switchboard::new(&mut svm);

        let feed = sb.create_price_feed(PriceConf::new_usd(100.0, 0.1));

        let (price, _) = sb.get_price(&feed).unwrap();
        assert!((price - 100.0).abs() < 0.001);
    }

    #[test]
    fn test_update_price() {
        let mut svm = LiteSVM::new().with_sysvars();
        let mut sb = Switchboard::new(&mut svm);

        let feed = sb.create_price_feed(PriceConf::new_usd(100.0, 0.1));
        sb.set_price_usd(&feed, 150.0, 0.2).unwrap();

        let (price, _) = sb.get_price_usd(&feed).unwrap();
        assert!((price - 150.0).abs() < 0.001);
    }

    #[test]
    fn test_standard_feeds() {
        let mut svm = LiteSVM::new().with_sysvars();
        let mut sb = Switchboard::new(&mut svm);

        let feeds = sb.create_standard_feeds();

        let (sol_price, _) = sb.get_price_usd(&feeds.sol).unwrap();
        assert!((sol_price - 100.0).abs() < 0.001);
    }

    #[test]
    fn test_simulate_crash() {
        let mut svm = LiteSVM::new().with_sysvars();
        let mut sb = Switchboard::new(&mut svm);

        let feed = sb.create_price_feed(PriceConf::new_usd(100.0, 0.1));
        sb.simulate_crash(&feed, 50.0).unwrap();

        let (price, _) = sb.get_price_usd(&feed).unwrap();
        assert!((price - 50.0).abs() < 0.001);
    }

    #[test]
    fn test_timestamp_uses_svm_clock() {
        let mut svm = LiteSVM::new().with_sysvars();

        let initial_clock = svm.get_sysvar::<Clock>();
        let initial_timestamp = initial_clock.unix_timestamp;

        let mut sb = Switchboard::new(&mut svm);
        let feed = sb.create_price_feed(PriceConf::new_usd(100.0, 0.1));

        let feed_timestamp = sb.get_timestamp(&feed).unwrap();
        assert_eq!(feed_timestamp, initial_timestamp);
    }

    #[test]
    fn test_slot_uses_svm_clock() {
        let mut svm = LiteSVM::new().with_sysvars();

        let initial_clock = svm.get_sysvar::<Clock>();
        let initial_slot = initial_clock.slot;

        let mut sb = Switchboard::new(&mut svm);
        let feed = sb.create_price_feed(PriceConf::new_usd(100.0, 0.1));

        let feed_slot = sb.get_slot(&feed).unwrap();
        assert_eq!(feed_slot, initial_slot);
    }

    #[test]
    fn test_make_stale() {
        let mut svm = LiteSVM::new().with_sysvars();

        let clock = svm.get_sysvar::<Clock>();
        let current_time = clock.unix_timestamp;

        let mut sb = Switchboard::new(&mut svm);
        let feed = sb.create_price_feed(PriceConf::new_usd(100.0, 0.1));

        // Make the feed 5 minutes stale
        sb.make_stale(&feed, 300).unwrap();

        let feed_timestamp = sb.get_timestamp(&feed).unwrap();
        assert_eq!(feed_timestamp, current_time - 300);
    }

    #[test]
    fn test_create_stale_feed_with_stale_by() {
        let mut svm = LiteSVM::new().with_sysvars();

        let clock = svm.get_sysvar::<Clock>();
        let current_time = clock.unix_timestamp;

        let mut sb = Switchboard::new(&mut svm);

        // Create a feed that's already 5 minutes old
        let stale_conf = PriceConf::new_usd(100.0, 0.1).stale_by(300, current_time);
        let feed = sb.create_price_feed(stale_conf);

        let feed_timestamp = sb.get_timestamp(&feed).unwrap();
        assert_eq!(feed_timestamp, current_time - 300);
    }
}
