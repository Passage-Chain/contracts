use cosmwasm_std::testing::{mock_dependencies_with_balance, mock_env, mock_info};
use cosmwasm_std::{coin, coins, Addr, Decimal, Empty, Timestamp, Uint128, Attribute};
use cosmwasm_std::{Api, Coin};
use cw721::{Cw721QueryMsg, OwnerOfResponse, NftInfoResponse};
use cw721_base::ExecuteMsg as Cw721ExecuteMsg;
use cw_multi_test::{App, AppBuilder, BankSudo, Contract, ContractWrapper, Executor, SudoMsg};
use pg721_metadata_onchain::msg::{
    InstantiateMsg as Pg721InstantiateMsg, RoyaltyInfoResponse, Metadata
};
use pg721_metadata_onchain::state::CollectionInfo;
use whitelist::msg::InstantiateMsg as WhitelistInstantiateMsg;
use whitelist::msg::{AddMembersMsg, ExecuteMsg as WhitelistExecuteMsg};

use crate::contract::instantiate;
use crate::msg::{
    ConfigResponse, ExecuteMsg, InstantiateMsg, MintCountResponse, MintPriceResponse,
    QueryMsg, StartTimeResponse, ExecuteMsg as Pg721MinterExecuteMsg, TokenMetadata,
    TokenMintsResponse, NumMintedResponse, NumRemainingResponse
};
use crate::ContractError;

const NATIVE_DENOM: &str = "ujunox";
const USER: &str = "USER";
const CREATION_FEE: u128 = 1_000_000_000;
const INITIAL_BALANCE: u128 = 2_000_000_000;

const UNIT_PRICE: u128 = 100_000_000;
const WHITELIST_AMOUNT: u128 = 66_000_000;
const WL_PER_ADDRESS_LIMIT: u32 = 1;
const ADMIN_MINT_PRICE: u128 = 0;
const START_TIME: u64 = 1647032400000000000;

fn custom_mock_app() -> App {
    AppBuilder::new().build(|router, _, storage| {
        router
            .bank
            .init_balance(
                storage,
                &Addr::unchecked(USER),
                vec![Coin {
                    denom: NATIVE_DENOM.to_string(),
                    amount: Uint128::new(3_000_000),
                }],
            )
            .unwrap();
    })
}

pub fn contract_whitelist() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        whitelist::contract::execute,
        whitelist::contract::instantiate,
        whitelist::contract::query,
    );
    Box::new(contract)
}

pub fn contract_minter() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        crate::contract::execute,
        crate::contract::instantiate,
        crate::contract::query,
    )
    .with_reply(crate::contract::reply);
    Box::new(contract)
}

pub fn contract_cw721() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        pg721_metadata_onchain::contract::execute,
        pg721_metadata_onchain::contract::instantiate,
        pg721_metadata_onchain::contract::query,
    );
    Box::new(contract)
}

fn setup_whitelist_contract(router: &mut App, creator: &Addr) -> Addr {
    let whitelist_code_id = router.store_code(contract_whitelist());

    let msg = WhitelistInstantiateMsg {
        members: vec![],
        start_time: Timestamp::from_nanos(START_TIME + 100),
        end_time: Timestamp::from_nanos(START_TIME + 10000000),
        unit_price: coin(WHITELIST_AMOUNT, NATIVE_DENOM),
        per_address_limit: WL_PER_ADDRESS_LIMIT,
        member_limit: 1000,
    };
    router
        .instantiate_contract(
            whitelist_code_id,
            creator.clone(),
            &msg,
            &[coin(100_000_000, NATIVE_DENOM)],
            "whitelist",
            None,
        )
        .unwrap()
}

// Upload contract code and instantiate minter contract
fn setup_minter_contract(
    router: &mut App,
    creator: &Addr,
    num_tokens: u32,
) -> (Addr, ConfigResponse) {
    // Upload contract code
    let cw721_code_id = router.store_code(contract_cw721());
    let minter_code_id = router.store_code(contract_minter());
    let creation_fee = coins(CREATION_FEE, NATIVE_DENOM);

    // Instantiate minter contract
    let msg = InstantiateMsg {
        unit_price: coin(UNIT_PRICE, NATIVE_DENOM),
        max_num_tokens: num_tokens,
        start_time: Timestamp::from_nanos(START_TIME),
        per_address_limit: 5,
        whitelist: None,
        cw721_code_id,
        cw721_address: None,
        cw721_instantiate_msg: Some(Pg721InstantiateMsg {
            name: String::from("TEST"),
            symbol: String::from("TEST"),
            minter: creator.to_string(),
            collection_info: CollectionInfo {
                creator: creator.to_string(),
                description: String::from("Passage Monkeys"),
                image: "https://example.com/image.png".to_string(),
                external_link: Some("https://example.com/external.html".to_string()),
                royalty_info: Some(RoyaltyInfoResponse {
                    payment_address: creator.to_string(),
                    share: Decimal::percent(10),
                }),
            },
        }),
    };
    let minter_addr = router
        .instantiate_contract(
            minter_code_id,
            creator.clone(),
            &msg,
            &creation_fee,
            "Minter",
            None,
        )
        .unwrap();

    let config: ConfigResponse = router
        .wrap()
        .query_wasm_smart(minter_addr.clone(), &QueryMsg::Config {})
        .unwrap();

    (minter_addr, config)
}

// Upload contract code and instantiate minter contract
fn upsert_metadata(
    router: &mut App,
    admin: &Addr,
    minter_addr: &Addr,
    num_tokens: u32,
    start_index: Option<u32>,
) {
    let _start_index = match start_index {
        Some(_start_index) => _start_index,
        None => 1,
    };

    let mut token_metadatas = vec![];
    for idx in _start_index..(_start_index+num_tokens) {
        token_metadatas.push(TokenMetadata {
            token_id: idx,
            metadata: Metadata {
                image: Some(format!("image-{}.png", idx)),
                image_data: None,
                external_url: None,
                description: None,
                name: None,
                attributes: None,
                background_color: None,
                animation_url: None,
                youtube_url: None,
            }
        })
    }
    let upsert_message = Pg721MinterExecuteMsg::UpsertTokenMetadatas { token_metadatas };
    let res = router.execute_contract(admin.clone(), minter_addr.clone(), &upsert_message, &[]);
    assert!(res.is_ok());
}

