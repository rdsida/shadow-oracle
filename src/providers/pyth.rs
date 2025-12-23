//! Pyth Oracle Provider
//!
//! Mock Pyth price feeds for LiteSVM testing.

use crate::{PriceConf, PriceStatus, ShadowOracleError, StandardFeeds};
use bytemuck::{Pod, Zeroable};
use litesvm::LiteSVM;
use solana_account::Account;
use solana_keypair::Keypair;
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use std::collections::HashMap;
use std::str::FromStr;

/// Pyth Oracle Program ID (mainnet)
pub const PYTH_PROGRAM_ID: &str = "FsJ3A3u2vn5cTVofAjvy6y5kwABJAqYWpe4975bi2epH";

/// Pyth magic number for V2 accounts
const PYTH_MAGIC: u32 = 0xa1b2c3d4;
/// Pyth version
const PYTH_VERSION: u32 = 2;
/// Price account type
const ACCOUNT_TYPE_PRICE: u32 = 3;

/// Price info structure (matches Pyth's PriceInfo)
#[derive(Debug, Clone, Copy, Default, Pod, Zeroable)]
#[repr(C)]
struct PriceInfo {
    price: i64,
    conf: u64,
    status: u32,
    corp_act: u32,
    pub_slot: u64,
}

/// Full Pyth price account structure
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
struct PythPriceAccount {
    magic: u32,
    ver: u32,
    atype: u32,
    size: u32,
    price_type: u32,
    expo: i32,
    num: u32,
    num_qt: u32,
    last_slot: u64,
    valid_slot: u64,
    ema_price: i64,
    ema_conf: u64,
    timestamp: i64,
    min_pub: u8,
    drv2: u8,
    drv3: i16,
    drv4: i32,
    prod: [u8; 32],
    next: [u8; 32],
    prev_slot: u64,
    prev_price: i64,
    prev_conf: u64,
    prev_timestamp: i64,
    agg: PriceInfo,
}

impl PythPriceAccount {
    const SIZE: usize = std::mem::size_of::<Self>();

    fn from_conf(conf: &PriceConf) -> Self {
        let now = conf.publish_time.unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64
        });

        Self {
            magic: PYTH_MAGIC,
            ver: PYTH_VERSION,
            atype: ACCOUNT_TYPE_PRICE,
            size: Self::SIZE as u32,
            price_type: 1,
            expo: conf.expo,
            num: 1,
            num_qt: 1,
            last_slot: 1000,
            valid_slot: 1000,
            ema_price: conf.ema_price.unwrap_or(conf.price),
            ema_conf: conf.ema_conf.unwrap_or(conf.conf),
            timestamp: now,
            min_pub: 1,
            drv2: 0,
            drv3: 0,
            drv4: 0,
            prod: [0u8; 32],
            next: [0u8; 32],
            prev_slot: 999,
            prev_price: conf.price,
            prev_conf: conf.conf,
            prev_timestamp: now - 1,
            agg: PriceInfo {
                price: conf.price,
                conf: conf.conf,
                status: pyth_status(conf.status),
                corp_act: 0,
                pub_slot: 1000,
            },
        }
    }

    fn set_price(&mut self, price: i64, conf: u64) {
        self.prev_price = self.agg.price;
        self.prev_conf = self.agg.conf;
        self.prev_timestamp = self.timestamp;
        self.prev_slot = self.last_slot;

        self.agg.price = price;
        self.agg.conf = conf;
        self.last_slot += 1;
        self.valid_slot = self.last_slot;
        self.agg.pub_slot = self.last_slot;

        self.timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        self.ema_price = (self.ema_price * 9 + price) / 10;
        self.ema_conf = (self.ema_conf * 9 + conf) / 10;
    }

    fn set_status(&mut self, status: PriceStatus) {
        self.agg.status = pyth_status(status);
    }

    fn to_bytes(&self) -> Vec<u8> {
        bytemuck::bytes_of(self).to_vec()
    }
}

fn pyth_status(status: PriceStatus) -> u32 {
    match status {
        PriceStatus::Unknown => 0,
        PriceStatus::Trading => 1,
        PriceStatus::Halted => 2,
        PriceStatus::Auction => 3,
    }
}

/// Pyth oracle provider for LiteSVM
pub struct Pyth<'a> {
    svm: &'a mut LiteSVM,
    price_feeds: HashMap<Pubkey, PythPriceAccount>,
    program_id: Pubkey,
}

