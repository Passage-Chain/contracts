use crate::error::ContractError;
use crate::msg::{
    AddMembersMsg, ConfigResponse, ExecuteMsg, HasEndedResponse, HasMemberResponse,
    HasStartedResponse, InstantiateMsg, IsActiveResponse, MembersResponse, QueryMsg,
    RemoveMembersMsg,
};
use crate::state::{Config, CONFIG, WHITELIST};
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{to_binary, Binary, Deps, DepsMut, Env, MessageInfo, StdResult, Response};
use cosmwasm_std::{Order, Timestamp};
use cw2::set_contract_version;
use cw_storage_plus::Bound;
use cw_utils::{maybe_addr};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:passage-whitelist";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

// queries
const PAGINATION_DEFAULT_LIMIT: u32 = 25;
const PAGINATION_MAX_LIMIT: u32 = 100;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    mut msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    if msg.member_limit == 0 {
        return Err(ContractError::InvalidMemberLimit {
            min: 1,
            got: msg.member_limit,
        });
    }

    if msg.unit_price.amount.u128() == 0 {
        return Err(ContractError::InvalidUnitPrice(
            msg.unit_price.amount.u128(),
        ));
    }

    if msg.per_address_limit == 0 {
        return Err(ContractError::InvalidPerAddressLimit {
            max: "must be > 0".to_string(),
            got: msg.per_address_limit.to_string(),
        });
    }

    // remove duplicate members
    msg.members.sort_unstable();
    msg.members.dedup();

    let config = Config {
        admin: info.sender.clone(),
        start_time: msg.start_time,
        end_time: msg.end_time,
        num_members: msg.members.len() as u32,
        unit_price: msg.unit_price,
        per_address_limit: msg.per_address_limit,
        member_limit: msg.member_limit,
    };
    CONFIG.save(deps.storage, &config)?;

    if msg.start_time >= msg.end_time {
        return Err(ContractError::InvalidStartTime(
            msg.start_time,
            msg.end_time,
        ));
    }

    if env.block.time >= msg.start_time {
        return Err(ContractError::InvalidStartTime(
            env.block.time,
            msg.start_time,
        ));
    }

    if config.member_limit < config.num_members {
        return Err(ContractError::MembersExceeded {
            expected: config.member_limit,
            actual: config.num_members,
        });
    }

    for member in msg.members.into_iter() {
        let addr = deps.api.addr_validate(&member.clone())?;
        WHITELIST.save(deps.storage, addr, &true)?;
    }

    Ok(Response::new()
        .add_attribute("action", "instantiate")
        .add_attribute("contract_name", CONTRACT_NAME)
        .add_attribute("contract_version", CONTRACT_VERSION)
        .add_attribute("sender", info.sender)
    )
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::UpdateStartTime(time) => execute_update_start_time(deps, env, info, time),
        ExecuteMsg::UpdateEndTime(time) => execute_update_end_time(deps, env, info, time),
        ExecuteMsg::AddMembers(msg) => execute_add_members(deps, env, info, msg),
        ExecuteMsg::RemoveMembers(msg) => execute_remove_members(deps, env, info, msg),
        ExecuteMsg::UpdatePerAddressLimit(per_address_limit) => {
            execute_update_per_address_limit(deps, info, per_address_limit)
        }
        ExecuteMsg::IncreaseMemberLimit(member_limit) => {
            execute_increase_member_limit(deps, info, member_limit)
        }
    }
}

pub fn execute_update_start_time(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    start_time: Timestamp,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized {});
    }

    // don't allow updating start time if whitelist is active
    if env.block.time >= config.start_time {
        return Err(ContractError::AlreadyStarted {});
    }

    if start_time > config.end_time {
        return Err(ContractError::InvalidStartTime(start_time, config.end_time));
    }

    config.start_time = start_time;
    CONFIG.save(deps.storage, &config)?;
    Ok(Response::new()
        .add_attribute("action", "update_start_time")
        .add_attribute("start_time", start_time.to_string())
        .add_attribute("sender", info.sender))
}

pub fn execute_update_end_time(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    end_time: Timestamp,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized {});
    }

    // don't allow updating end time if whitelist is active
    if env.block.time >= config.start_time {
        return Err(ContractError::AlreadyStarted {});
    }

    if end_time < config.start_time {
        return Err(ContractError::InvalidEndTime(end_time, config.start_time));
    }

    config.end_time = end_time;
    CONFIG.save(deps.storage, &config)?;
    Ok(Response::new()
        .add_attribute("action", "update_end_time")
        .add_attribute("end_time", end_time.to_string())
        .add_attribute("sender", info.sender))
}