// Add a creator account with initial balances
fn setup_accounts(router: &mut App) -> (Addr, Addr) {
    let buyer = Addr::unchecked("buyer");
    let creator = Addr::unchecked("creator");
    // 3,000 tokens
    let creator_funds = coins(INITIAL_BALANCE + CREATION_FEE, NATIVE_DENOM);
    // 2,000 tokens
    let buyer_funds = coins(INITIAL_BALANCE, NATIVE_DENOM);
    router
        .sudo(SudoMsg::Bank({
            BankSudo::Mint {
                to_address: creator.to_string(),
                amount: creator_funds.clone(),
            }
        }))
        .map_err(|err| println!("{:?}", err))
        .ok();

    router
        .sudo(SudoMsg::Bank({
            BankSudo::Mint {
                to_address: buyer.to_string(),
                amount: buyer_funds.clone(),
            }
        }))
        .map_err(|err| println!("{:?}", err))
        .ok();

    // Check native balances
    let creator_native_balances = router.wrap().query_all_balances(creator.clone()).unwrap();
    assert_eq!(creator_native_balances, creator_funds);

    // Check native balances
    let buyer_native_balances = router.wrap().query_all_balances(buyer.clone()).unwrap();
    assert_eq!(buyer_native_balances, buyer_funds);

    (creator, buyer)
}

// Set blockchain time to after mint by default
fn setup_block_time(router: &mut App, nanos: u64) {
    let mut block = router.block_info();
    block.time = Timestamp::from_nanos(nanos);
    router.set_block(block);
}

// Deal with zero and non-zero coin amounts for msgs
fn coins_for_msg(msg_coin: Coin) -> Vec<Coin> {
    if msg_coin.amount > Uint128::zero() {
        vec![msg_coin]
    } else {
        vec![]
    }
}

#[test]
fn initialization() {
    let mut deps = mock_dependencies_with_balance(&coins(2, "token"));

    // Check valid addr
    let addr = "earth1";
    let res = deps.api.addr_validate(&(*addr));
    assert!(res.is_ok());

    // 0 per address limit returns error
    let info = mock_info("creator", &coins(INITIAL_BALANCE, NATIVE_DENOM));
    let msg = InstantiateMsg {
        unit_price: coin(UNIT_PRICE, NATIVE_DENOM),
        max_num_tokens: 100,
        start_time: Timestamp::from_nanos(START_TIME),
        per_address_limit: 0,
        whitelist: None,
        cw721_code_id: 1,
        cw721_address: None,
        cw721_instantiate_msg: Some(Pg721InstantiateMsg {
            name: String::from("TEST"),
            symbol: String::from("TEST"),
            minter: info.sender.to_string(),
            collection_info: CollectionInfo {
                creator: info.sender.to_string(),
                description: String::from("Passage Monkeys"),
                image: "https://example.com/image.png".to_string(),
                external_link: Some("https://example.com/external.html".to_string()),
                royalty_info: Some(RoyaltyInfoResponse {
                    payment_address: info.sender.to_string(),
                    share: Decimal::percent(10),
                }),
            },
        }),
    };
    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap_err();

    // Under min token limit
    let info = mock_info("creator", &coins(INITIAL_BALANCE, NATIVE_DENOM));
    let msg = InstantiateMsg {
        unit_price: coin(UNIT_PRICE, NATIVE_DENOM),
        max_num_tokens: 0,
        start_time: Timestamp::from_nanos(START_TIME),
        per_address_limit: 5,
        whitelist: None,
        cw721_code_id: 1,
        cw721_address: None,
        cw721_instantiate_msg: Some(Pg721InstantiateMsg {
            name: String::from("TEST"),
            symbol: String::from("TEST"),
            minter: info.sender.to_string(),
            collection_info: CollectionInfo {
                creator: info.sender.to_string(),
                description: String::from("Passage Monkeys"),
                image: "https://example.com/image.png".to_string(),
                external_link: Some("https://example.com/external.html".to_string()),
                royalty_info: Some(RoyaltyInfoResponse {
                    payment_address: info.sender.to_string(),
                    share: Decimal::percent(10),
                }),
            },
        }),
    };
    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap_err();
}

#[test]
fn initialization_with_cw721_addr() {
    let mut router = custom_mock_app();
    let (creator, _buyer) = setup_accounts(&mut router);

    // Upload contract code
    let cw721_code_id = router.store_code(contract_cw721());
    let minter_code_id = router.store_code(contract_minter());
    let creation_fee = coins(CREATION_FEE, NATIVE_DENOM);

    let cw721_addr = String::from("test_cw721_adr");

    // Instantiate minter contract
    let msg = InstantiateMsg {
        unit_price: coin(UNIT_PRICE, NATIVE_DENOM),
        max_num_tokens: 100u32,
        start_time: Timestamp::from_nanos(START_TIME),
        per_address_limit: 5,
        whitelist: None,
        cw721_code_id,
        cw721_address: Some(cw721_addr.clone()),
        cw721_instantiate_msg: None,
    };
    let minter_addr = router
        .instantiate_contract(
            minter_code_id,
            creator.clone(),
            &msg,
            &creation_fee,
            "Minter",
            None,
        )
        .unwrap();

    let config: ConfigResponse = router
        .wrap()
        .query_wasm_smart(minter_addr.clone(), &QueryMsg::Config {})
        .unwrap();
    assert_eq!(config.cw721_address, cw721_addr.clone());

}

