//! Common price types shared across all oracle providers

/// Price status values (compatible across providers)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PriceStatus {
    #[default]
    Trading,
    Halted,
    Unknown,
    Auction,
}

/// Configuration for creating a price feed
///
/// This is provider-agnostic and gets converted to the appropriate
/// on-chain format by each provider.
#[derive(Debug, Clone)]
pub struct PriceConf {
    /// Price value (scaled by 10^|expo|)
    pub price: i64,
    /// Confidence interval (scaled by 10^|expo|)
    pub conf: u64,
    /// Price exponent (typically -8 for USD prices)
    pub expo: i32,
    /// EMA price (defaults to price if not set)
    pub ema_price: Option<i64>,
    /// EMA confidence (defaults to conf if not set)
    pub ema_conf: Option<u64>,
    /// Publish timestamp (defaults to current time)
    pub publish_time: Option<i64>,
    /// Price status
    pub status: PriceStatus,
    /// Number of decimals for the asset (used by some providers)
    pub decimals: u8,
}

impl Default for PriceConf {
    fn default() -> Self {
        Self {
            price: 0,
            conf: 0,
            expo: -8,
            ema_price: None,
            ema_conf: None,
            publish_time: None,
            status: PriceStatus::Trading,
            decimals: 8,
        }
    }
}

impl PriceConf {
    /// Create a new price config with the given USD price
    ///
    /// # Example
    /// ```
    /// use shadow_oracle::PriceConf;
    ///
    /// // $100.50 with $0.05 confidence
    /// let conf = PriceConf::new_usd(100.50, 0.05);
    /// ```
    pub fn new_usd(price: f64, confidence: f64) -> Self {
        let expo = -8i32;
        let scale = 10f64.powi(expo.abs());
        Self {
            price: (price * scale) as i64,
            conf: (confidence * scale) as u64,
            expo,
            ..Default::default()
        }
    }

    /// Create a stablecoin price (pegged to $1.00)
    pub fn stablecoin() -> Self {
        Self::new_usd(1.0, 0.0001)
    }

    /// Create a price with high volatility (wide confidence interval)
    pub fn volatile(price: f64) -> Self {
        Self::new_usd(price, price * 0.02) // 2% confidence
    }

    /// Set custom decimals
    pub fn with_decimals(mut self, decimals: u8) -> Self {
        self.decimals = decimals;
        self
    }

    /// Set custom exponent
    pub fn with_expo(mut self, expo: i32) -> Self {
        self.expo = expo;
        self
    }

    /// Set status
    pub fn with_status(mut self, status: PriceStatus) -> Self {
        self.status = status;
        self
    }

    /// Get price as f64 USD value
    pub fn price_usd(&self) -> f64 {
        let scale = 10f64.powi(self.expo.abs());
        self.price as f64 / scale
    }

    /// Get confidence as f64 USD value
    pub fn conf_usd(&self) -> f64 {
        let scale = 10f64.powi(self.expo.abs());
        self.conf as f64 / scale
    }
}

/// Standard price feeds for common test scenarios
#[derive(Debug, Clone)]
pub struct StandardFeeds {
    pub sol: solana_pubkey::Pubkey,
    pub btc: solana_pubkey::Pubkey,
    pub eth: solana_pubkey::Pubkey,
    pub usdc: solana_pubkey::Pubkey,
    pub usdt: solana_pubkey::Pubkey,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_price_conf_usd() {
        let conf = PriceConf::new_usd(100.50, 0.05);
        assert_eq!(conf.price, 10050000000);
        assert_eq!(conf.conf, 5000000);
        assert_eq!(conf.expo, -8);
    }

    #[test]
    fn test_price_usd_roundtrip() {
        let conf = PriceConf::new_usd(123.456, 0.789);
        assert!((conf.price_usd() - 123.456).abs() < 0.0001);
        assert!((conf.conf_usd() - 0.789).abs() < 0.0001);
    }

    #[test]
    fn test_stablecoin() {
        let conf = PriceConf::stablecoin();
        assert!((conf.price_usd() - 1.0).abs() < 0.0001);
    }
}