pub fn execute_add_members(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    mut msg: AddMembersMsg,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized {});
    }

    // remove duplicate members
    msg.to_add.sort_unstable();
    msg.to_add.dedup();

    for add in msg.to_add.into_iter() {
        if config.num_members >= config.member_limit {
            return Err(ContractError::MembersExceeded {
                expected: config.member_limit,
                actual: config.num_members,
            });
        }
        let addr = deps.api.addr_validate(&add)?;
        if WHITELIST.has(deps.storage, addr.clone()) {
            return Err(ContractError::DuplicateMember(addr.to_string()));
        }
        WHITELIST.save(deps.storage, addr, &true)?;
        config.num_members += 1;
    }

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new()
        .add_attribute("action", "add_members")
        .add_attribute("sender", info.sender))
}

pub fn execute_remove_members(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: RemoveMembersMsg,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized {});
    }

    if env.block.time >= config.start_time {
        return Err(ContractError::AlreadyStarted {});
    }

    for remove in msg.to_remove.into_iter() {
        let addr = deps.api.addr_validate(&remove)?;
        if !WHITELIST.has(deps.storage, addr.clone()) {
            return Err(ContractError::NoMemberFound(addr.to_string()));
        }
        WHITELIST.remove(deps.storage, addr);
        config.num_members -= 1;
    }

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new()
        .add_attribute("action", "remove_members")
        .add_attribute("sender", info.sender))
}

pub fn execute_update_per_address_limit(
    deps: DepsMut,
    info: MessageInfo,
    per_address_limit: u32,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized {});
    }

    config.per_address_limit = per_address_limit;
    CONFIG.save(deps.storage, &config)?;
    Ok(Response::new()
        .add_attribute("action", "update_per_address_limit")
        .add_attribute("per_address_limit", per_address_limit.to_string()))
}

/// Increase member limit. Must include a fee if crossing 1000, 2000, etc member limit.
pub fn execute_increase_member_limit(
    deps: DepsMut,
    _info: MessageInfo,
    member_limit: u32,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    if config.member_limit >= member_limit {
        return Err(ContractError::InvalidMemberLimit {
            min: config.member_limit,
            got: member_limit,
        });
    }

    config.member_limit = member_limit;
    CONFIG.save(deps.storage, &config)?;
    Ok(Response::new()
        .add_attribute("action", "increase_member_limit")
        .add_attribute("member_limit", member_limit.to_string())
    )
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Members { start_after, limit } => {
            to_binary(&query_members(deps, start_after, limit)?)
        }

        QueryMsg::HasStarted {} => to_binary(&query_has_started(deps, env)?),
        QueryMsg::HasEnded {} => to_binary(&query_has_ended(deps, env)?),
        QueryMsg::IsActive {} => to_binary(&query_is_active(deps, env)?),
        QueryMsg::HasMember { member } => to_binary(&query_has_member(deps, member)?),
        QueryMsg::Config {} => to_binary(&query_config(deps, env)?),
    }
}

fn query_has_started(deps: Deps, env: Env) -> StdResult<HasStartedResponse> {
    let config = CONFIG.load(deps.storage)?;
    Ok(HasStartedResponse {
        has_started: (env.block.time >= config.start_time),
    })
}

fn query_has_ended(deps: Deps, env: Env) -> StdResult<HasEndedResponse> {
    let config = CONFIG.load(deps.storage)?;
    Ok(HasEndedResponse {
        has_ended: (env.block.time >= config.end_time),
    })
}

fn query_is_active(deps: Deps, env: Env) -> StdResult<IsActiveResponse> {
    let config = CONFIG.load(deps.storage)?;
    Ok(IsActiveResponse {
        is_active: (env.block.time >= config.start_time) && (env.block.time < config.end_time),
    })
}

fn query_members(
    deps: Deps,
    start_after: Option<String>,
    limit: Option<u32>,
) -> StdResult<MembersResponse> {
    let limit = limit
        .unwrap_or(PAGINATION_DEFAULT_LIMIT)
        .min(PAGINATION_MAX_LIMIT) as usize;
    let start_addr = maybe_addr(deps.api, start_after)?;
    let start = start_addr.map(Bound::exclusive);
    let members = WHITELIST
        .range(deps.storage, start, None, Order::Ascending)
        .take(limit)
        .map(|addr| addr.unwrap().0.to_string())
        .collect::<Vec<String>>();

    Ok(MembersResponse { members })
}