#[test]
fn happy_path() {
    let mut router = custom_mock_app();
    setup_block_time(&mut router, START_TIME - 1);
    let (creator, buyer) = setup_accounts(&mut router);
    let num_tokens = 2;
    let (minter_addr, config) = setup_minter_contract(&mut router, &creator, num_tokens);
    upsert_metadata(&mut router, &creator, &minter_addr, num_tokens, None);

    // Default start time genesis mint time
    let res: StartTimeResponse = router
        .wrap()
        .query_wasm_smart(minter_addr.clone(), &QueryMsg::StartTime {})
        .unwrap();

    assert_eq!(
        res.start_time,
        Timestamp::from_nanos(START_TIME).to_string()
    );

    setup_block_time(&mut router, START_TIME + 1);

    // Fail with incorrect tokens
    let mint_msg = ExecuteMsg::Mint {};
    let err = router.execute_contract(
        buyer.clone(),
        minter_addr.clone(),
        &mint_msg,
        &coins(UNIT_PRICE + 100, NATIVE_DENOM),
    );
    assert!(err.is_err());

    // Succeeds if funds are sent
    let mint_msg = ExecuteMsg::Mint {};
    let res = router.execute_contract(
        buyer.clone(),
        minter_addr.clone(),
        &mint_msg,
        &coins(UNIT_PRICE, NATIVE_DENOM),
    );
    assert!(res.is_ok());

    // Balances are correct
    // The creator should get the unit price - mint fee for the mint above
    let creator_balances = router.wrap().query_all_balances(creator.clone()).unwrap();
    assert_eq!(creator_balances, coins(INITIAL_BALANCE, NATIVE_DENOM));
    // The buyer's tokens should reduce by unit price
    let buyer_balances = router.wrap().query_all_balances(buyer.clone()).unwrap();
    assert_eq!(
        buyer_balances,
        coins(INITIAL_BALANCE - UNIT_PRICE, NATIVE_DENOM)
    );

    let res: MintCountResponse = router
        .wrap()
        .query_wasm_smart(
            minter_addr.clone(),
            &QueryMsg::MintCount {
                address: buyer.to_string(),
            },
        )
        .unwrap();
    assert_eq!(res.count, 1);
    assert_eq!(res.address, buyer.to_string());

    // Check NFT is transferred
    let query_owner_msg = Cw721QueryMsg::OwnerOf {
        token_id: String::from("2"),
        include_expired: None,
    };
    let res: OwnerOfResponse = router
        .wrap()
        .query_wasm_smart(config.cw721_address.clone(), &query_owner_msg)
        .unwrap();
    assert_eq!(res.owner, buyer.to_string());

    // Buyer can't call MintTo
    let mint_to_msg = ExecuteMsg::MintTo {
        recipient: buyer.to_string(),
    };
    let res = router.execute_contract(
        buyer.clone(),
        minter_addr.clone(),
        &mint_to_msg,
        &coins_for_msg(Coin {
            amount: Uint128::from(ADMIN_MINT_PRICE),
            denom: NATIVE_DENOM.to_string(),
        }),
    );
    assert!(res.is_err());

    // Creator mints an extra NFT for the buyer (who is a friend)
    let res = router.execute_contract(
        creator.clone(),
        minter_addr.clone(),
        &mint_to_msg,
        &coins_for_msg(Coin {
            amount: Uint128::from(ADMIN_MINT_PRICE),
            denom: NATIVE_DENOM.to_string(),
        }),
    );
    assert!(res.is_ok());

    // Mint count is not increased if admin mints for the user
    let res: MintCountResponse = router
        .wrap()
        .query_wasm_smart(
            minter_addr.clone(),
            &QueryMsg::MintCount {
                address: buyer.to_string(),
            },
        )
        .unwrap();
    assert_eq!(res.count, 1);
    assert_eq!(res.address, buyer.to_string());

    // Minter contract should have a balance
    let minter_balance = router
        .wrap()
        .query_all_balances(minter_addr.clone())
        .unwrap();
    assert_eq!(1, minter_balance.len());
    assert_eq!(minter_balance[0].amount.u128(), UNIT_PRICE);

    // Check that NFT is transferred
    let query_owner_msg = Cw721QueryMsg::OwnerOf {
        token_id: String::from("1"),
        include_expired: None,
    };
    let res: OwnerOfResponse = router
        .wrap()
        .query_wasm_smart(config.cw721_address, &query_owner_msg)
        .unwrap();
    assert_eq!(res.owner, buyer.to_string());

    // Errors if sold out
    let mint_msg = ExecuteMsg::Mint {};
    let res = router.execute_contract(
        buyer,
        minter_addr.clone(),
        &mint_msg,
        &coins_for_msg(Coin {
            amount: Uint128::from(UNIT_PRICE),
            denom: NATIVE_DENOM.to_string(),
        }),
    );
    assert!(res.is_err());

    // Creator can't use MintTo if sold out
    let res = router.execute_contract(
        creator,
        minter_addr,
        &mint_to_msg,
        &coins_for_msg(Coin {
            amount: Uint128::from(ADMIN_MINT_PRICE),
            denom: NATIVE_DENOM.to_string(),
        }),
    );
    assert!(res.is_err());
}

#[test]
fn mint_count_query() {
    let mut router = custom_mock_app();
    let (creator, buyer) = setup_accounts(&mut router);
    let num_tokens = 10;
    let (minter_addr, config) = setup_minter_contract(&mut router, &creator, num_tokens);
    upsert_metadata(&mut router, &creator, &minter_addr, num_tokens, None);

    let cw721_addr = Addr::unchecked(config.cw721_address);
    let whitelist_addr = setup_whitelist_contract(&mut router, &creator);
    const EXPIRATION_TIME: Timestamp = Timestamp::from_nanos(START_TIME + 10_000);

    // Set block to before genesis mint start time
    setup_block_time(&mut router, START_TIME - 1000);

    let wl_msg = WhitelistExecuteMsg::UpdateEndTime(EXPIRATION_TIME);
    let res = router.execute_contract(creator.clone(), whitelist_addr.clone(), &wl_msg, &[]);
    assert!(res.is_ok());

    let wl_msg = WhitelistExecuteMsg::UpdateStartTime(Timestamp::from_nanos(0));
    let res = router.execute_contract(creator.clone(), whitelist_addr.clone(), &wl_msg, &[]);
    assert!(res.is_ok());

    // Set whitelist in minter contract
    let set_whitelist_msg = ExecuteMsg::SetWhitelist {
        whitelist: whitelist_addr.to_string(),
    };
    let res = router.execute_contract(
        creator.clone(),
        minter_addr.clone(),
        &set_whitelist_msg,
        &[],
    );
    assert!(res.is_ok());

    // Update per address_limit
    let set_whitelist_msg = ExecuteMsg::UpdatePerAddressLimit {
        per_address_limit: 3,
    };
    let res = router.execute_contract(
        creator.clone(),
        minter_addr.clone(),
        &set_whitelist_msg,
        &[],
    );
    assert!(res.is_ok());

    // Add buyer to whitelist
    let inner_msg = AddMembersMsg {
        to_add: vec![buyer.to_string()],
    };
    let wasm_msg = WhitelistExecuteMsg::AddMembers(inner_msg);
    let res = router.execute_contract(creator.clone(), whitelist_addr, &wasm_msg, &[]);
    assert!(res.is_ok());

    setup_block_time(&mut router, START_TIME);

    // Mint succeeds
    let mint_msg = ExecuteMsg::Mint {};
    let res = router.execute_contract(
        buyer.clone(),
        minter_addr.clone(),
        &mint_msg,
        &coins(WHITELIST_AMOUNT, NATIVE_DENOM),
    );
    assert!(res.is_ok());

    // Query count
    let res: MintCountResponse = router
        .wrap()
        .query_wasm_smart(
            minter_addr.clone(),
            &QueryMsg::MintCount {
                address: buyer.to_string(),
            },
        )
        .unwrap();
    assert_eq!(res.count, 1);
    assert_eq!(res.address, buyer.to_string());

    // Mint fails, over whitelist per address limit
    let mint_msg = ExecuteMsg::Mint {};
    let err = router
        .execute_contract(
            buyer.clone(),
            minter_addr.clone(),
            &mint_msg,
            &coins(WHITELIST_AMOUNT, NATIVE_DENOM),
        )
        .unwrap_err();
    assert_eq!(
        err.source().unwrap().to_string(),
        ContractError::MaxPerAddressLimitExceeded {}.to_string()
    );

    // Set time after wl ends
    setup_block_time(&mut router, START_TIME + 20_000);

    // Public mint succeeds
    let mint_msg = ExecuteMsg::Mint {};
    let res = router.execute_contract(
        buyer.clone(),
        minter_addr.clone(),
        &mint_msg,
        &coins(UNIT_PRICE, NATIVE_DENOM),
    );
    assert!(res.is_ok());

    // Query count
    let res: MintCountResponse = router
        .wrap()
        .query_wasm_smart(
            minter_addr.clone(),
            &QueryMsg::MintCount {
                address: buyer.to_string(),
            },
        )
        .unwrap();
    assert_eq!(res.count, 2);
    assert_eq!(res.address, buyer.to_string());

    // Buyer transfers NFT to creator
    let transfer_msg: Cw721ExecuteMsg<Empty> = Cw721ExecuteMsg::TransferNft {
        recipient: creator.to_string(),
        token_id: "1".to_string(),
    };
    let res = router.execute_contract(
        buyer.clone(),
        cw721_addr,
        &transfer_msg,
        &coins_for_msg(coin(123, NATIVE_DENOM)),
    );
    assert!(res.is_ok());

    // Mint succeeds
    let mint_msg = ExecuteMsg::Mint {};
    let res = router.execute_contract(
        buyer.clone(),
        minter_addr.clone(),
        &mint_msg,
        &coins(UNIT_PRICE, NATIVE_DENOM),
    );
    assert!(res.is_ok());

    // Query count
    let res: MintCountResponse = router
        .wrap()
        .query_wasm_smart(
            minter_addr.clone(),
            &QueryMsg::MintCount {
                address: buyer.to_string(),
            },
        )
        .unwrap();
    assert_eq!(res.count, 3);
    assert_eq!(res.address, buyer.to_string());

    // Mint fails
    let mint_msg = ExecuteMsg::Mint {};
    let err = router
        .execute_contract(
            buyer.clone(),
            minter_addr.clone(),
            &mint_msg,
            &coins(WHITELIST_AMOUNT, NATIVE_DENOM),
        )
        .unwrap_err();
    assert_eq!(
        err.source().unwrap().to_string(),
        ContractError::MaxPerAddressLimitExceeded {}.to_string()
    );

    // Query count
    let res: MintCountResponse = router
        .wrap()
        .query_wasm_smart(
            minter_addr,
            &QueryMsg::MintCount {
                address: buyer.to_string(),
            },
        )
        .unwrap();
    assert_eq!(res.count, 3);
    assert_eq!(res.address, buyer.to_string());
}

