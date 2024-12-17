#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    coin, to_binary, Addr, BankMsg, Binary, Coin, CosmosMsg, Deps, DepsMut, Env,
    MessageInfo, Order, Reply, ReplyOn, StdError, StdResult, Timestamp, WasmMsg,
    Response, SubMsg, Event
};
use cw_storage_plus::{Bound};
use cw2::{set_contract_version, get_contract_version};
use cw721_base::MintMsg;
use cw_utils::{may_pay, parse_reply_instantiate_data};
use pg721_metadata_onchain::msg::{
    InstantiateMsg as Pg721InstantiateMsg, ExecuteMsg as Pg721ExecuteMsg, Metadata
};
use pg721::state::{MIGRATION_STATUS,MigrationStatus,migration_done,migration_status};

use crate::error::ContractError;
use crate::msg::{
    ConfigResponse, ExecuteMsg, InstantiateMsg, MintCountResponse, MintPriceResponse,
    QueryMsg, StartTimeResponse, TokenMetadata, TokenMintResponse, TokenMintsResponse,
    NumMintedResponse, NumRemainingResponse, MigrateMsg, Minter
};
use crate::state::{
    CONFIG, MINTER_ADDRS, CW721_ADDRESS, MINTABLE_TOKEN_IDS,
    Config, TokenMint, token_mints, 
};
use whitelist::msg::{
    ConfigResponse as WhitelistConfigResponse, HasMemberResponse, QueryMsg as WhitelistQueryMsg,
};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:passage-minter-metadata-onchain";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

const INSTANTIATE_CW721_REPLY_ID: u64 = 1;

