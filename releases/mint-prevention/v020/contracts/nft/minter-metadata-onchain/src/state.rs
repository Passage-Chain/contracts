
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use cosmwasm_std::{Addr, Coin, Timestamp};
use cw_storage_plus::{Item, Map, Index, IndexList, IndexedMap, MultiIndex};
use pg721_metadata_onchain::msg::Metadata;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub admin: Addr,
    pub max_num_tokens: u32,
    pub cw721_code_id: u64,
    pub unit_price: Coin,
    pub whitelist: Option<Addr>,
    pub start_time: Timestamp,
    pub per_address_limit: u32,
}

pub const CONFIG: Item<Config> = Item::new("config");
pub const CW721_ADDRESS: Item<Addr> = Item::new("cw721_address");
pub const MINTER_ADDRS: Map<Addr, u32> = Map::new("minter_address");
pub const MINTABLE_TOKEN_IDS: Item<Vec<u32>> = Item::new("mintable_token_ids");

pub type TokenId = u32;

/// Represents the state of a mintable NFT
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct TokenMint {
    pub token_id: TokenId,
    pub is_minted: bool,
    pub metadata: Metadata,
}

/// Defines indices for accessing TokenMint
pub struct TokenMintIndices<'a> {
    pub is_minted: MultiIndex<'a, u8, TokenMint, TokenId>,
}

impl<'a> IndexList<TokenMint> for TokenMintIndices<'a> {
    fn get_indexes(&'_ self) -> Box<dyn Iterator<Item = &'_ dyn Index<TokenMint>> + '_> {
        let v: Vec<&dyn Index<TokenMint>> = vec![&self.is_minted];
        Box::new(v.into_iter())
    }
}

pub fn token_mints<'a>() -> IndexedMap<'a, TokenId, TokenMint, TokenMintIndices<'a>> {
    let indexes = TokenMintIndices {
        is_minted: MultiIndex::new(|_, d: &TokenMint|  d.is_minted as u8, "token_mint", "token_mint__is_minted"),
    };
    IndexedMap::new("token_mint", indexes)
}