#[test]
fn whitelist_already_started() {
    let mut router = custom_mock_app();
    let (creator, _) = setup_accounts(&mut router);
    let num_tokens = 1;
    let (minter_addr, _) = setup_minter_contract(&mut router, &creator, num_tokens);
    upsert_metadata(&mut router, &creator, &minter_addr, num_tokens, None);
    let whitelist_addr = setup_whitelist_contract(&mut router, &creator);

    setup_block_time(&mut router, START_TIME + 101);

    // set whitelist in minter contract
    let set_whitelist_msg = ExecuteMsg::SetWhitelist {
        whitelist: whitelist_addr.to_string(),
    };
    router
        .execute_contract(
            creator.clone(),
            minter_addr,
            &set_whitelist_msg,
            &coins(UNIT_PRICE, NATIVE_DENOM),
        )
        .unwrap_err();
}

#[test]
fn whitelist_can_update_before_start() {
    let mut router = custom_mock_app();
    let (creator, _) = setup_accounts(&mut router);
    let num_tokens = 1;
    let (minter_addr, _) = setup_minter_contract(&mut router, &creator, num_tokens);
    upsert_metadata(&mut router, &creator, &minter_addr, num_tokens, None);
    let whitelist_addr = setup_whitelist_contract(&mut router, &creator);

    setup_block_time(&mut router, START_TIME - 1000);

    // set whitelist in minter contract
    let set_whitelist_msg = ExecuteMsg::SetWhitelist {
        whitelist: whitelist_addr.to_string(),
    };
    router
        .execute_contract(
            creator.clone(),
            minter_addr.clone(),
            &set_whitelist_msg,
            &coins(UNIT_PRICE, NATIVE_DENOM),
        )
        .unwrap();

    // can set twice before starting
    router
        .execute_contract(
            creator.clone(),
            minter_addr,
            &set_whitelist_msg,
            &coins(UNIT_PRICE, NATIVE_DENOM),
        )
        .unwrap();
}