const DEFAULT_QUERY_LIMIT: u32 = 10;
const MAX_QUERY_LIMIT: u32 = 30;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    if msg.cw721_instantiate_msg.is_none() && msg.cw721_address.is_none() ||
       msg.cw721_instantiate_msg.is_some() && msg.cw721_address.is_some() {
        return Err(ContractError::InvalidInstantiateMsg {});
    }

    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    // Check the number of tokens is more than zero
    if msg.max_num_tokens == 0 {
        return Err(ContractError::InvalidNumTokens {
            min: 1,
        });
    }

    // Check per address limit is valid
    if msg.per_address_limit == 0 {
        return Err(ContractError::InvalidPerAddressLimit {
            min: 1,
            got: msg.per_address_limit,
        });
    }

    // If current time is beyond the provided start time return error
    if env.block.time > msg.start_time {
        return Err(ContractError::InvalidStartTime(
            msg.start_time,
            env.block.time,
        ));
    }

    // Validate address for the optional whitelist contract
    let whitelist_addr = msg
        .whitelist
        .and_then(|w| deps.api.addr_validate(w.as_str()).ok());

    let config = Config {
        admin: info.sender.clone(),
        max_num_tokens: msg.max_num_tokens,
        cw721_code_id: msg.cw721_code_id,
        unit_price: msg.unit_price,
        per_address_limit: msg.per_address_limit,
        whitelist: whitelist_addr,
        start_time: msg.start_time,
    };
    CONFIG.save(deps.storage, &config)?;
    MINTABLE_TOKEN_IDS.save(deps.storage, &vec![])?;
    
    let migration_status=MigrationStatus{
        done: false,
    };

    MIGRATION_STATUS.save(deps.storage, &migration_status)?;

    let response = match msg.cw721_address {
        Some(_addr) => {
            let cw721_address = deps.api.addr_validate(&_addr)?;
            CW721_ADDRESS.save(deps.storage, &cw721_address)?;
            Response::new()
        },
        None => {
            let cw721_instantiate_msg = msg.cw721_instantiate_msg.unwrap();
            // Submessage to instantiate cw721 contract
            let sub_msgs: Vec<SubMsg> = vec![SubMsg {
                msg: WasmMsg::Instantiate {
                    code_id: msg.cw721_code_id,
                    msg: to_binary(&Pg721InstantiateMsg {
                        name: cw721_instantiate_msg.name,
                        symbol: cw721_instantiate_msg.symbol,
                        minter: env.contract.address.to_string(),
                        collection_info: cw721_instantiate_msg.collection_info,
                    })?,
                    funds: info.funds.clone(),
                    admin: Some(info.sender.clone().to_string()),
                    label: String::from("Fixed price minter"),
                }
                .into(),
                id: INSTANTIATE_CW721_REPLY_ID,
                gas_limit: None,
                reply_on: ReplyOn::Success,
            }];
            Response::new().add_submessages(sub_msgs)
        }
    };

    Ok(response
        .add_attribute("action", "instantiate")
        .add_attribute("contract_name", CONTRACT_NAME)
        .add_attribute("contract_version", CONTRACT_VERSION)
        .add_attribute("sender", info.sender))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    let api = deps.api;

    match msg {
        ExecuteMsg::MigrateData { migrations } => migrate_tokens(deps, info, migrations.tokens, migrations.mintable_tokens, migrations.minters),
        ExecuteMsg::MigrationDone {  } => migrated(deps, info),
        _ => {
            if migration_status(deps.as_ref()){
                match msg {
                    ExecuteMsg::UpsertTokenMetadatas { token_metadatas } => execute_upsert_token_metadatas(deps, info, token_metadatas ),
                    ExecuteMsg::Mint {} => execute_mint_sender(deps, env, info),
                    ExecuteMsg::UpdateStartTime(time) => execute_update_start_time(deps, env, info, time),
                    ExecuteMsg::UpdatePerAddressLimit { per_address_limit } => {
                        execute_update_per_address_limit(deps, env, info, per_address_limit)
                    }
                    ExecuteMsg::UpdateUnitPrice { unit_price } => execute_update_unit_price(deps, env, info, unit_price),
                    ExecuteMsg::MintTo { recipient } => execute_mint_to(deps, env, info, recipient),
                    ExecuteMsg::MintFor {
                        token_id,
                        recipient,
                    } => execute_mint_for(deps, env, info, token_id, recipient),
                    ExecuteMsg::SetAdmin { admin } => execute_set_admin(deps, info, api.addr_validate(&admin)?),
                    ExecuteMsg::SetWhitelist { whitelist } => {
                        execute_set_whitelist(deps, env, info, &whitelist)
                    }
                    ExecuteMsg::Withdraw { recipient } => execute_withdraw(deps, env, info, api.addr_validate(&recipient)?),
                    ExecuteMsg::MigrateData { migrations:_ } => return Err(ContractError::MigrationDone {  }),
                    ExecuteMsg::MigrationDone {  } => migrated(deps, info),
                }
            } else {
                return Err(ContractError::MigrationInProgress {  })
            }
        }
    }
}

fn migrated(
    deps: DepsMut,
    info: MessageInfo,
)->Result<Response,ContractError>{

    let config = CONFIG.load(deps.storage)?;
    if config.admin != info.sender {
        return Err(ContractError::Unauthorized(
            "Sender is not an admin".to_owned(),
        ));
    };

    migration_done(deps)?;
    
    Ok(Response::new()
        .add_attribute("action", "migration_done")
        .add_attribute("marked done as", true.to_string())
    )
}

