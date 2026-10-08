//! Synthetic fixtures only: no production ownership/genesis/signing data.
use cosmwasm_std::{testing::{mock_dependencies, mock_env, mock_info}, Addr, Binary, Order, Storage, coin};
use pg721_metadata_onchain::{baseline, contract, msg::{ExecuteMsg, InstantiateMsg, MigrateMsg, Metadata, QueryMsg}, retirement_targets, state::{CollectionInfo, MigrationStatus, MIGRATION_STATUS}};
use cw721_base::MintMsg;

fn mint(id: &str) -> MintMsg<Option<Metadata>> {
    MintMsg { token_id: id.into(), owner: "holder".into(), token_uri: Some("https://example.invalid/token".into()), extension: Some(Metadata::default()) }
}
fn instantiate() -> InstantiateMsg {
    InstantiateMsg { name: "Synthetic collection".into(), symbol: "TEST".into(), minter: "minter".into(), collection_info: CollectionInfo { creator: "creator".into(), description: "synthetic".into(), image: "https://example.invalid/image".into(), external_link: None, royalty_info: None } }
}
fn migration(count: u64, done: bool) -> MigrateMsg {
    MigrateMsg { expected_version: env!("CARGO_PKG_VERSION").into(), expected_count: count, expected_migration_done: done, minter: "minter".into() }
}
fn bytes(storage: &dyn Storage) -> Vec<(Vec<u8>,Vec<u8>)> { storage.range(None,None,Order::Ascending).collect() }

#[test]
fn every_mint_and_import_path_rejects_without_mutation_for_any_caller_or_gate() {
    for done in [false, true] {
        let mut deps=mock_dependencies(); let env=mock_env();
        baseline::instantiate(deps.as_mut(),env.clone(),mock_info("creator",&[]),instantiate()).unwrap();
        MIGRATION_STATUS.save(&mut deps.storage,&MigrationStatus {done}).unwrap();
        let before=bytes(&deps.storage);
        for sender in ["minter","admin","buyer"] {
            for msg in [ExecuteMsg::Mint(mint("new")),ExecuteMsg::Migrate {migrations: vec![mint("a"),mint("b")]},ExecuteMsg::Migrate {migrations: vec![]}] {
                assert!(contract::execute(deps.as_mut(),env.clone(),mock_info(sender,&[coin(777,"upasg")]),msg).unwrap_err().to_string().contains("NFT minting retired"));
                assert_eq!(before,bytes(&deps.storage));
            }
        }
    }
}

#[test]
fn migration_is_storage_neutral_and_checks_every_precondition() {
    let mut deps=mock_dependencies(); let mut env=mock_env();
    baseline::instantiate(deps.as_mut(),env.clone(),mock_info("creator",&[]),instantiate()).unwrap();
    let before=bytes(&deps.storage);
    for target in retirement_targets::TARGETS {
        env.contract.address=Addr::unchecked(*target);
        contract::migrate(deps.as_mut(),env.clone(),migration(0,false)).unwrap();
        assert_eq!(before,bytes(&deps.storage));
    }
    for bad in [MigrateMsg {expected_count:1,..migration(0,false)},MigrateMsg {minter:"different".into(),..migration(0,false)},MigrateMsg {expected_migration_done:true,..migration(0,false)},MigrateMsg {expected_version:"99.0.0".into(),..migration(0,false)}] {
        assert!(contract::migrate(deps.as_mut(),env.clone(),bad).is_err()); assert_eq!(before,bytes(&deps.storage));
    }
    env.contract.address=Addr::unchecked("pasg1kwdranvwf6vl2grh99layugwdnph6um2x0c0l8qyvrcgsjcykuhs6vaz9p");
    assert!(contract::migrate(deps.as_mut(),env.clone(),migration(0,false)).unwrap_err().to_string().contains("not a reviewed"));
    env.contract.address=Addr::unchecked(retirement_targets::TARGETS[0]);
    cw2::set_contract_version(&mut deps.storage,"unrelated-family",env!("CARGO_PKG_VERSION")).unwrap();
    assert!(contract::migrate(deps.as_mut(),env,migration(0,false)).is_err());
}

