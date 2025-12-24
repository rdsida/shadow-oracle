# shadow-oracle

LiteSVM Oracle mocks for Solana testing. Provides mock implementations of Pyth, Switchboard, and Chainlink oracles for use with LiteSVM.

## Installation

Add to your `Cargo.toml`:

```toml
[dev-dependencies]
shadow-oracle = "0.1"
```

### Feature Flags

All oracle providers are enabled by default. To use only specific providers:

```toml
[dev-dependencies]
shadow-oracle = { version = "0.1", default-features = false, features = ["pyth"] }
```

Available features:

- `pyth` - Pyth Network oracle
- `switchboard` - Switchboard V2 oracle
- `chainlink` - Chainlink oracle

## Quick Start

```rust
use litesvm::LiteSVM;
use shadow_oracle::{ShadowOracle, PriceConf};

// Use with_sysvars() to initialize Clock sysvar (required for oracle timestamps)
let mut svm = LiteSVM::new().with_sysvars();
let mut oracle = ShadowOracle::new(&mut svm);

// Create a price feed
let sol_feed = oracle.pyth().create_price_feed(PriceConf::new_usd(100.0, 0.1));

// Update the price
oracle.pyth().set_price_usd(&sol_feed, 150.0, 0.2).unwrap();

// Read the price
let (price, confidence) = oracle.pyth().get_price_usd(&sol_feed).unwrap();
```

## PriceConf

`PriceConf` is a provider-agnostic price configuration struct used across all oracles.

### Creating Prices

```rust
use shadow_oracle::{PriceConf, PriceStatus};

// From USD values (price, confidence)
let price = PriceConf::new_usd(100.0, 0.1);

// Stablecoin ($1.00 with tight confidence)
let usdc = PriceConf::stablecoin();

// Volatile asset (2% confidence interval)
let sol = PriceConf::volatile(100.0);

// With custom settings
let custom = PriceConf::new_usd(100.0, 0.1)
    .with_decimals(9)
    .with_expo(-8)
    .with_status(PriceStatus::Trading);
```

### Price Status

```rust
pub enum PriceStatus {
    Trading,  // Normal trading
    Halted,   // Trading halted
    Unknown,  // Status unknown
    Auction,  // In auction
}
```

## Pyth Oracle

Pyth provides high-fidelity price feeds with confidence intervals, EMA prices, and detailed status tracking.

### Setup

```rust
use litesvm::LiteSVM;
use shadow_oracle::{ShadowOracle, PriceConf};

let mut svm = LiteSVM::new().with_sysvars();
let mut oracle = ShadowOracle::new(&mut svm);
let pyth = oracle.pyth();
```

### Creating Price Feeds

```rust
// Create with random address
let feed = pyth.create_price_feed(PriceConf::new_usd(100.0, 0.1));

// Create at specific address
use solana_pubkey::Pubkey;
let address = Pubkey::new_unique();
pyth.create_price_feed_at(address, PriceConf::new_usd(100.0, 0.1));

// Create standard feeds (SOL, BTC, ETH, USDC, USDT)
let feeds = pyth.create_standard_feeds();
// feeds.sol, feeds.btc, feeds.eth, feeds.usdc, feeds.usdt
```

### Reading and Writing Prices

```rust
// Set price in USD
pyth.set_price_usd(&feed, 150.0, 0.2).unwrap();

// Set raw price values (i64 price, u64 confidence)
pyth.set_price(&feed, 15000000000, 20000000).unwrap();

// Get price in USD
let (price, confidence) = pyth.get_price_usd(&feed).unwrap();

// Get raw price values
let (price, conf) = pyth.get_price(&feed).unwrap();

// Set price status
pyth.set_status(&feed, PriceStatus::Halted).unwrap();
```

### Simulating Market Events

```rust
// Simulate a 50% crash
pyth.simulate_crash(&feed, 50.0).unwrap();

// Simulate stablecoin depeg (sets price to given value)
pyth.simulate_depeg(&feed, 0.85).unwrap();
```

### Known Mainnet Addresses

```rust
use shadow_oracle::feeds::pyth;

// Use mainnet feed addresses for realistic testing
pyth.create_price_feed_at(pyth::SOL_USD, PriceConf::new_usd(100.0, 0.1));
pyth.create_price_feed_at(pyth::BTC_USD, PriceConf::new_usd(43000.0, 10.0));
pyth.create_price_feed_at(pyth::ETH_USD, PriceConf::new_usd(2200.0, 1.0));
pyth.create_price_feed_at(pyth::USDC_USD, PriceConf::stablecoin());
pyth.create_price_feed_at(pyth::USDT_USD, PriceConf::stablecoin());
```

## Switchboard Oracle

Switchboard V2 provides price feeds with standard deviation values.

### Setup

```rust
use litesvm::LiteSVM;
use shadow_oracle::{ShadowOracle, PriceConf};

let mut svm = LiteSVM::new().with_sysvars();
let mut oracle = ShadowOracle::new(&mut svm);
let switchboard = oracle.switchboard();
```