pub fn execute_upsert_token_metadatas(
    deps: DepsMut,
    info: MessageInfo,
    token_metadatas: Vec<TokenMetadata>
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if config.admin != info.sender {
        return Err(ContractError::Unauthorized(
            "Sender is not an admin".to_owned(),
        ));
    };

    let mut append_token_ids = vec![];
    for token_metadata in token_metadatas {
        if token_metadata.token_id == 0 {
            return Err(ContractError::InvalidTokenId {});
        }
        token_mints().update(
            deps.storage,
            token_metadata.token_id.clone(),
            |existing_token_mint| -> Result<TokenMint, ContractError> {
                if let Some(_existing_token_mint) = existing_token_mint {
                    if let true = _existing_token_mint.is_minted {
                        return Err(ContractError::TokenAlreadyMinted { token_id: _existing_token_mint.token_id });
                    }
                };
                Ok(TokenMint {
                    token_id: token_metadata.clone().token_id,
                    metadata: token_metadata.clone().metadata,
                    is_minted: false,
                })
            }
        )?;
        append_token_ids.push(token_metadata.token_id);
    }

    let mut mintable_token_ids = MINTABLE_TOKEN_IDS.load(deps.storage)?;
    mintable_token_ids.append(&mut append_token_ids.clone());
    MINTABLE_TOKEN_IDS.save(deps.storage, &mintable_token_ids)?;

    let mut response = Response::new();
    let append_token_ids_fmt: Vec<String> = append_token_ids
        .into_iter().map(|token_id| token_id.to_string()).collect();
    let event = Event::new("upsert-metadata")
        .add_attribute("append-token-ids", append_token_ids_fmt.join(", "));
    response.events.push(event);

    Ok(response)
}

fn migrate_tokens(
    deps: DepsMut,
    info: MessageInfo,
    tokens: Option<Vec<TokenMint>>,
    mintable_ids: Option<Vec<u32>>,
    minters: Option<Vec<Minter>>
) -> Result<Response, ContractError> {

    if migration_status(deps.as_ref()){
        return Err(ContractError::MigrationDone {  });
    }
    
    let config = CONFIG.load(deps.storage)?;
    if config.admin != info.sender {
        return Err(ContractError::Unauthorized(
            "Sender is not an admin".to_owned(),
        ));
    };

    // migrate token mints
    let mut append_token_ids = vec![];
    if tokens.is_some(){
        for token in tokens.unwrap_or_default() {
            if token.token_id == 0 {
                return Err(ContractError::InvalidTokenId {});
            }
            token_mints().update(
                deps.storage,
                token.token_id.clone(),
                |existing_token_mint| -> Result<TokenMint, ContractError> {
                    if let Some(_existing_token_mint) = existing_token_mint {
                        if let true = _existing_token_mint.is_minted {
                            return Err(ContractError::TokenAlreadyMinted { token_id: _existing_token_mint.token_id });
                        }
                    };
                    Ok(TokenMint {
                        token_id: token.clone().token_id,
                        metadata: token.clone().metadata,
                        is_minted: token.is_minted,
                    })
                }
            )?;
            append_token_ids.push(token.token_id);
        }    
    }

    // migrate minters
    if minters.is_some(){
        for minter in minters.clone().unwrap_or_default() {
            MINTER_ADDRS.save(deps.storage, Addr::unchecked(minter.address), &minter.mints)?;    
        }
    }
    

    // migrate mintable tokens
    if mintable_ids.is_some(){
        let mut mintable_token_ids = MINTABLE_TOKEN_IDS.load(deps.storage)?;
       mintable_token_ids.append(&mut &mut mintable_ids.clone().unwrap_or_default());
        MINTABLE_TOKEN_IDS.save(deps.storage, &mintable_token_ids)?;
    }
    


    // response
    let mut response = Response::new();
    let append_token_ids_fmt: Vec<String> = append_token_ids
        .into_iter().map(|token_id| token_id.to_string()).collect();
    let mut event = Event::new("migrate data")
        .add_attribute("mintable-ids",mintable_ids.is_some().to_string())
        .add_attribute("minters", minters.is_some().to_string());
    
    if append_token_ids_fmt.len()>0{
        event=event.add_attribute("token-ids",append_token_ids_fmt.join(","))
    }

    response.events.push(event);

    Ok(response)
}

pub fn execute_withdraw(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    recipient: Addr,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if config.admin != info.sender {
        return Err(ContractError::Unauthorized(
            "Sender is not an admin".to_owned(),
        ));
    };

    // query balance from the contract
    let balance = deps
        .querier
        .query_balance(env.contract.address, config.unit_price.denom)?;
    if balance.amount.is_zero() {
        return Err(ContractError::ZeroBalance {});
    }

    // send contract balance to creator
    let send_msg = CosmosMsg::Bank(BankMsg::Send {
        to_address: recipient.to_string(),
        amount: vec![balance],
    });

    Ok(Response::default()
        .add_attribute("action", "withdraw")
        .add_message(send_msg))
}