#[test]
fn whitelist_access_len_add_remove_expiration() {
    let mut router = custom_mock_app();
    let (creator, buyer) = setup_accounts(&mut router);
    let num_tokens = 1;
    let (minter_addr, config) = setup_minter_contract(&mut router, &creator, num_tokens);
    upsert_metadata(&mut router, &creator, &minter_addr, num_tokens, None);
    
    let cw721_addr = config.cw721_address;
    let whitelist_addr = setup_whitelist_contract(&mut router, &creator);
    const AFTER_GENESIS_TIME: Timestamp = Timestamp::from_nanos(START_TIME + 100);

    // Set to just before genesis mint start time
    setup_block_time(&mut router, START_TIME - 10);

    // Update whitelist_expiration fails if not admin
    let wl_msg = WhitelistExecuteMsg::UpdateEndTime(AFTER_GENESIS_TIME);
    router
        .execute_contract(buyer.clone(), whitelist_addr.clone(), &wl_msg, &[])
        .unwrap_err();

    // Update whitelist_expiration succeeds when from admin
    let wl_msg = WhitelistExecuteMsg::UpdateEndTime(AFTER_GENESIS_TIME);
    let res = router.execute_contract(creator.clone(), whitelist_addr.clone(), &wl_msg, &[]);
    assert!(res.is_ok());

    let wl_msg = WhitelistExecuteMsg::UpdateStartTime(Timestamp::from_nanos(0));
    let res = router.execute_contract(creator.clone(), whitelist_addr.clone(), &wl_msg, &[]);
    assert!(res.is_ok());

    // Set whitelist in minter contract
    let set_whitelist_msg = ExecuteMsg::SetWhitelist {
        whitelist: whitelist_addr.to_string(),
    };
    let res = router.execute_contract(
        creator.clone(),
        minter_addr.clone(),
        &set_whitelist_msg,
        &[],
    );
    assert!(res.is_ok());

    // Mint fails, buyer is not on whitelist
    let mint_msg = ExecuteMsg::Mint {};
    let res = router.execute_contract(
        buyer.clone(),
        minter_addr.clone(),
        &mint_msg,
        &coins(UNIT_PRICE, NATIVE_DENOM),
    );
    assert!(res.is_err());

    // Add buyer to whitelist
    let inner_msg = AddMembersMsg {
        to_add: vec![buyer.to_string()],
    };
    let wasm_msg = WhitelistExecuteMsg::AddMembers(inner_msg);
    let res = router.execute_contract(creator.clone(), whitelist_addr.clone(), &wasm_msg, &[]);
    assert!(res.is_ok());

    // Mint fails, not whitelist price
    let mint_msg = ExecuteMsg::Mint {};
    router
        .execute_contract(
            buyer.clone(),
            minter_addr.clone(),
            &mint_msg,
            &coins(UNIT_PRICE, NATIVE_DENOM),
        )
        .unwrap_err();

    setup_block_time(&mut router, START_TIME);

    // Query mint price
    let mint_price_response: MintPriceResponse = router
        .wrap()
        .query_wasm_smart(minter_addr.clone(), &QueryMsg::MintPrice {})
        .unwrap();
    assert_eq!(
        coin(WHITELIST_AMOUNT, NATIVE_DENOM),
        mint_price_response.whitelist_price.unwrap()
    );
    assert_eq!(
        coin(WHITELIST_AMOUNT, NATIVE_DENOM),
        mint_price_response.current_price
    );
    assert_eq!(
        coin(UNIT_PRICE, NATIVE_DENOM),
        mint_price_response.public_price
    );

    // Mint succeeds with whitelist price
    let mint_msg = ExecuteMsg::Mint {};
    let res = router.execute_contract(
        buyer.clone(),
        minter_addr.clone(),
        &mint_msg,
        &coins(WHITELIST_AMOUNT, NATIVE_DENOM),
    );
    assert!(res.is_ok());

    // Mint fails, over whitelist per address limit
    let mint_msg = ExecuteMsg::Mint {};
    let err = router
        .execute_contract(
            buyer.clone(),
            minter_addr.clone(),
            &mint_msg,
            &coins(WHITELIST_AMOUNT, NATIVE_DENOM),
        )
        .unwrap_err();
    assert_eq!(
        err.source().unwrap().to_string(),
        ContractError::MaxPerAddressLimitExceeded {}.to_string()
    );

    // Buyer is generous and transfers to creator
    let transfer_msg: Cw721ExecuteMsg<Empty> = Cw721ExecuteMsg::TransferNft {
        recipient: creator.to_string(),
        token_id: "1".to_string(),
    };
    let res = router.execute_contract(
        buyer.clone(),
        Addr::unchecked(cw721_addr),
        &transfer_msg,
        &coins_for_msg(coin(123, NATIVE_DENOM)),
    );
    assert!(res.is_ok());

    // Mint fails, buyer exceeded per address limit
    let mint_msg = ExecuteMsg::Mint {};
    let err = router
        .execute_contract(
            buyer.clone(),
            minter_addr.clone(),
            &mint_msg,
            &coins(WHITELIST_AMOUNT, NATIVE_DENOM),
        )
        .unwrap_err();
    assert_eq!(
        err.source().unwrap().to_string(),
        ContractError::MaxPerAddressLimitExceeded {}.to_string()
    );

    // Remove buyer from whitelist
    let inner_msg = AddMembersMsg { to_add: vec![] };
    let wasm_msg = WhitelistExecuteMsg::AddMembers(inner_msg);
    let res = router.execute_contract(creator.clone(), whitelist_addr, &wasm_msg, &[]);
    assert!(res.is_ok());

    // Mint fails
    let mint_msg = ExecuteMsg::Mint {};
    let res = router.execute_contract(
        buyer,
        minter_addr,
        &mint_msg,
        &coins(UNIT_PRICE, NATIVE_DENOM),
    );
    assert!(res.is_err());
}

#[test]
fn before_start_time() {
    let mut router = custom_mock_app();
    let (creator, buyer) = setup_accounts(&mut router);
    let num_tokens = 1;
    let (minter_addr, _) = setup_minter_contract(&mut router, &creator, num_tokens);
    upsert_metadata(&mut router, &creator, &minter_addr, num_tokens, None);

    // Set to before genesis mint start time
    setup_block_time(&mut router, START_TIME - 10);

    // Set start_time fails if not admin
    let start_time_msg = ExecuteMsg::UpdateStartTime(Timestamp::from_nanos(0));
    let res = router.execute_contract(
        buyer.clone(),
        minter_addr.clone(),
        &start_time_msg,
        &coins(UNIT_PRICE, NATIVE_DENOM),
    );
    assert!(res.is_err());

    // Buyer can't mint before start_time
    let mint_msg = ExecuteMsg::Mint {};
    let res = router.execute_contract(
        buyer.clone(),
        minter_addr.clone(),
        &mint_msg,
        &coins(UNIT_PRICE, NATIVE_DENOM),
    );
    assert!(res.is_err());

    // Query start_time, confirm expired
    let start_time_response: StartTimeResponse = router
        .wrap()
        .query_wasm_smart(minter_addr.clone(), &QueryMsg::StartTime {})
        .unwrap();
    assert_eq!(
        Timestamp::from_nanos(START_TIME).to_string(),
        start_time_response.start_time
    );

    // Set block forward, after start time. mint succeeds
    setup_block_time(&mut router, START_TIME + 10_000_000);

    // Mint succeeds
    let mint_msg = ExecuteMsg::Mint {};
    let res = router.execute_contract(
        buyer,
        minter_addr,
        &mint_msg,
        &coins(UNIT_PRICE, NATIVE_DENOM),
    );
    assert!(res.is_ok());
}

