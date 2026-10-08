#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{to_binary, Binary, Deps, DepsMut, Empty, Env, MessageInfo, StdResult, Response, Event};
use cw2::{set_contract_version, get_contract_version};

use crate::ContractError;
use cw721::ContractInfoResponse;
use cw721_base::{ContractError as BaseError, MintMsg};
use url::Url;

use crate::msg::{
    CollectionInfoResponse, InstantiateMsg, QueryMsg, RoyaltyInfoResponse,
    Extension, ExecuteMsg, MigrateMsg
};
use crate::state::{CollectionInfo, RoyaltyInfo, COLLECTION_INFO,MIGRATION_STATUS,MigrationStatus,migration_status,migration_done};

pub type Pg721MetadataContract<'a> = cw721_base::Cw721Contract<'a, Extension, Empty>;

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:pg-721-metadata-onchain";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

const MAX_DESCRIPTION_LENGTH: u32 = 512;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    // cw721 instantiation
    let info = ContractInfoResponse {
        name: msg.name,
        symbol: msg.symbol,
    };
    Pg721MetadataContract::default()
        .contract_info
        .save(deps.storage, &info)?;

    let minter = deps.api.addr_validate(&msg.minter)?;
    Pg721MetadataContract::default()
        .minter
        .save(deps.storage, &minter)?;

    // pg721 instantiation
    if msg.collection_info.description.len() > MAX_DESCRIPTION_LENGTH as usize {
        return Err(ContractError::DescriptionTooLong {});
    }

    let image = Url::parse(&msg.collection_info.image)?;

    if let Some(ref external_link) = msg.collection_info.external_link {
        Url::parse(external_link)?;
    }

    let royalty_info: Option<RoyaltyInfo> = match msg.collection_info.royalty_info {
        Some(royalty_info) => Some(RoyaltyInfo {
            payment_address: deps.api.addr_validate(&royalty_info.payment_address)?,
            share: royalty_info.share_validate()?,
        }),
        None => None,
    };

    deps.api.addr_validate(&msg.collection_info.creator)?;

    let collection_info = CollectionInfo {
        creator: msg.collection_info.creator,
        description: msg.collection_info.description,
        image: msg.collection_info.image,
        external_link: msg.collection_info.external_link,
        royalty_info,
    };

    COLLECTION_INFO.save(deps.storage, &collection_info)?;

    let migration_status=MigrationStatus{
        done: false,
    };

    MIGRATION_STATUS.save(deps.storage, &migration_status)?;
    
    Ok(Response::default()
        .add_attribute("action", "instantiate")
        .add_attribute("contract_name", CONTRACT_NAME)
        .add_attribute("contract_version", CONTRACT_VERSION)
        .add_attribute("image", image.to_string()))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, BaseError> {
    match msg{
        ExecuteMsg::Migrate{migrations}=>migrate_data(deps,env,info,migrations),
        ExecuteMsg::MigrationDone{}=>migrated(deps, info),
        _ => if migration_status(deps.as_ref()){
            Pg721MetadataContract::default().execute(deps, env, info, msg.into())
        } else {
            return Err(ContractError::MigrationInProgress {  }.into());
        },
    }
}

fn migrate_data(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    migrations: Vec<MintMsg<Extension>>,
)-> Result<Response, BaseError>{

    // check migration status
    if migration_status(deps.as_ref()){
        return Err(ContractError::MigrationDone {  }.into());
    }

    //  verify sender == minter 
    let minter = Pg721MetadataContract::default().minter.load(deps.storage)?;
    if info.sender != minter {
        return Err(BaseError::Unauthorized {});
    }

     // migrate tokens
     for migration in migrations.into_iter(){
        let new_deps = DepsMut { storage: deps.storage, api: deps.api, querier: deps.querier };
        let exce_msg=ExecuteMsg::Mint(migration.clone());
        let res = Pg721MetadataContract::default().execute(new_deps, env.clone(), info.clone(), exce_msg.into());

        match res {
            Ok(response) => {
                for attribute in response.attributes.iter() {
                    if attribute.key == "token_id" {
                        if attribute.value != migration.token_id {
                            return Err(ContractError::MintFalied(migration.token_id).into());
                        }
                    }
                }
            },
            Err(error) => {
                // Handle the error case
                return Err(ContractError::MigrationFailed(error).into());
            }
        }
    }

    Ok(Response::new()
    .add_attribute("action", "migrated")
    .add_attribute("minter", info.sender))
}

fn migrated(
    deps: DepsMut,
    info: MessageInfo,
)->Result<Response,BaseError>{
    //  verify sender == minter 
    let minter = Pg721MetadataContract::default().minter.load(deps.storage)?;
    if info.sender != minter {
        return Err(BaseError::Unauthorized {});
    }

    migration_done(deps)?;
    
    Ok(Response::new()
        .add_attribute("action", "migration_done")
        .add_attribute("marked done as", true.to_string())
    )
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::CollectionInfo {} => to_binary(&query_config(deps)?),
        _ => Pg721MetadataContract::default().query(deps, env, msg.into()),
    }
}