pub fn execute_set_admin(
    deps: DepsMut,
    info: MessageInfo,
    admin: Addr,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    if config.admin != info.sender {
        return Err(ContractError::Unauthorized(
            "Sender is not an admin".to_owned(),
        ));
    };

    config.admin = admin.clone();
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::default()
        .add_attribute("action", "set_admin")
        .add_attribute("admin", admin.to_string()))
}

pub fn execute_set_whitelist(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    whitelist: &str,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    if config.admin != info.sender {
        return Err(ContractError::Unauthorized(
            "Sender is not an admin".to_owned(),
        ));
    };

    if env.block.time >= config.start_time {
        return Err(ContractError::AlreadyStarted {});
    }

    if let Some(wl) = config.whitelist {
        let res: WhitelistConfigResponse = deps
            .querier
            .query_wasm_smart(wl, &WhitelistQueryMsg::Config {})?;

        if res.is_active {
            return Err(ContractError::WhitelistAlreadyStarted {});
        }
    }

    config.whitelist = Some(deps.api.addr_validate(whitelist)?);
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::default()
        .add_attribute("action", "set_whitelist")
        .add_attribute("whitelist", whitelist.to_string()))
}

pub fn execute_mint_sender(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let action = "mint_sender";

    // If there is no active whitelist right now, check public mint
    // Check if after start_time
    if is_public_mint(deps.as_ref(), &info, &config)? && (env.block.time < config.start_time) {
        return Err(ContractError::BeforeMintStartTime {});
    }

    // Check if already minted max per address limit
    let mint_count = mint_count(deps.as_ref(), &info)?;
    if mint_count >= config.per_address_limit {
        return Err(ContractError::MaxPerAddressLimitExceeded {});
    }

    _execute_mint(deps, env, info, &config, action, false, None, None)
}

// Check if a whitelist exists and not ended
// Sender has to be whitelisted to mint
fn is_public_mint(deps: Deps, info: &MessageInfo, config: &Config) -> Result<bool, ContractError> {
    let config = config.clone();

    // If there is no whitelist, there's only a public mint
    if config.whitelist.is_none() {
        return Ok(true);
    }

    let whitelist = config.whitelist.unwrap();

    let wl_config: WhitelistConfigResponse = deps
        .querier
        .query_wasm_smart(whitelist.clone(), &WhitelistQueryMsg::Config {})?;

    if !wl_config.is_active {
        return Ok(true);
    }

    let res: HasMemberResponse = deps.querier.query_wasm_smart(
        whitelist,
        &WhitelistQueryMsg::HasMember {
            member: info.sender.to_string(),
        },
    )?;
    if !res.has_member {
        return Err(ContractError::NotWhitelisted {
            addr: info.sender.to_string(),
        });
    }

    // Check wl per address limit
    let mint_count = mint_count(deps, info)?;
    if mint_count >= wl_config.per_address_limit {
        return Err(ContractError::MaxPerAddressLimitExceeded {});
    }

    Ok(false)
}

pub fn execute_mint_to(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    recipient: String,
) -> Result<Response, ContractError> {
    let recipient = deps.api.addr_validate(&recipient)?;
    let config = CONFIG.load(deps.storage)?;
    let action = "mint_to";

    // Check only admin
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized(
            "Sender is not an admin".to_owned(),
        ));
    }

    _execute_mint(deps, env, info, &config, action, true, Some(recipient), None)
}