#[test]
fn check_per_address_limit() {
    let mut router = custom_mock_app();
    let (creator, buyer) = setup_accounts(&mut router);
    let num_tokens = 2;
    let (minter_addr, _config) = setup_minter_contract(&mut router, &creator, num_tokens);
    upsert_metadata(&mut router, &creator, &minter_addr, num_tokens, None);

    // Set to genesis mint start time
    setup_block_time(&mut router, START_TIME);

    // Set limit, check unauthorized
    let per_address_limit_msg = ExecuteMsg::UpdatePerAddressLimit {
        per_address_limit: 30,
    };
    let res = router.execute_contract(
        buyer.clone(),
        minter_addr.clone(),
        &per_address_limit_msg,
        &coins(UNIT_PRICE, NATIVE_DENOM),
    );
    assert!(res.is_err());

    // Set limit errors, invalid limit == 0
    let per_address_limit_msg = ExecuteMsg::UpdatePerAddressLimit {
        per_address_limit: 0,
    };
    let res = router.execute_contract(
        creator.clone(),
        minter_addr.clone(),
        &per_address_limit_msg,
        &coins(UNIT_PRICE, NATIVE_DENOM),
    );
    assert!(res.is_err());

    // Set limit succeeds, mint fails, over max
    let per_address_limit_msg = ExecuteMsg::UpdatePerAddressLimit {
        per_address_limit: 1,
    };
    let res = router.execute_contract(
        creator.clone(),
        minter_addr.clone(),
        &per_address_limit_msg,
        &coins(UNIT_PRICE, NATIVE_DENOM),
    );
    assert!(res.is_ok());

    // First mint succeeds
    let mint_msg = ExecuteMsg::Mint {};
    let res = router.execute_contract(
        buyer.clone(),
        minter_addr.clone(),
        &mint_msg,
        &coins(UNIT_PRICE, NATIVE_DENOM),
    );

    assert!(res.is_ok());

    // Second mint fails from exceeding per address limit
    let mint_msg = ExecuteMsg::Mint {};
    let res = router.execute_contract(
        buyer,
        minter_addr,
        &mint_msg,
        &coins(UNIT_PRICE, NATIVE_DENOM),
    );
    assert!(res.is_err());
}

#[test]
fn mint_for_token_id_addr() {
    let mut router = custom_mock_app();
    let (creator, buyer) = setup_accounts(&mut router);
    let num_tokens = 4;
    let (minter_addr, _config) = setup_minter_contract(&mut router, &creator, num_tokens);
    upsert_metadata(&mut router, &creator, &minter_addr, num_tokens, None);

    // Set to genesis mint start time
    setup_block_time(&mut router, START_TIME);

    // Try mint_for, test unauthorized
    let mint_for_msg = ExecuteMsg::MintFor {
        token_id: 1,
        recipient: buyer.to_string(),
    };
    let err = router
        .execute_contract(
            buyer.clone(),
            minter_addr.clone(),
            &mint_for_msg,
            &coins_for_msg(Coin {
                amount: Uint128::from(ADMIN_MINT_PRICE),
                denom: NATIVE_DENOM.to_string(),
            }),
        )
        .unwrap_err();
    assert_eq!(
        err.source().unwrap().to_string(),
        ContractError::Unauthorized("Sender is not an admin".to_string()).to_string(),
    );

    // Test token id already sold
    // 1. mint token_id 1
    // 2. mint_for token_id 1
    let mint_msg = ExecuteMsg::Mint {};
    let res = router.execute_contract(
        buyer.clone(),
        minter_addr.clone(),
        &mint_msg,
        &coins(UNIT_PRICE, NATIVE_DENOM),
    );
    assert!(res.is_ok());

    // Minter contract should have a balance
    let minter_balance = router
        .wrap()
        .query_all_balances(minter_addr.clone())
        .unwrap();
    assert_eq!(minter_balance[0].amount.u128(), UNIT_PRICE);
    assert_eq!(1, minter_balance.len());

    // Mint fails, invalid token_id
    let token_id = 0;
    let mint_for_msg = ExecuteMsg::MintFor {
        token_id,
        recipient: buyer.to_string(),
    };
    let err = router
        .execute_contract(
            creator.clone(),
            minter_addr.clone(),
            &mint_for_msg,
            &coins_for_msg(Coin {
                amount: Uint128::from(ADMIN_MINT_PRICE),
                denom: NATIVE_DENOM.to_string(),
            }),
        )
        .unwrap_err();
    assert_eq!(
        ContractError::InvalidTokenId {}.to_string(),
        err.source().unwrap().to_string()
    );

    // Mint fails, token_id already sold
    let token_id = 1;
    let mint_for_msg = ExecuteMsg::MintFor {
        token_id,
        recipient: buyer.to_string(),
    };
    let err = router
        .execute_contract(
            creator.clone(),
            minter_addr.clone(),
            &mint_for_msg,
            &coins_for_msg(Coin {
                amount: Uint128::from(ADMIN_MINT_PRICE),
                denom: NATIVE_DENOM.to_string(),
            }),
        )
        .unwrap_err();
    assert_eq!(
        ContractError::TokenAlreadyMinted { token_id }.to_string(),
        err.source().unwrap().to_string()
    );

    let num_minted_response: NumMintedResponse = router
        .wrap()
        .query_wasm_smart(minter_addr.clone(), &QueryMsg::NumMinted {})
        .unwrap();
    assert_eq!(num_minted_response, NumMintedResponse { num_minted: 1 });

    let num_remaining_response: NumRemainingResponse = router
        .wrap()
        .query_wasm_smart(minter_addr.clone(), &QueryMsg::NumRemaining {})
        .unwrap();
    assert_eq!(num_remaining_response, NumRemainingResponse { num_remaining: 3 });

    // Test mint_for token_id 2 then normal mint
    let token_id = 2;
    let mint_for_msg = ExecuteMsg::MintFor {
        token_id,
        recipient: buyer.to_string(),
    };
    let res = router.execute_contract(
        creator.clone(),
        minter_addr.clone(),
        &mint_for_msg,
        &coins_for_msg(Coin {
            amount: Uint128::from(ADMIN_MINT_PRICE),
            denom: NATIVE_DENOM.to_string(),
        }),
    );
    assert!(res.is_ok());

    let num_minted_response: NumMintedResponse = router
        .wrap()
        .query_wasm_smart(minter_addr.clone(), &QueryMsg::NumMinted {})
        .unwrap();
    assert_eq!(num_minted_response, NumMintedResponse { num_minted: 2 });

    let num_remaining_response: NumRemainingResponse = router
        .wrap()
        .query_wasm_smart(minter_addr.clone(), &QueryMsg::NumRemaining {})
        .unwrap();
    assert_eq!(num_remaining_response, NumRemainingResponse { num_remaining: 2 });
}

