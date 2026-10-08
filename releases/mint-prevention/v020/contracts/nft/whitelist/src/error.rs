use cosmwasm_std::{StdError, Timestamp};
use cw_utils::PaymentError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("AlreadyStarted")]
    AlreadyStarted {},

    #[error("DuplicateMember: {0}")]
    DuplicateMember(String),

    #[error("NoMemberFound: {0}")]
    NoMemberFound(String),

    #[error("InvalidStartTime {0} > {1}")]
    InvalidStartTime(Timestamp, Timestamp),

    #[error("InvalidEndTime {0} > {1}")]
    InvalidEndTime(Timestamp, Timestamp),

    #[error("MembersExceeded: {expected} got {actual}")]
    MembersExceeded { expected: u32, actual: u32 },

    #[error("Invalid minting limit per address. max: {max}, got: {got}")]
    InvalidPerAddressLimit { max: String, got: String },

    #[error("Invalid member limit. min: {min}, got: {got}")]
    InvalidMemberLimit { min: u32, got: u32 },

    #[error("Max minting limit per address exceeded")]
    MaxPerAddressLimitExceeded {},

    #[error("InvalidUnitPrice {0}")]
    InvalidUnitPrice(u128),

    #[error("{0}")]
    PaymentError(#[from] PaymentError),
}
