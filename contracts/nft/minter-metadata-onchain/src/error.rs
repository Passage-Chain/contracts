use cosmwasm_std::{Coin, StdError, Timestamp};
use cw_utils::PaymentError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Invalid reply ID")]
    InvalidReplyID {},

    #[error("Not enough funds sent")]
    NotEnoughFunds {},

    #[error("TooManyCoins")]
    TooManyCoins {},

    #[error("IncorrectPaymentAmount {0} != {1}")]
    IncorrectPaymentAmount(Coin, Coin),

    #[error("InvalidNumTokens, min: 1")]
    InvalidNumTokens { min: u32 },

    #[error("Sold out")]
    SoldOut {},

    #[error("Minimum network mint price {expected} got {got}")]
    InsufficientMintPrice { expected: u128, got: u128 },

    #[error("Invalid address {addr}")]
    InvalidAddress { addr: String },

    #[error("Invalid token id")]
    InvalidTokenId {},
    
    #[error("Token has already been minted {token_id}")]
    TokenAlreadyMinted { token_id: u32 },

    #[error("AlreadyStarted")]
    AlreadyStarted {},

    #[error("WhitelistAlreadyStarted")]
    WhitelistAlreadyStarted {},

    #[error("InvalidStartTime {0} < {1}")]
    InvalidStartTime(Timestamp, Timestamp),

    #[error("Must set either cw721_address or cw721_instantiate_msg, but not both")]
    InvalidInstantiateMsg {},

    #[error("Instantiate cw721 error")]
    InstantiatePg721Error {},

    #[error("Invalid base token URI (must be an IPFS URI)")]
    InvalidBaseTokenURI {},

    #[error("address not on whitelist: {addr}")]
    NotWhitelisted { addr: String },

    #[error("Minting has not started yet")]
    BeforeMintStartTime {},

    #[error("Invalid minting limit per address. min: 1, got: {got}")]
    InvalidPerAddressLimit { min: u32, got: u32 },

    #[error("Max minting limit per address exceeded")]
    MaxPerAddressLimitExceeded {},

    #[error("Token id: {token_id} did not have matching metadata")]
    MetadataNotFound { token_id: u32 },

    #[error("Full set of metadata not found on the contract. expected: {expected}, actual: {actual}")]
    MissingMetadata { expected: u32, actual: u32 },

    #[error("ZeroBalance")]
    ZeroBalance {},

    #[error("{0}")]
    Payment(#[from] PaymentError),

    #[error("Migration in progress")]
    MigrationInProgress{},

    #[error("Migration completed")]
    MigrationDone{},
}