pub fn execute_mint_for(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    token_id: u32,
    recipient: String,
) -> Result<Response, ContractError> {
    let recipient = deps.api.addr_validate(&recipient)?;
    let config = CONFIG.load(deps.storage)?;
    let action = "mint_for";

    // Check only admin
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized(
            "Sender is not an admin".to_owned(),
        ));
    }

    _execute_mint(deps, env, info, &config, action, true, Some(recipient), Some(token_id))
}

// Generalize checks and mint message creation
// mint -> _execute_mint(recipient: None, token_id: None)
// mint_to(recipient: "friend") -> _execute_mint(Some(recipient), token_id: None)
// mint_for(recipient: "friend2", token_id: 420) -> _execute_mint(recipient, token_id)
fn _execute_mint(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    config: &Config,
    action: &str,
    is_admin: bool,
    recipient: Option<Addr>,
    token_id: Option<u32>,
) -> Result<Response, ContractError> {
    let cw721_address = CW721_ADDRESS.load(deps.storage)?;

    let recipient_addr = match recipient {
        Some(some_recipient) => some_recipient,
        None => info.sender.clone(),
    };

    let mint_price: Coin = mint_price(deps.as_ref(), &config, is_admin)?;
    // Exact payment only accepted
    let payment = may_pay(&info, &config.unit_price.denom)?;
    if payment != mint_price.amount {
        return Err(ContractError::IncorrectPaymentAmount(
            coin(payment.u128(), &config.unit_price.denom),
            mint_price,
        ));
    }

    let mut mintable_token_ids = MINTABLE_TOKEN_IDS.load(deps.storage)?;
    if mintable_token_ids.is_empty() {
        return Err(ContractError::SoldOut {});
    }

    let mintable_token_position = match token_id {
        Some(token_id) => {
            if token_id == 0 {
                return Err(ContractError::InvalidTokenId {});
            }
            match mintable_token_ids.iter().position(|_id| *_id == token_id) {
                Some(position) => position,
                None => return Err(ContractError::TokenAlreadyMinted { token_id })
            }
        }
        None => {
            env.block.time.nanos() as usize % mintable_token_ids.len()
        }
    };

    let mintable_token_id = mintable_token_ids[mintable_token_position];
    let token_mint = token_mints().load(deps.storage, mintable_token_id)?;
    if token_mint.is_minted {
        return Err(ContractError::TokenAlreadyMinted { token_id: mintable_token_id });
    }

    // Create mint msgs
    let mint_msg = Pg721ExecuteMsg::Mint(MintMsg::<Option<Metadata>> {
        token_id: mintable_token_id.to_string(),
        owner: recipient_addr.to_string(),
        token_uri: None,
        extension: Some(token_mint.metadata),
    });
    let msg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: cw721_address.to_string(),
        msg: to_binary(&mint_msg)?,
        funds: vec![],
    });

    // Mark TokenMint as minted
    token_mints().update(
        deps.storage,
        mintable_token_id,
        |token_mint| -> Result<TokenMint, StdError> {
            let mut updated_token_mint = token_mint.unwrap();
            updated_token_mint.is_minted = true;
            Ok(updated_token_mint)
        }
    )?;

    // Remove mintable token id
    mintable_token_ids.remove(mintable_token_position);
    MINTABLE_TOKEN_IDS.save(deps.storage, &mintable_token_ids)?;

    // Save the new mint count for the sender's address
    let new_mint_count = mint_count(deps.as_ref(), &info)? + 1;
    MINTER_ADDRS.save(deps.storage, info.clone().sender, &new_mint_count)?;

    Ok(Response::default()
        .add_attribute("action", action)
        .add_attribute("sender", info.sender)
        .add_attribute("recipient", recipient_addr)
        .add_attribute("token_id", mintable_token_id.to_string())
        .add_attribute("mint_price", mint_price.amount)
        .add_message(msg))
}