fn query_has_member(deps: Deps, member: String) -> StdResult<HasMemberResponse> {
    let addr = deps.api.addr_validate(&member)?;

    Ok(HasMemberResponse {
        has_member: WHITELIST.has(deps.storage, addr),
    })
}

fn query_config(deps: Deps, env: Env) -> StdResult<ConfigResponse> {
    let config = CONFIG.load(deps.storage)?;
    Ok(ConfigResponse {
        num_members: config.num_members,
        per_address_limit: config.per_address_limit,
        member_limit: config.member_limit,
        start_time: config.start_time,
        end_time: config.end_time,
        unit_price: config.unit_price,
        is_active: (env.block.time >= config.start_time) && (env.block.time < config.end_time),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::{
        coin,
        testing::{mock_dependencies, mock_env, mock_info},
        Attribute
    };

    const ADMIN: &str = "admin";
    const NATIVE_DENOM: &str = "ujuno";
    const UNIT_AMOUNT: u128 = 100_000_000;

    const START_TIME: Timestamp = Timestamp::from_nanos(1647032400000000000);
    const END_TIME: Timestamp = START_TIME.plus_seconds(1);

    fn setup_contract(deps: DepsMut) {
        let msg = InstantiateMsg {
            members: vec!["adsfsa".to_string()],
            start_time: START_TIME,
            end_time: END_TIME,
            unit_price: coin(UNIT_AMOUNT, NATIVE_DENOM),
            per_address_limit: 1,
            member_limit: 1000,
        };
        let info = mock_info(ADMIN, &[coin(100_000_000, "ujuno")]);
        let res = instantiate(deps, mock_env(), info.clone(), msg).unwrap();
        assert!(res.attributes[0].eq(&Attribute::new("action", "instantiate")));
        assert!(res.attributes[1].eq(&Attribute::new("contract_name", CONTRACT_NAME)));
        assert!(res.attributes[2].eq(&Attribute::new("contract_version", CONTRACT_VERSION)));
        assert!(res.attributes[3].eq(&Attribute::new("sender", info.sender.into_string())));
    }

    #[test]
    fn proper_initialization() {
        let mut deps = mock_dependencies();
        setup_contract(deps.as_mut());
    }

    #[test]
    fn improper_initialization() {
        let mut deps = mock_dependencies();
        let msg = InstantiateMsg {
            members: vec!["adsfsa".to_string()],
            start_time: END_TIME,
            end_time: END_TIME,
            unit_price: coin(1, NATIVE_DENOM),
            per_address_limit: 1,
            member_limit: 1000,
        };
        let info = mock_info(ADMIN, &[coin(100_000_000, "ujuno")]);
        instantiate(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    }

    #[test]
    fn improper_initialization_dedup() {
        let mut deps = mock_dependencies();
        let msg = InstantiateMsg {
            members: vec![
                "adsfsa".to_string(),
                "adsfsa".to_string(),
                "adsfsa".to_string(),
            ],
            start_time: START_TIME,
            end_time: END_TIME,
            unit_price: coin(UNIT_AMOUNT, NATIVE_DENOM),
            per_address_limit: 1,
            member_limit: 1000,
        };
        let info = mock_info(ADMIN, &[coin(100_000_000, "ujuno")]);
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        let res = query_config(deps.as_ref(), mock_env()).unwrap();
        assert_eq!(1, res.num_members);
    }

    #[test]
    fn check_start_time_after_end_time() {
        let msg = InstantiateMsg {
            members: vec!["adsfsa".to_string()],
            start_time: END_TIME,
            end_time: START_TIME,
            unit_price: coin(UNIT_AMOUNT, NATIVE_DENOM),
            per_address_limit: 1,
            member_limit: 1000,
        };
        let info = mock_info(ADMIN, &[coin(100_000_000, "ujuno")]);
        let mut deps = mock_dependencies();
        instantiate(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    }

    #[test]
    fn update_start_time() {
        let mut deps = mock_dependencies();
        setup_contract(deps.as_mut());

        let new_start_time = START_TIME.minus_nanos(100);
        let msg = ExecuteMsg::UpdateStartTime(new_start_time);
        let info = mock_info(ADMIN, &[]);
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(res.attributes.len(), 3);
        let res = query_config(deps.as_ref(), mock_env()).unwrap();
        assert_eq!(res.start_time, new_start_time);
    }

    #[test]
    fn update_end_time() {
        let mut deps = mock_dependencies();
        setup_contract(deps.as_mut());

        let new_end_time = START_TIME.plus_nanos(300);
        let msg = ExecuteMsg::UpdateEndTime(new_end_time);
        let info = mock_info(ADMIN, &[]);
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(res.attributes.len(), 3);
        let res = query_config(deps.as_ref(), mock_env()).unwrap();
        assert_eq!(res.end_time, new_end_time);
    }

    #[test]
    fn update_members() {
        let mut deps = mock_dependencies();
        setup_contract(deps.as_mut());

        // dedupe addrs
        let add_msg = AddMembersMsg {
            to_add: vec!["adsfsa1".to_string(), "adsfsa1".to_string()],
        };
        let msg = ExecuteMsg::AddMembers(add_msg);
        let info = mock_info(ADMIN, &[]);
        let res = execute(deps.as_mut(), mock_env(), info.clone(), msg.clone()).unwrap();
        assert_eq!(res.attributes.len(), 2);
        let res = query_members(deps.as_ref(), None, None).unwrap();
        assert_eq!(res.members.len(), 2);

        execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap_err();

        let remove_msg = RemoveMembersMsg {
            to_remove: vec!["adsfsa1".to_string()],
        };
        let msg = ExecuteMsg::RemoveMembers(remove_msg);
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(res.attributes.len(), 2);
        let res = query_members(deps.as_ref(), None, None).unwrap();
        assert_eq!(res.members.len(), 1);
    }

    #[test]
    fn update_per_address_limit() {
        let mut deps = mock_dependencies();
        setup_contract(deps.as_mut());

        let per_address_limit: u32 = 2;
        let msg = ExecuteMsg::UpdatePerAddressLimit(per_address_limit);
        let info = mock_info(ADMIN, &[]);
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(res.attributes.len(), 2);
        let wl_config: ConfigResponse = query_config(deps.as_ref(), mock_env()).unwrap();
        assert_eq!(wl_config.per_address_limit, per_address_limit);
    }
    
    #[test]
    fn query_members_pagination() {
        let mut deps = mock_dependencies();
        let mut members = vec![];
        for i in 0..150 {
            members.push(format!("juno1{}", i));
        }
        let msg = InstantiateMsg {
            members: members.clone(),
            start_time: START_TIME,
            end_time: END_TIME,
            unit_price: coin(UNIT_AMOUNT, NATIVE_DENOM),
            per_address_limit: 1,
            member_limit: 1000,
        };
        let info = mock_info(ADMIN, &[coin(100_000_000, "ujuno")]);
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        let mut all_elements: Vec<String> = vec![];

        // enforcing a min
        let res = query_members(deps.as_ref(), None, None).unwrap();
        assert_eq!(res.members.len(), 25);

        // enforcing a max
        let res = query_members(deps.as_ref(), None, Some(125)).unwrap();
        assert_eq!(res.members.len(), 100);

        // first fetch
        let res = query_members(deps.as_ref(), None, Some(50)).unwrap();
        assert_eq!(res.members.len(), 50);
        all_elements.append(&mut res.members.clone());

        // second
        let res = query_members(
            deps.as_ref(),
            Some(res.members[res.members.len() - 1].clone()),
            Some(50),
        )
        .unwrap();
        assert_eq!(res.members.len(), 50);
        all_elements.append(&mut res.members.clone());

        // third
        let res = query_members(
            deps.as_ref(),
            Some(res.members[res.members.len() - 1].clone()),
            Some(50),
        )
        .unwrap();
        all_elements.append(&mut res.members.clone());
        assert_eq!(res.members.len(), 50);

        // check fetched items
        assert_eq!(all_elements.len(), 150);
        members.sort();
        all_elements.sort();
        assert_eq!(members, all_elements);
    }

    #[test]
    fn increase_member_limit() {
        let mut deps = mock_dependencies();
        setup_contract(deps.as_mut());
        let res = query_config(deps.as_ref(), mock_env()).unwrap();
        assert_eq!(1000, res.member_limit);

        let msg = ExecuteMsg::IncreaseMemberLimit(1001);
        let info = mock_info(ADMIN, &[]);
        let res = execute(deps.as_mut(), mock_env(), info, msg);
        assert!(res.is_ok());

        let msg = ExecuteMsg::IncreaseMemberLimit(1002);
        let info = mock_info(ADMIN, &[]);
        let res = execute(deps.as_mut(), mock_env(), info, msg);
        assert!(res.is_ok());
    }
}