### Creating Price Feeds

```rust
// Create with random address
let feed = switchboard.create_price_feed(PriceConf::new_usd(100.0, 0.1));

// Create at specific address
use solana_pubkey::Pubkey;
let address = Pubkey::new_unique();
switchboard.create_price_feed_at(address, PriceConf::new_usd(100.0, 0.1));

// Create standard feeds
let feeds = switchboard.create_standard_feeds();
```

### Reading and Writing Prices

```rust
// Set price (price, std_deviation)
switchboard.set_price(&feed, 150.0, 0.5).unwrap();

// set_price_usd is an alias to set_price for Switchboard
switchboard.set_price_usd(&feed, 150.0, 0.5).unwrap();

// Get price returns (price, std_deviation)
let (price, std_dev) = switchboard.get_price(&feed).unwrap();
let (price, std_dev) = switchboard.get_price_usd(&feed).unwrap();
```

### Simulating Market Events

```rust
// Simulate a 50% crash
switchboard.simulate_crash(&feed, 50.0).unwrap();

// Simulate stablecoin depeg
switchboard.simulate_depeg(&feed, 0.85).unwrap();
```

### Known Mainnet Addresses

```rust
use shadow_oracle::feeds::switchboard;

switchboard.create_price_feed_at(switchboard::SOL_USD, PriceConf::new_usd(100.0, 0.1));
switchboard.create_price_feed_at(switchboard::BTC_USD, PriceConf::new_usd(43000.0, 10.0));
switchboard.create_price_feed_at(switchboard::ETH_USD, PriceConf::new_usd(2200.0, 1.0));
```

## Chainlink Oracle

Chainlink provides simple price feeds with round tracking.

### Setup

```rust
use litesvm::LiteSVM;
use shadow_oracle::{ShadowOracle, PriceConf};

let mut svm = LiteSVM::new().with_sysvars();
let mut oracle = ShadowOracle::new(&mut svm);
let chainlink = oracle.chainlink();
```

### Creating Price Feeds

```rust
// Create with random address
let feed = chainlink.create_price_feed(PriceConf::new_usd(100.0, 0.1));

// Create at specific address
use solana_pubkey::Pubkey;
let address = Pubkey::new_unique();
chainlink.create_price_feed_at(address, PriceConf::new_usd(100.0, 0.1));

// Create standard feeds
let feeds = chainlink.create_standard_feeds();
```

### Reading and Writing Prices

```rust
// Set price (confidence parameter is ignored for Chainlink)
chainlink.set_price(&feed, 150.0).unwrap();
chainlink.set_price_usd(&feed, 150.0, 0.0).unwrap();

// Get price
let price = chainlink.get_price(&feed).unwrap();

// get_price_usd returns (price, 0.0) for API compatibility
let (price, _) = chainlink.get_price_usd(&feed).unwrap();

// Chainlink-specific methods
let answer = chainlink.get_latest_answer(&feed).unwrap();  // i128 scaled value
let decimals = chainlink.get_decimals(&feed).unwrap();      // u8
let round = chainlink.get_latest_round(&feed).unwrap();     // u32
```

### Simulating Market Events

```rust
// Simulate a 50% crash
chainlink.simulate_crash(&feed, 50.0).unwrap();

// Simulate stablecoin depeg
chainlink.simulate_depeg(&feed, 0.85).unwrap();
```

### Known Mainnet Addresses

```rust
use shadow_oracle::feeds::chainlink;

chainlink.create_price_feed_at(chainlink::SOL_USD, PriceConf::new_usd(100.0, 0.1));
chainlink.create_price_feed_at(chainlink::BTC_USD, PriceConf::new_usd(43000.0, 10.0));
chainlink.create_price_feed_at(chainlink::ETH_USD, PriceConf::new_usd(2200.0, 1.0));
```

## Testing Patterns

### Basic Price Feed Test

```rust
#[test]
fn test_price_feed() {
    let mut svm = LiteSVM::new().with_sysvars();
    let mut oracle = ShadowOracle::new(&mut svm);

    let feed = oracle.pyth().create_price_feed(PriceConf::new_usd(100.0, 0.1));

    // Your program uses the feed...

    // Update price and test again
    oracle.pyth().set_price_usd(&feed, 150.0, 0.2).unwrap();
}
```

### Testing Multiple Oracles

```rust
#[test]
fn test_multi_oracle() {
    let mut svm = LiteSVM::new().with_sysvars();
    let mut oracle = ShadowOracle::new(&mut svm);

    let pyth_feed = oracle.pyth().create_price_feed(PriceConf::new_usd(100.0, 0.1));
    let sb_feed = oracle.switchboard().create_price_feed(PriceConf::new_usd(100.0, 0.1));
    let cl_feed = oracle.chainlink().create_price_feed(PriceConf::new_usd(100.0, 0.1));

    // Test your program with different oracle sources
}
```

### Testing Crash Scenarios