pub fn execute_update_start_time(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    start_time: Timestamp,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized(
            "Sender is not an admin".to_owned(),
        ));
    }
    // If current time is after the stored start time return error
    if env.block.time >= config.start_time {
        return Err(ContractError::AlreadyStarted {});
    }

    // If current time already passed the new start_time return error
    if env.block.time > start_time {
        return Err(ContractError::InvalidStartTime(start_time, env.block.time));
    }

    config.start_time = start_time;
    CONFIG.save(deps.storage, &config)?;
    Ok(Response::new()
        .add_attribute("action", "update_start_time")
        .add_attribute("sender", info.sender)
        .add_attribute("start_time", start_time.to_string()))
}

pub fn execute_update_per_address_limit(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    per_address_limit: u32,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized(
            "Sender is not an admin".to_owned(),
        ));
    }
    if per_address_limit == 0 {
        return Err(ContractError::InvalidPerAddressLimit {
            min: 1,
            got: per_address_limit,
        });
    }
    config.per_address_limit = per_address_limit;
    CONFIG.save(deps.storage, &config)?;
    Ok(Response::new()
        .add_attribute("action", "update_per_address_limit")
        .add_attribute("sender", info.sender)
        .add_attribute("limit", per_address_limit.to_string()))
}

pub fn execute_update_unit_price(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    unit_price: Coin,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized(
            "Sender is not an admin".to_owned(),
        ));
    }

    config.unit_price = unit_price.clone();
    CONFIG.save(deps.storage, &config)?;
    Ok(Response::new()
        .add_attribute("action", "update_unit_price")
        .add_attribute("sender", info.sender)
        .add_attribute("unit_price", unit_price.to_string()))
}

// if admin_no_fee => no fee,
// else if in whitelist => whitelist price
// else => config unit price
pub fn mint_price(deps: Deps, config: &Config, is_admin: bool) -> Result<Coin, StdError> {
    let config = config.clone();
    if is_admin {
        return Ok(coin(0, config.unit_price.denom));
    }

    if config.whitelist.is_none() {
        return Ok(config.unit_price);
    }

    let whitelist = config.whitelist.unwrap();

    let wl_config: WhitelistConfigResponse = deps
        .querier
        .query_wasm_smart(whitelist, &WhitelistQueryMsg::Config {})?;

    if wl_config.is_active {
        Ok(wl_config.unit_price)
    } else {
        Ok(config.unit_price)
    }
}

fn mint_count(deps: Deps, info: &MessageInfo) -> Result<u32, StdError> {
    let mint_count = (MINTER_ADDRS
        .key(info.sender.clone())
        .may_load(deps.storage)?)
    .unwrap_or(0);
    Ok(mint_count)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::StartTime {} => to_binary(&query_start_time(deps)?),
        QueryMsg::NumMinted {} => to_binary(&query_num_minted(deps)?),
        QueryMsg::NumRemaining {} => to_binary(&query_num_remaining(deps)?),
        QueryMsg::MintPrice {} => to_binary(&query_mint_price(deps)?),
        QueryMsg::MintCount { address } => to_binary(&query_mint_count(deps, address)?),
        QueryMsg::TokenMint { token_id } => to_binary(&query_token_mint(deps, token_id)?),
        QueryMsg::TokenMints { descending, filter_minted, start_after, limit } =>
            to_binary(&query_token_mints(deps, descending, filter_minted, start_after, limit)?),
    }
}

fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config = CONFIG.load(deps.storage)?;
    let cw721_address = CW721_ADDRESS.load(deps.storage)?;

    Ok(ConfigResponse {
        admin: config.admin.to_string(),
        cw721_address: cw721_address.to_string(),
        cw721_code_id: config.cw721_code_id,
        max_num_tokens: config.max_num_tokens,
        start_time: config.start_time,
        unit_price: config.unit_price,
        per_address_limit: config.per_address_limit,
        whitelist: config.whitelist.map(|w| w.to_string()),
    })
}

fn query_mint_count(deps: Deps, address: String) -> StdResult<MintCountResponse> {
    let addr = deps.api.addr_validate(&address)?;
    let mint_count = (MINTER_ADDRS.key(addr.clone()).may_load(deps.storage)?).unwrap_or(0);
    Ok(MintCountResponse {
        address: addr.to_string(),
        count: mint_count,
    })
}

