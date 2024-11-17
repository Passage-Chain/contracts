use cosmwasm_std::{Addr, Decimal, DepsMut, StdResult, Deps};
use cw_storage_plus::Item;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct CollectionInfo<T> {
    pub creator: String,
    pub description: String,
    pub image: String,
    pub external_link: Option<String>,
    pub royalty_info: Option<T>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct RoyaltyInfo {
    pub payment_address: Addr,
    pub share: Decimal,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MigrationStatus {
    pub done: bool,
}

// marks migrations as done
pub fn migration_done(deps: DepsMut) -> StdResult<()>{
    let mut status=MIGRATION_STATUS.load(deps.storage)?;
    status.done=true;
    MIGRATION_STATUS.save(deps.storage, &status)
}

// returns true if migration is done
pub fn migration_status(deps: Deps) -> bool {
    let status = MIGRATION_STATUS.load(deps.storage).unwrap();
    return status.done;
}

pub const COLLECTION_INFO: Item<CollectionInfo<RoyaltyInfo>> = Item::new("collection_info");
pub const MIGRATION_STATUS: Item<MigrationStatus> = Item::new("migration_status");