#[test]
fn unfinished_import_gate_and_completion_authority_are_unchanged() {
    let mut deps=mock_dependencies(); let mut env=mock_env(); env.contract.address=Addr::unchecked(retirement_targets::TARGETS[0]);
    baseline::instantiate(deps.as_mut(),env.clone(),mock_info("creator",&[]),instantiate()).unwrap();
    baseline::execute(deps.as_mut(),env.clone(),mock_info("minter",&[]),ExecuteMsg::Migrate {migrations:vec![mint("existing")]}).unwrap();
    contract::migrate(deps.as_mut(),env.clone(),migration(1,false)).unwrap();
    for msg in [ExecuteMsg::TransferNft {recipient:"recipient".into(),token_id:"existing".into()},ExecuteMsg::SendNft {contract:"receiver".into(),token_id:"existing".into(),msg:Binary::default()},ExecuteMsg::Approve {spender:"spender".into(),token_id:"existing".into(),expires:None},ExecuteMsg::Burn {token_id:"existing".into()}] {
        let before=bytes(&deps.storage);
        assert_eq!(baseline::execute(deps.as_mut(),env.clone(),mock_info("holder",&[]),msg.clone()).unwrap_err().to_string(),contract::execute(deps.as_mut(),env.clone(),mock_info("holder",&[]),msg).unwrap_err().to_string());
        assert_eq!(before,bytes(&deps.storage));
    }
    assert!(contract::execute(deps.as_mut(),env.clone(),mock_info("stranger",&[]),ExecuteMsg::MigrationDone{}).is_err());
    assert!(!MIGRATION_STATUS.load(&deps.storage).unwrap().done);
    // Historical minter-authorized completion remains possible, but migration never unlocks it.
    contract::execute(deps.as_mut(),env,mock_info("minter",&[]),ExecuteMsg::MigrationDone{}).unwrap();
    assert!(MIGRATION_STATUS.load(&deps.storage).unwrap().done);
}

#[test]
fn holders_keep_queries_approvals_transfers_send_and_authorized_burn() {
    let mut deps=mock_dependencies(); let mut env=mock_env(); env.contract.address=Addr::unchecked(retirement_targets::TARGETS[0]);
    baseline::instantiate(deps.as_mut(),env.clone(),mock_info("creator",&[]),instantiate()).unwrap();
    baseline::execute(deps.as_mut(),env.clone(),mock_info("minter",&[]),ExecuteMsg::Migrate {migrations:vec![mint("existing")]}).unwrap();
    baseline::execute(deps.as_mut(),env.clone(),mock_info("minter",&[]),ExecuteMsg::MigrationDone{}).unwrap();
    let before=bytes(&deps.storage); contract::migrate(deps.as_mut(),env.clone(),migration(1,true)).unwrap(); assert_eq!(before,bytes(&deps.storage));
    for query in [QueryMsg::ContractInfo{},QueryMsg::CollectionInfo{},QueryMsg::OwnerOf {token_id:"existing".into(),include_expired:Some(true)},QueryMsg::NftInfo {token_id:"existing".into()}] {
        assert_eq!(baseline::query(deps.as_ref(),env.clone(),query.clone()).unwrap(),contract::query(deps.as_ref(),env.clone(),query).unwrap());
    }
    for msg in [ExecuteMsg::Approve {spender:"spender".into(),token_id:"existing".into(),expires:None},ExecuteMsg::Revoke {spender:"spender".into(),token_id:"existing".into()},ExecuteMsg::ApproveAll {operator:"operator".into(),expires:None},ExecuteMsg::RevokeAll {operator:"operator".into()}] {
        contract::execute(deps.as_mut(),env.clone(),mock_info("holder",&[]),msg).unwrap();
    }
    assert!(contract::execute(deps.as_mut(),env.clone(),mock_info("stranger",&[]),ExecuteMsg::Burn {token_id:"existing".into()}).is_err());
    contract::execute(deps.as_mut(),env.clone(),mock_info("holder",&[]),ExecuteMsg::TransferNft {recipient:"recipient".into(),token_id:"existing".into()}).unwrap();
    assert_eq!(contract::execute(deps.as_mut(),env.clone(),mock_info("recipient",&[]),ExecuteMsg::SendNft {contract:"receiver".into(),token_id:"existing".into(),msg:Binary::default()}).unwrap().messages.len(),1);
    contract::execute(deps.as_mut(),env,mock_info("receiver",&[]),ExecuteMsg::Burn {token_id:"existing".into()}).unwrap();
}