#[test]
fn test_start_time_before_genesis() {
    let mut router = custom_mock_app();
    let (creator, _) = setup_accounts(&mut router);
    let num_tokens = 10;

    // Upload contract code
    let cw721_code_id = router.store_code(contract_cw721());
    let minter_code_id = router.store_code(contract_minter());
    let creation_fee = coins(CREATION_FEE, NATIVE_DENOM);

    // Instantiate sale contract
    let msg = InstantiateMsg {
        unit_price: coin(UNIT_PRICE, NATIVE_DENOM),
        max_num_tokens: num_tokens,
        start_time: Timestamp::from_nanos(START_TIME),
        per_address_limit: 5,
        whitelist: None,
        cw721_code_id,
        cw721_address: None,
        cw721_instantiate_msg: Some(Pg721InstantiateMsg {
            name: String::from("TEST"),
            symbol: String::from("TEST"),
            minter: creator.to_string(),
            collection_info: CollectionInfo {
                creator: creator.to_string(),
                description: String::from("Passage Monkeys"),
                image: "https://example.com/image.png".to_string(),
                external_link: Some("https://example.com/external.html".to_string()),
                royalty_info: Some(RoyaltyInfoResponse {
                    payment_address: creator.to_string(),
                    share: Decimal::percent(10),
                }),
            },
        }),
    };
    let minter_addr = router
        .instantiate_contract(minter_code_id, creator, &msg, &creation_fee, "Minter", None)
        .unwrap();

    let res: StartTimeResponse = router
        .wrap()
        .query_wasm_smart(minter_addr, &QueryMsg::StartTime {})
        .unwrap();
    assert_eq!(
        res.start_time,
        Timestamp::from_nanos(START_TIME).to_string()
    );
}

#[test]
fn test_update_start_time() {
    let mut router = custom_mock_app();
    let (creator, _) = setup_accounts(&mut router);
    let num_tokens = 10;

    // Upload contract code
    let cw721_code_id = router.store_code(contract_cw721());
    let minter_code_id = router.store_code(contract_minter());
    let creation_fee = coins(CREATION_FEE, NATIVE_DENOM);

    // Instantiate sale contract
    let msg = InstantiateMsg {
        unit_price: coin(UNIT_PRICE, NATIVE_DENOM),
        max_num_tokens: num_tokens,
        start_time: Timestamp::from_nanos(START_TIME),
        per_address_limit: 5,
        whitelist: None,
        cw721_code_id,
        cw721_address: None,
        cw721_instantiate_msg: Some(Pg721InstantiateMsg {
            name: String::from("TEST"),
            symbol: String::from("TEST"),
            minter: creator.to_string(),
            collection_info: CollectionInfo {
                creator: creator.to_string(),
                description: String::from("Passage Monkeys"),
                image: "https://example.com/image.png".to_string(),
                external_link: Some("https://example.com/external.html".to_string()),
                royalty_info: None,
            },
        }),
    };
    let minter_addr = router
        .instantiate_contract(
            minter_code_id,
            creator.clone(),
            &msg,
            &creation_fee,
            "Minter",
            None,
        )
        .unwrap();

    // Public mint has started
    setup_block_time(&mut router, START_TIME + 100);

    // Update to a start time in the past
    let msg = ExecuteMsg::UpdateStartTime(Timestamp::from_nanos(START_TIME - 1000));
    let err = router
        .execute_contract(creator, minter_addr, &msg, &[])
        .unwrap_err();
    assert_eq!(
        err.source().unwrap().to_string(),
        ContractError::AlreadyStarted {}.to_string(),
    );
}


#[test]
fn test_update_unit_price() {
    let mut router = custom_mock_app();
    setup_block_time(&mut router, START_TIME - 1);
    let (creator, _buyer) = setup_accounts(&mut router);
    let num_tokens = 2;
    let (minter_addr, _config) = setup_minter_contract(&mut router, &creator, num_tokens);

    let new_unit_price = coin(UNIT_PRICE + 100u128, NATIVE_DENOM);
    let msg = ExecuteMsg::UpdateUnitPrice { unit_price: new_unit_price.clone() };
    let _res = router
        .execute_contract(creator, minter_addr.clone(), &msg, &[])
        .unwrap();

    let config: ConfigResponse = router
        .wrap()
        .query_wasm_smart(minter_addr.clone(), &QueryMsg::Config {})
        .unwrap();
    assert_eq!(config.unit_price, new_unit_price);
}

#[test]
fn test_invalid_start_time() {
    let mut router = custom_mock_app();
    let (creator, _) = setup_accounts(&mut router);
    let num_tokens = 10;

    // Upload contract code
    let cw721_code_id = router.store_code(contract_cw721());
    let minter_code_id = router.store_code(contract_minter());
    let creation_fee = coins(CREATION_FEE, NATIVE_DENOM);

    // Instantiate sale contract before genesis mint
    let mut msg = InstantiateMsg {
        unit_price: coin(UNIT_PRICE, NATIVE_DENOM),
        max_num_tokens: num_tokens,
        start_time: Timestamp::from_nanos(START_TIME - 100),
        per_address_limit: 5,
        whitelist: None,
        cw721_code_id,
        cw721_address: None,
        cw721_instantiate_msg: Some(Pg721InstantiateMsg {
            name: String::from("TEST"),
            symbol: String::from("TEST"),
            minter: creator.to_string(),
            collection_info: CollectionInfo {
                creator: creator.to_string(),
                description: String::from("Passage Monkeys"),
                image: "https://example.com/image.png".to_string(),
                external_link: Some("https://example.com/external.html".to_string()),
                royalty_info: None,
            },
        }),
    };

    // move date after genesis mint
    setup_block_time(&mut router, START_TIME + 1000);

    // move start time after genesis but before current time
    msg.start_time = Timestamp::from_nanos(START_TIME + 500);

    router
        .instantiate_contract(
            minter_code_id,
            creator.clone(),
            &msg,
            &creation_fee,
            "Minter",
            None,
        )
        .unwrap_err();

    // position block time before the start time
    setup_block_time(&mut router, START_TIME + 400);

    let minter_addr = router
        .instantiate_contract(
            minter_code_id,
            creator.clone(),
            &msg,
            &creation_fee,
            "Minter",
            None,
        )
        .unwrap();

    // Update to a start time in the past
    let msg = ExecuteMsg::UpdateStartTime(Timestamp::from_nanos(START_TIME - 100));
    let res = router.execute_contract(creator.clone(), minter_addr.clone(), &msg, &[]);
    assert!(res.is_err());

    // Update to a time after genesis but before the current block_time (GENESIS+400)
    let msg = ExecuteMsg::UpdateStartTime(Timestamp::from_nanos(START_TIME + 300));
    let res = router.execute_contract(creator.clone(), minter_addr.clone(), &msg, &[]);
    assert!(res.is_err());

    // Update to a time after genesis and after current blocktime (GENESIS+400)
    let msg = ExecuteMsg::UpdateStartTime(Timestamp::from_nanos(START_TIME + 450));
    let res = router.execute_contract(creator.clone(), minter_addr.clone(), &msg, &[]);
    assert!(res.is_ok());

    // position block after start time (GENESIS+450);
    setup_block_time(&mut router, START_TIME + 500);

    // Update to a time after genesis and after current blocktime (GENESIS+400)
    let msg = ExecuteMsg::UpdateStartTime(Timestamp::from_nanos(START_TIME + 450));
    let err = router
        .execute_contract(creator, minter_addr, &msg, &[])
        .unwrap_err();
    assert_eq!(err.source().unwrap().to_string(), "AlreadyStarted");
}