fn query_config(deps: Deps) -> StdResult<CollectionInfoResponse> {
    let info = COLLECTION_INFO.load(deps.storage)?;

    let royalty_info_res: Option<RoyaltyInfoResponse> = match info.royalty_info {
        Some(royalty_info) => Some(RoyaltyInfoResponse {
            payment_address: royalty_info.payment_address.to_string(),
            share: royalty_info.share,
        }),
        None => None,
    };

    Ok(CollectionInfoResponse {
        creator: info.creator,
        description: info.description,
        image: info.image,
        external_link: info.external_link,
        royalty_info: royalty_info_res,
    })
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, msg: MigrateMsg) -> Result<Response, ContractError> {
    let storage_version: &str = &get_contract_version(deps.storage)?.version.to_string();

    let mut response = Response::new();
    if storage_version < CONTRACT_VERSION {
        set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
        let minter = deps.api.addr_validate(&msg.minter)?;
        Pg721MetadataContract::default()
            .minter
            .save(deps.storage, &minter)?;

        let event = Event::new("migrate-storage")
            .add_attribute("new-minter", minter.to_string());
        response.events.push(event);
    }

    let event = Event::new("contract-migrated")
        .add_attribute("prev-version", storage_version)
        .add_attribute("next-version", CONTRACT_VERSION);
    response.events.push(event);
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::msg::Metadata;
    use crate::state::CollectionInfo;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{coins, from_binary, Decimal, Attribute};
    use cw721::NftInfoResponse;

    const NATIVE_DENOM: &str = "ujunox";

    fn setup_contract(deps: DepsMut, royalty_info: Option<RoyaltyInfoResponse>) {
        let collection = String::from("collection0");
        let image: String = "https://example.com/image.png".to_string();
        let msg = InstantiateMsg {
            name: collection,
            symbol: String::from("BOBO"),
            minter: String::from("minter"),
            collection_info: CollectionInfo {
                creator: String::from("creator"),
                description: String::from("Passage Monkeys"),
                image: image.clone(),
                external_link: Some("https://example.com/external.html".to_string()),
                royalty_info: royalty_info,
            },
        };
        let info = mock_info("minter", &coins(0, NATIVE_DENOM));
        let res = instantiate(deps, mock_env(), info.clone(), msg).unwrap();
        assert!(res.attributes[0].eq(&Attribute::new("action", "instantiate")));
        assert!(res.attributes[1].eq(&Attribute::new("contract_name", CONTRACT_NAME)));
        assert!(res.attributes[2].eq(&Attribute::new("contract_version", CONTRACT_VERSION)));
        assert!(res.attributes[3].eq(&Attribute::new("image", image)));
    }

    #[test]
    fn proper_initialization_no_royalties() {
        let mut deps = mock_dependencies();
        setup_contract(deps.as_mut(), None);

        // let's query the collection info
        let res = query(deps.as_ref(), mock_env(), QueryMsg::CollectionInfo {}).unwrap();
        let value: CollectionInfoResponse = from_binary(&res).unwrap();
        assert_eq!("https://example.com/image.png", value.image);
        assert_eq!("Passage Monkeys", value.description);
        assert_eq!(
            "https://example.com/external.html",
            value.external_link.unwrap()
        );
        assert_eq!(None, value.royalty_info);

        let nft_res = query(deps.as_ref(), mock_env(), QueryMsg::NftInfo { token_id: "0001".to_string() }).unwrap();
        let nft_info: NftInfoResponse<Extension> = from_binary(&nft_res).unwrap();

        let metadata = Metadata{
            name: Some("test nft".to_string()),
            ..Default::default()
        };

        let nft = NftInfoResponse{
            token_uri: None,
            extension: Some(metadata)
        };
        assert_eq!(nft,nft_info)
    }

    #[test]
    fn proper_initialization_with_royalties() {
        let mut deps = mock_dependencies();
        let creator: String = String::from("creator");
        setup_contract(deps.as_mut(), Some(RoyaltyInfoResponse {
            payment_address: creator.clone(),
            share: Decimal::percent(10)
        }));

        // let's query the collection info
        let res = query(deps.as_ref(), mock_env(), QueryMsg::CollectionInfo {}).unwrap();
        let value: CollectionInfoResponse = from_binary(&res).unwrap();
        assert_eq!(
            Some(RoyaltyInfoResponse {
                payment_address: creator,
                share: Decimal::percent(10),
            }),
            value.royalty_info
        );
    }
}