```rust
#[test]
fn test_liquidation_on_crash() {
    let mut svm = LiteSVM::new().with_sysvars();
    let mut oracle = ShadowOracle::new(&mut svm);

    let feed = oracle.pyth().create_price_feed(PriceConf::new_usd(100.0, 0.1));

    // Setup position at $100

    // Simulate 50% crash
    oracle.pyth().simulate_crash(&feed, 50.0).unwrap();

    // Verify liquidation triggered
}
```

### Testing Stablecoin Depeg

```rust
#[test]
fn test_depeg_handling() {
    let mut svm = LiteSVM::new().with_sysvars();
    let mut oracle = ShadowOracle::new(&mut svm);

    let usdc_feed = oracle.pyth().create_price_feed(PriceConf::stablecoin());

    // Setup with USDC at $1.00

    // Simulate depeg to $0.85
    oracle.pyth().simulate_depeg(&usdc_feed, 0.85).unwrap();

    // Verify protocol handles depeg correctly
}
```

### Testing with Mainnet Addresses

```rust
use shadow_oracle::feeds;

#[test]
fn test_with_mainnet_addresses() {
    let mut svm = LiteSVM::new().with_sysvars();
    let mut oracle = ShadowOracle::new(&mut svm);

    // Use actual mainnet addresses your program expects
    oracle.pyth().create_price_feed_at(feeds::pyth::SOL_USD, PriceConf::new_usd(100.0, 0.1));

    // Your program will work with the same addresses it uses on mainnet
}
```

### Testing Price Staleness

Oracle timestamps are tied to LiteSVM's Clock sysvar. When you warp time forward, oracle prices become stale without needing to update them. This allows testing staleness checks in your program.

```rust
use solana_clock::Clock;

#[test]
fn test_stale_price_rejected() {
    let mut svm = LiteSVM::new().with_sysvars();
    let mut oracle = ShadowOracle::new(&mut svm);

    // Create a fresh price feed (timestamp = current clock time)
    let feed = oracle.pyth().create_price_feed(PriceConf::new_usd(100.0, 0.1));

    // Price is fresh, transaction succeeds
    // ... execute your program ...

    // Warp time forward by 1 hour (3600 seconds)
    let mut clock = svm.get_sysvar::<Clock>();
    clock.unix_timestamp += 3600;
    svm.set_sysvar(&clock);

    // Now the oracle price is 1 hour old
    // Your program's staleness check should reject this
    // ... execute your program, expect failure ...
}

#[test]
fn test_refresh_stale_price() {
    let mut svm = LiteSVM::new().with_sysvars();
    let mut oracle = ShadowOracle::new(&mut svm);

    let feed = oracle.pyth().create_price_feed(PriceConf::new_usd(100.0, 0.1));

    // Warp time forward
    let mut clock = svm.get_sysvar::<Clock>();
    clock.unix_timestamp += 3600;
    svm.set_sysvar(&clock);

    // Price is now stale...

    // Update the price - this refreshes the timestamp to current clock time
    oracle.pyth().set_price_usd(&feed, 100.0, 0.1).unwrap();

    // Price is fresh again, transaction succeeds
    // ... execute your program ...
}

#[test]
fn test_slot_based_staleness() {
    let mut svm = LiteSVM::new().with_sysvars();
    let mut oracle = ShadowOracle::new(&mut svm);

    let feed = oracle.pyth().create_price_feed(PriceConf::new_usd(100.0, 0.1));

    // Warp to a much later slot
    svm.warp_to_slot(1_000_000);

    // The oracle's slot is now far behind the current slot
    // Programs checking slot-based staleness will reject this price

    // Update price to refresh both timestamp and slot
    oracle.pyth().set_price_usd(&feed, 100.0, 0.1).unwrap();
}
```

## Error Handling

```rust
use shadow_oracle::ShadowOracleError;

match oracle.pyth().get_price(&unknown_feed) {
    Ok((price, conf)) => { /* use price */ },
    Err(ShadowOracleError::PriceFeedNotFound(addr)) => {
        // Feed not registered
    },
    Err(ShadowOracleError::InvalidPriceData(msg)) => {
        // Invalid price data
    },
    Err(e) => { /* other errors */ }
}
```

## Program IDs

| Oracle                | Program ID                                     |
| --------------------- | ---------------------------------------------- |
| Pyth                  | `FsJ3A3u2vn5cTVofAjvy6y5kwABJAqYWpe4975bi2epH` |
| Switchboard           | `SW1TCH7qEPTdLsDHRgPuMQjbQxKdH2aBStViMFnt64f`  |
| Switchboard On-Demand | `SBondMDrcV3K4kxZR1HNVT7osZxAHVHgYXL5Ze1oMUv`  |
| Chainlink             | `HEvSKofvBgfaexv23kMabbYqxasxU3mQ4ibBMEmJWHny` |
| Chainlink Store       | `CaH12fwNTKJAG8PxEvo9R96Zc2j8Jq3Q5K9B7tTFQ2by` |

## License

MIT