#[test]
fn unhappy_path() {
    let mut router = custom_mock_app();
    let (creator, buyer) = setup_accounts(&mut router);
    let num_tokens = 1;
    let (minter_addr, _config) = setup_minter_contract(&mut router, &creator, num_tokens);
    upsert_metadata(&mut router, &creator, &minter_addr, num_tokens, None);

    // Fails if too little funds are sent
    let mint_msg = ExecuteMsg::Mint {};
    let res = router.execute_contract(
        buyer.clone(),
        minter_addr.clone(),
        &mint_msg,
        &coins(1, NATIVE_DENOM),
    );
    assert!(res.is_err());

    // Fails if too many funds are sent
    let mint_msg = ExecuteMsg::Mint {};
    let res = router.execute_contract(
        buyer.clone(),
        minter_addr.clone(),
        &mint_msg,
        &coins(11111, NATIVE_DENOM),
    );
    assert!(res.is_err());

    // Fails wrong denom is sent
    let mint_msg = ExecuteMsg::Mint {};
    let res = router.execute_contract(buyer, minter_addr, &mint_msg, &coins(UNIT_PRICE, "uatom"));
    assert!(res.is_err());
}

#[test]
fn can_withdraw() {
    // create minter
    let mut router = custom_mock_app();
    let (creator, buyer) = setup_accounts(&mut router);
    let num_tokens = 4;
    let (minter_addr, _config) = setup_minter_contract(&mut router, &creator, num_tokens);
    upsert_metadata(&mut router, &creator, &minter_addr, num_tokens, None);

    setup_block_time(&mut router, START_TIME + 1);

    // someone who isn't the creator cannot withdraw
    let withdraw_msg = ExecuteMsg::Withdraw { recipient: creator.to_string() };
    router
        .execute_contract(buyer.clone(), minter_addr.clone(), &withdraw_msg, &[])
        .unwrap_err();

    // withdraw with a zero balance
    router
        .execute_contract(creator.clone(), minter_addr.clone(), &withdraw_msg, &[])
        .unwrap_err();

    // do a mint
    let mint_msg = ExecuteMsg::Mint {};
    let res = router.execute_contract(
        buyer,
        minter_addr.clone(),
        &mint_msg,
        &coins(UNIT_PRICE, NATIVE_DENOM),
    );
    assert!(res.is_ok());

    // Minter contract should have a balance
    let minter_balance = router
        .wrap()
        .query_all_balances(minter_addr.clone())
        .unwrap();
    assert_eq!(minter_balance[0].amount.u128(), UNIT_PRICE);

    // withdraw
    let res = router.execute_contract(creator.clone(), minter_addr.clone(), &withdraw_msg, &[]);
    assert!(res.is_ok());

    // Minter contract should have no balance
    let minter_balance = router.wrap().query_all_balances(minter_addr).unwrap();
    assert_eq!(0, minter_balance.len());

    // creator should have received their payment
    let creator_balances = router.wrap().query_all_balances(creator).unwrap();
    assert_eq!(
        creator_balances,
        coins(INITIAL_BALANCE + UNIT_PRICE, NATIVE_DENOM)
    );
}

#[test]
fn metadata_test() {
    let mut router = custom_mock_app();
    setup_block_time(&mut router, START_TIME - 1);
    let (creator, buyer) = setup_accounts(&mut router);
    let num_tokens = 4;
    let (minter_addr, config) = setup_minter_contract(&mut router, &creator, num_tokens);

    // Default start time genesis mint time
    let res: StartTimeResponse = router
        .wrap()
        .query_wasm_smart(minter_addr.clone(), &QueryMsg::StartTime {})
        .unwrap();

    assert_eq!(
        res.start_time,
        Timestamp::from_nanos(START_TIME).to_string()
    );

    setup_block_time(&mut router, START_TIME + 1);
    
    // Fails with missing metadata
    upsert_metadata(&mut router, &creator, &minter_addr, 2, None);
    let mint_msg = ExecuteMsg::Mint {};
    let res = router.execute_contract(
        buyer.clone(),
        minter_addr.clone(),
        &mint_msg,
        &coins(UNIT_PRICE, NATIVE_DENOM),
    );
    assert!(res.is_ok());

    // Succeeds with enough metadata
    upsert_metadata(&mut router, &creator, &minter_addr, 2, Some(3));
    let mint_msg = ExecuteMsg::Mint {};
    let res = router.execute_contract(
        buyer.clone(),
        minter_addr.clone(),
        &mint_msg,
        &coins(UNIT_PRICE, NATIVE_DENOM),
    );
    assert!(res.is_ok());
    assert_eq!(res.unwrap().events[1].attributes[4], Attribute {
        key: String::from("token_id"),
        value: String::from("3")
    });

    // Check NFT is transferred
    let query_info = Cw721QueryMsg::NftInfo {
        token_id: String::from("3"),
    };
    let res: NftInfoResponse<Metadata> = router
        .wrap()
        .query_wasm_smart(config.cw721_address.clone(), &query_info)
        .unwrap();
    assert_eq!(res.extension.image, Some(String::from("image-3.png")));

    // Check minter TokenMints
    let query_info = QueryMsg::TokenMints {
        descending: Some(true),
        filter_minted: Some(true),
        start_after: Some(2u32),
        limit: Some(10),
    };
    let res: TokenMintsResponse = router
        .wrap()
        .query_wasm_smart(minter_addr, &query_info)
        .unwrap();
    assert_eq!(res.token_mints.len(), 1);
    assert_eq!(res.token_mints[0].token_id, 4);
    assert_eq!(res.token_mints[0].is_minted, false);
}

#[test]
fn update_admin() {
    let mut router = custom_mock_app();
    setup_block_time(&mut router, START_TIME - 1);
    let (creator, buyer) = setup_accounts(&mut router);
    let num_tokens = 2;
    let (minter_addr, _) = setup_minter_contract(&mut router, &creator, num_tokens);

    let config: ConfigResponse = router
    .wrap()
    .query_wasm_smart(minter_addr.clone(), &QueryMsg::Config {})
    .unwrap();
    assert_eq!(config.admin, creator.to_string());

    // Update the admin address
    let set_admin = ExecuteMsg::SetAdmin { admin: buyer.to_string() };
    let res = router.execute_contract(
        creator.clone(),
        minter_addr.clone(),
        &set_admin,
        &[],
    );
    assert!(res.is_ok());

    let config: ConfigResponse = router
        .wrap()
        .query_wasm_smart(minter_addr.clone(), &QueryMsg::Config {})
        .unwrap();
    assert_eq!(config.admin, buyer.to_string());
}