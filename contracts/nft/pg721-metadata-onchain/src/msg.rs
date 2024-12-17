use serde::{Deserialize, Serialize};
use schemars::JsonSchema;
use cw721_base::{ExecuteMsg as Cw721ExecuteMsg,MintMsg};
use cw721::Expiration;
use cosmwasm_std::Binary;
pub use pg721::msg::*;

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug, Default)]
pub struct Trait {
    pub display_type: Option<String>,
    pub trait_type: String,
    pub value: String,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug, Default)]
pub struct Metadata {
    pub image: Option<String>,
    pub image_data: Option<String>,
    pub external_url: Option<String>,
    pub description: Option<String>,
    pub name: Option<String>,
    pub attributes: Option<Vec<Trait>>,
    pub background_color: Option<String>,
    pub animation_url: Option<String>,
    pub youtube_url: Option<String>,
}

pub type Extension = Option<Metadata>;

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    /// Transfer is a base message to move a token to another account without triggering actions
    TransferNft { recipient: String, token_id: String },
    /// Send is a base message to transfer a token to a contract and trigger an action
    /// on the receiving contract.
    SendNft {
        contract: String,
        token_id: String,
        msg: Binary,
    },
    /// Allows operator to transfer / send the token from the owner's account.
    /// If expiration is set, then this allowance has a time/height limit
    Approve {
        spender: String,
        token_id: String,
        expires: Option<Expiration>,
    },
    /// Remove previously granted Approval
    Revoke { spender: String, token_id: String },
    /// Allows operator to transfer / send any token from the owner's account.
    /// If expiration is set, then this allowance has a time/height limit
    ApproveAll {
        operator: String,
        expires: Option<Expiration>,
    },
    /// Remove previously granted ApproveAll permission
    RevokeAll { operator: String },

    /// Mint a new NFT, can only be called by the contract minter
    Mint(MintMsg<Extension>),

    /// Burn an NFT the sender has access to
    Burn { token_id: String },

    // Migrate data
    Migrate { migrations: Vec<MintMsg<Extension>> },

    // Migration done
    MigrationDone{},
}

impl From<ExecuteMsg> for Cw721ExecuteMsg<Extension>{
    fn from(msg: ExecuteMsg) -> Cw721ExecuteMsg<Extension>{
        match msg {
            ExecuteMsg::TransferNft { recipient, token_id } => Cw721ExecuteMsg::TransferNft { recipient, token_id },
            ExecuteMsg::SendNft { contract, token_id, msg } =>Cw721ExecuteMsg::SendNft { contract, token_id, msg },
            ExecuteMsg::Approve { spender, token_id, expires }=>Cw721ExecuteMsg::Approve { spender, token_id, expires },
            ExecuteMsg::ApproveAll { operator, expires } => Cw721ExecuteMsg::ApproveAll { operator, expires },
            ExecuteMsg::Revoke { spender, token_id }=>Cw721ExecuteMsg::Revoke { spender, token_id },
            ExecuteMsg::RevokeAll { operator }=>Cw721ExecuteMsg::RevokeAll { operator },
            ExecuteMsg::Mint(MintMsg { token_id, owner, token_uri, extension })=>Cw721ExecuteMsg::Mint(MintMsg { token_id, owner, token_uri, extension }),
            ExecuteMsg::Burn { token_id }=>Cw721ExecuteMsg::Burn { token_id },
            _ => unreachable!("cannot convert {:?} to Cw721QueryMsg", msg),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug, Default)]
pub struct MigrateMsg {
    pub minter: String,
}