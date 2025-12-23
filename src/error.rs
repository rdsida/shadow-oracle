use thiserror::Error;

#[derive(Error, Debug)]
pub enum ShadowOracleError {
    #[error("Price feed not found: {0}")]
    PriceFeedNotFound(String),

    #[error("Invalid price data: {0}")]
    InvalidPriceData(String),

    #[error("Failed to serialize account: {0}")]
    SerializationError(String),

    #[error("Provider not available: {0}")]
    ProviderNotAvailable(String),
}