fn query_start_time(deps: Deps) -> StdResult<StartTimeResponse> {
    let config = CONFIG.load(deps.storage)?;
    Ok(StartTimeResponse {
        start_time: config.start_time.to_string(),
    })
}

fn query_num_minted(deps: Deps) -> StdResult<NumMintedResponse> {
    let config = CONFIG.load(deps.storage)?;
    let mintable_token_ids = MINTABLE_TOKEN_IDS.load(deps.storage)?;
    let num_minted: u32 = config.max_num_tokens - mintable_token_ids.len() as u32;
    return Ok(NumMintedResponse { num_minted });
}

fn query_num_remaining(deps: Deps) -> StdResult<NumRemainingResponse> {
    let mintable_token_ids = MINTABLE_TOKEN_IDS.load(deps.storage)?;
    Ok(NumRemainingResponse { num_remaining: mintable_token_ids.len() as u32 })
}

fn query_mint_price(deps: Deps) -> StdResult<MintPriceResponse> {
    let config = CONFIG.load(deps.storage)?;
    let current_price = mint_price(deps, &config, false)?;
    let public_price = config.unit_price;
    let whitelist_price: Option<Coin> = if let Some(whitelist) = config.whitelist {
        let wl_config: WhitelistConfigResponse = deps
            .querier
            .query_wasm_smart(whitelist, &WhitelistQueryMsg::Config {})?;
        Some(wl_config.unit_price)
    } else {
        None
    };
    Ok(MintPriceResponse {
        current_price,
        public_price,
        whitelist_price,
    })
}

fn query_token_mint(deps: Deps, token_id: u32) -> StdResult<TokenMintResponse> {
    let token_mint = token_mints().may_load(deps.storage, token_id)?;
    Ok(TokenMintResponse { token_mint })
}

fn query_token_mints(
    deps: Deps,
    descending: Option<bool>,
    filter_minted: Option<bool>,
    start_after: Option<u32>,
    limit: Option<u32>
) -> StdResult<TokenMintsResponse> {
    let limit = limit.unwrap_or(DEFAULT_QUERY_LIMIT).min(MAX_QUERY_LIMIT) as usize;
    let start = start_after.as_ref().map(|offset| {
        Bound::exclusive(*offset)
    });
    let order = match descending {
        Some(_descending) => if _descending { Order::Descending } else { Order::Ascending },
        _ => Order::Ascending
    };

    let token_mints = token_mints()
        .range(deps.storage, start, None, order)
        .filter(|item| match item {
            Ok((_, token_mint)) => match filter_minted {
                Some(_filter_minted) => !_filter_minted || !token_mint.is_minted,
                _ => true,
            },
            Err(_) => true,
        })
        .take(limit)
        .map(|res| res.map(|item| item.1))
        .collect::<StdResult<Vec<_>>>()?;

    Ok(TokenMintsResponse { token_mints })
}

// Reply callback triggered from cw721 contract instantiation
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, _env: Env, msg: Reply) -> Result<Response, ContractError> {
    if msg.id != INSTANTIATE_CW721_REPLY_ID {
        return Err(ContractError::InvalidReplyID {});
    }

    let reply = parse_reply_instantiate_data(msg);
    match reply {
        Ok(res) => {
            CW721_ADDRESS.save(deps.storage, &Addr::unchecked(res.contract_address))?;
            Ok(Response::default().add_attribute("action", "instantiate_cw721_reply"))
        }
        Err(_) => Err(ContractError::InstantiatePg721Error {}),
    }
}

#[entry_point]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    let storage_version: &str = &get_contract_version(deps.storage)?.version.to_string();

    let mut response = Response::new();
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let event = Event::new("contract-migrated")
        .add_attribute("prev-version", storage_version)
        .add_attribute("next-version", CONTRACT_VERSION);
    response.events.push(event);
    Ok(response)
}