impl<'a> Pyth<'a> {
    /// Create a new Pyth provider
    pub fn new(svm: &'a mut LiteSVM) -> Self {
        Self {
            svm,
            price_feeds: HashMap::new(),
            program_id: Pubkey::from_str(PYTH_PROGRAM_ID).unwrap(),
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

        let price_account = PythPriceAccount::from_conf(&conf);
        self.set_account(&pubkey, &price_account);
        self.price_feeds.insert(pubkey, price_account);

        pubkey
    }

    /// Create a price feed at a specific address
    pub fn create_price_feed_at(&mut self, address: Pubkey, conf: PriceConf) -> Pubkey {
        let price_account = PythPriceAccount::from_conf(&conf);
        self.set_account(&address, &price_account);
        self.price_feeds.insert(address, price_account);
        address
    }

    /// Update the price of an existing feed
    pub fn set_price(
        &mut self,
        feed: &Pubkey,
        price: i64,
        conf: u64,
    ) -> Result<(), ShadowOracleError> {
        let account = self
            .price_feeds
            .get_mut(feed)
            .ok_or_else(|| ShadowOracleError::PriceFeedNotFound(feed.to_string()))?;

        account.set_price(price, conf);
        let account_copy = *account;
        self.set_account(feed, &account_copy);
        Ok(())
    }

    /// Update price using human-readable USD values
    pub fn set_price_usd(
        &mut self,
        feed: &Pubkey,
        price: f64,
        confidence: f64,
    ) -> Result<(), ShadowOracleError> {
        let scale = 10f64.powi(8);
        self.set_price(feed, (price * scale) as i64, (confidence * scale) as u64)
    }

    /// Set the status of a price feed
    pub fn set_status(
        &mut self,
        feed: &Pubkey,
        status: PriceStatus,
    ) -> Result<(), ShadowOracleError> {
        let account = self
            .price_feeds
            .get_mut(feed)
            .ok_or_else(|| ShadowOracleError::PriceFeedNotFound(feed.to_string()))?;

        account.set_status(status);
        let account_copy = *account;
        self.set_account(feed, &account_copy);
        Ok(())
    }

    /// Get the current price from a feed
    pub fn get_price(&self, feed: &Pubkey) -> Option<(i64, u64)> {
        self.price_feeds
            .get(feed)
            .map(|a| (a.agg.price, a.agg.conf))
    }

    /// Get the current price in human-readable USD
    pub fn get_price_usd(&self, feed: &Pubkey) -> Option<(f64, f64)> {
        self.get_price(feed).map(|(price, conf)| {
            let scale = 10f64.powi(8);
            (price as f64 / scale, conf as f64 / scale)
        })
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
        let (current_price, current_conf) = self
            .get_price(feed)
            .ok_or_else(|| ShadowOracleError::PriceFeedNotFound(feed.to_string()))?;

        let new_price = (current_price as f64 * (1.0 - crash_percent / 100.0)) as i64;
        let new_conf = (current_conf as f64 * 5.0) as u64;

        self.set_price(feed, new_price, new_conf)
    }

    /// Simulate a depeg for stablecoins
    pub fn simulate_depeg(
        &mut self,
        feed: &Pubkey,
        new_price: f64,
    ) -> Result<(), ShadowOracleError> {
        self.set_price_usd(feed, new_price, (1.0 - new_price).abs() * 0.1 + 0.001)
    }

    fn set_account(&mut self, pubkey: &Pubkey, account: &PythPriceAccount) {
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
        let mut svm = LiteSVM::default();
        let mut pyth = Pyth::new(&mut svm);

        let feed = pyth.create_price_feed(PriceConf::new_usd(100.0, 0.1));

        let (price, conf) = pyth.get_price(&feed).unwrap();
        assert_eq!(price, 10000000000);
        assert_eq!(conf, 10000000);
    }

    #[test]
    fn test_update_price() {
        let mut svm = LiteSVM::default();
        let mut pyth = Pyth::new(&mut svm);

        let feed = pyth.create_price_feed(PriceConf::new_usd(100.0, 0.1));
        pyth.set_price_usd(&feed, 150.0, 0.2).unwrap();

        let (price, _) = pyth.get_price_usd(&feed).unwrap();
        assert!((price - 150.0).abs() < 0.001);
    }

    #[test]
    fn test_standard_feeds() {
        let mut svm = LiteSVM::default();
        let mut pyth = Pyth::new(&mut svm);

        let feeds = pyth.create_standard_feeds();

        let (sol_price, _) = pyth.get_price_usd(&feeds.sol).unwrap();
        assert!((sol_price - 100.0).abs() < 0.001);

        let (usdc_price, _) = pyth.get_price_usd(&feeds.usdc).unwrap();
        assert!((usdc_price - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_simulate_crash() {
        let mut svm = LiteSVM::default();
        let mut pyth = Pyth::new(&mut svm);

        let feed = pyth.create_price_feed(PriceConf::new_usd(100.0, 0.1));
        pyth.simulate_crash(&feed, 50.0).unwrap();

        let (price, _) = pyth.get_price_usd(&feed).unwrap();
        assert!((price - 50.0).abs() < 0.001);
    }

    #[test]
    fn test_simulate_depeg() {
        let mut svm = LiteSVM::default();
        let mut pyth = Pyth::new(&mut svm);

        let feed = pyth.create_price_feed(PriceConf::stablecoin());
        pyth.simulate_depeg(&feed, 0.95).unwrap();

        let (price, _) = pyth.get_price_usd(&feed).unwrap();
        assert!((price - 0.95).abs() < 0.001);
    }
}
