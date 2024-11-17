use cosmwasm_std::{Coin, Timestamp};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use pg721_metadata_onchain::msg::{InstantiateMsg as Pg721InstantiateMsg, Metadata};
use crate::state::{TokenMint};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub max_num_tokens: u32,
    pub cw721_code_id: u64,
    pub start_time: Timestamp,
    pub per_address_limit: u32,
    pub unit_price: Coin,
    pub whitelist: Option<String>,
    pub cw721_address: Option<String>,
    pub cw721_instantiate_msg: Option<Pg721InstantiateMsg>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    UpsertTokenMetadatas { token_metadatas: Vec<TokenMetadata> },
    Mint {},
    SetAdmin { admin: String },
    SetWhitelist { whitelist: String },
    UpdateStartTime(Timestamp),
    UpdatePerAddressLimit { per_address_limit: u32 },
    UpdateUnitPrice { unit_price: Coin },
    MintTo { recipient: String },
    MintFor { token_id: u32, recipient: String },
    Withdraw { recipient: String },
    MigrateData {migrations: Migration},
    MigrationDone{},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    NumMinted {},
    NumRemaining {},
    StartTime {},
    MintPrice {},
    MintCount { address: String },
    TokenMint { token_id: u32 },
    TokenMints {
        descending: Option<bool>,
        filter_minted: Option<bool>,
        start_after: Option<u32>,
        limit: Option<u32>
     },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MigrateMsg {
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    pub admin: String,
    pub max_num_tokens: u32,
    pub per_address_limit: u32,
    pub cw721_address: String,
    pub cw721_code_id: u64,
    pub start_time: Timestamp,
    pub unit_price: Coin,
    pub whitelist: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct NumMintedResponse {
    pub num_minted: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct NumRemainingResponse {
    pub num_remaining: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct StartTimeResponse {
    pub start_time: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MintPriceResponse {
    pub public_price: Coin,
    pub whitelist_price: Option<Coin>,
    pub current_price: Coin,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MintCountResponse {
    pub address: String,
    pub count: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct TokenMintResponse {
    pub token_mint: Option<TokenMint>
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct TokenMintsResponse {
    pub token_mints: Vec<TokenMint>
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct TokenMetadata {
    pub token_id: u32,
    pub metadata: Metadata
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Migration{
    pub tokens: Option<Vec<TokenMint>>,
    pub mintable_tokens: Option<Vec<u32>>,
    pub minters: Option<Vec<Minter>>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Minter{
    pub address: String,
    pub mints: u32
}