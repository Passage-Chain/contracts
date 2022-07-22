#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    coin, Addr, Decimal, DepsMut, Env, Event, MessageInfo, StdError,
    Uint128, Response,
};
use cw2::set_contract_version;
use cw_utils::{maybe_addr, must_pay, nonpayable};

use crate::error::ContractError;
use crate::helpers::{
    map_validate, ExpiryRange, finalize_sale, price_validate, store_bid,
    store_collection_bid, only_owner_or_seller, only_owner, only_seller, only_operator,
    transfer_nft, transfer_token, match_bid
};
use crate::msg::{InstantiateMsg, ExecuteMsg, QueryOptions};
use crate::query::query_bids_token_price;
use crate::state::{
    Params, PARAMS, Ask, asks, TokenId, bid_key, bids, Expiration, Recipient,
    Bid, CollectionBid, collection_bids, Auction, auctions
};

// Version info for migration info
const CONTRACT_NAME: &str = "crates.io:marketplace-v2";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    msg.ask_expiry.validate()?;
    msg.bid_expiry.validate()?;

    let api = deps.api;
    let params = Params {
        cw721_address: api.addr_validate(&msg.cw721_address)?,
        denom: msg.denom,
        collector_address: api.addr_validate(&msg.collector_address)?,
        trading_fee_percent: Decimal::percent(msg.trading_fee_bps),
        ask_expiry: msg.ask_expiry,
        bid_expiry: msg.bid_expiry,
        auction_expiry: msg.auction_expiry,
        operators: map_validate(deps.api, &msg.operators)?,
        min_price: msg.min_price,
    };
    PARAMS.save(deps.storage, &params)?;

    Ok(Response::new())
}


#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    let api = deps.api;
    let message_info = info.clone();

    match msg {
        ExecuteMsg::UpdateParams {
            trading_fee_bps,
            ask_expiry,
            bid_expiry,
            auction_expiry,
            operators,
            min_price,
        } => execute_update_params(
            deps,
            env,
            info,
            trading_fee_bps,
            ask_expiry,
            bid_expiry,
            auction_expiry,
            operators,
            min_price,
        ),
        ExecuteMsg::SetAsk {
            token_id,
            price,
            funds_recipient,
            reserve_for,
            expires_at,
        } => execute_set_ask(
            deps,
            env,
            info,
            Ask {
                token_id,
                seller: message_info.sender,
                price,
                funds_recipient: maybe_addr(api, funds_recipient)?,
                reserve_for: maybe_addr(api, reserve_for)?,
                expires_at,
            },
        ),
        ExecuteMsg::RemoveAsk {
            token_id,
        } => execute_remove_ask(deps, info, token_id),
        ExecuteMsg::SetBid {
            token_id,
            price,
            expires_at,
        } => execute_set_bid(
            deps,
            env,
            info,
            Bid {
                token_id,
                bidder: message_info.sender,
                price,
                expires_at,
            },
        ),
        ExecuteMsg::RemoveBid {
            token_id,
        } => execute_remove_bid(deps, env, info, token_id),
        ExecuteMsg::AcceptBid {
            token_id,
            bidder,
        } => execute_accept_bid(
            deps,
            env,
            info,
            token_id,
            api.addr_validate(&bidder)?,
        ),
        ExecuteMsg::SetCollectionBid {
            units,
            price,
            expires_at,
        } => execute_set_collection_bid(
            deps,
            env,
            info,
            CollectionBid {
                units,
                price,
                bidder: message_info.sender,
                expires_at
            }
        ),
        ExecuteMsg::RemoveCollectionBid { } => {
            execute_remove_collection_bid(deps, env, info)
        }
        ExecuteMsg::AcceptCollectionBid {
            token_id,
            bidder,
        } => execute_accept_collection_bid(
            deps,
            env,
            info,
            token_id,
            api.addr_validate(&bidder)?,
        ),
        ExecuteMsg::SetAuction {
            token_id,
            starting_price,
            reserve_price,
            funds_recipient,
            expires_at,
        } => execute_set_auction(
            deps,
            env,
            info,
            Auction {
                token_id,
                seller: message_info.sender,
                starting_price,
                reserve_price,
                funds_recipient: maybe_addr(api, funds_recipient)?,
                expires_at,
            },
        ),
        ExecuteMsg::CloseAuction {
            token_id,
            accept_highest_bid,
        } => execute_close_auction(
            deps,
            env,
            info,
            token_id,
            accept_highest_bid,
        ),
    }
}

/// An operator may update the marketplace params
pub fn execute_update_params(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    trading_fee_bps: Option<u64>,
    ask_expiry: Option<ExpiryRange>,
    bid_expiry: Option<ExpiryRange>,
    auction_expiry: Option<ExpiryRange>,
    operators: Option<Vec<String>>,
    min_price: Option<Uint128>,
) -> Result<Response, ContractError> {
    let mut params = PARAMS.load(deps.storage)?;
    only_operator(&info, &params)?;

    if let Some(_trading_fee_bps) = trading_fee_bps {
        params.trading_fee_percent = Decimal::percent(_trading_fee_bps);
    }
    if let Some(_ask_expiry) = ask_expiry {
        _ask_expiry.validate()?;
        params.ask_expiry = _ask_expiry;
    }
    if let Some(_bid_expiry) = bid_expiry {
        _bid_expiry.validate()?;
        params.bid_expiry = _bid_expiry;
    }
    if let Some(_auction_expiry) = auction_expiry {
        _auction_expiry.validate()?;
        params.auction_expiry = _auction_expiry;
    }
    if let Some(_operators) = operators {
        params.operators = map_validate(deps.api, &_operators)?;
    }
    if let Some(_min_price) = min_price {
        params.min_price = _min_price;
    }
    
    PARAMS.save(deps.storage, &params)?;
    Ok(Response::new())
}

/// A seller may set an Ask on their NFT to list it on Marketplace
pub fn execute_set_ask(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    ask: Ask,
) -> Result<Response, ContractError> {
    nonpayable(&info)?;
    
    let params = PARAMS.load(deps.storage)?;
    params.ask_expiry.is_valid(&env.block, ask.expires_at)?;
    price_validate(&ask.price, &params)?;

    let existing_ask = asks().load(deps.storage, ask.token_id.clone()).ok();
    only_owner_or_seller(
        deps.as_ref(),
        &info,
        &params.cw721_address,
        &ask.token_id,
        &existing_ask.clone().map_or(None, |a| Some(a.seller)),
    )?;

    // Upsert ask
    asks().update(
        deps.storage,
        ask.token_id.clone(),
        |_| -> Result<Ask, StdError> { Ok(ask.clone()) },
    )?;

    let mut response = Response::new();

    match existing_ask {
        None => transfer_nft(&ask.token_id, &env.contract.address, &params.cw721_address, &mut response)?,
        _ => (),
    }

    let event = Event::new("set-ask")
        .add_attribute("collection", params.cw721_address.to_string())
        .add_attribute("token_id", ask.token_id.to_string())
        .add_attribute("seller", ask.seller)
        .add_attribute("price", ask.price.to_string())
        .add_attribute("expires_at", ask.expires_at.to_string());

    Ok(response.add_event(event))
}

/// Removes the ask on a particular NFT
pub fn execute_remove_ask(
    deps: DepsMut,
    info: MessageInfo,
    token_id: TokenId,
) -> Result<Response, ContractError> {
    nonpayable(&info)?;

    let ask = asks().load(deps.storage, token_id.clone())?;
    only_seller(&info, &ask.seller)?;

    asks().remove(deps.storage, token_id.clone())?;

    let params = PARAMS.load(deps.storage)?;
    let mut response = Response::new();

    transfer_nft(&ask.token_id, &ask.seller, &params.cw721_address, &mut response)?;

    let event = Event::new("remove-ask")
        .add_attribute("collection", params.cw721_address.to_string())
        .add_attribute("token_id", token_id.to_string());

    Ok(response.add_event(event))
}

/// Places a bid on a listed or unlisted NFT. The bid is escrowed in the contract.
pub fn execute_set_bid(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    bid: Bid,
) -> Result<Response, ContractError> {
    let params = PARAMS.load(deps.storage)?;

    let payment_amount = must_pay(&info, &params.denom)?;
    if bid.price.amount != payment_amount  {
        return Err(ContractError::IncorrectBidPayment(bid.price.amount, payment_amount));
    }
    price_validate(&bid.price, &params)?;
    params.bid_expiry.is_valid(&env.block, bid.expires_at)?;

    let mut response = Response::new();
    let bid_key = bid_key(bid.token_id.clone(), &bid.bidder);
    let ask_key = &bid.token_id;

    // If bid exists, refund the escrowed tokens
    if let Some(existing_bid) = bids().may_load(deps.storage, bid_key.clone())? {
        bids().remove(deps.storage, bid_key)?;
        transfer_token(
            existing_bid.price,
            existing_bid.bidder.to_string(),
            "refund-bidder",
            &mut response,
        )?;
    }

    let matching_ask = match_bid(deps.as_ref(), env, &bid, &mut response)?;

    // If existing ask found, finalize the sale
    match matching_ask {
        Some(ask) => {
            asks().remove(deps.storage, ask_key.clone())?;
            finalize_sale(
                deps.as_ref(),
                &bid.bidder,
                &ask.token_id,
                payment_amount,
                &ask.get_recipient(),
                &params,
                &mut response,
            )?
        },
        None => store_bid(deps.storage, &bid)?,
    };

    let event = Event::new("set-bid")
        .add_attribute("token_id", bid.token_id.to_string())
        .add_attribute("bidder", bid.bidder)
        .add_attribute("price", bid.price.to_string())
        .add_attribute("expires_at", bid.expires_at.to_string());
    response.events.push(event);

    Ok(response)
}

/// Removes a bid made by the bidder. Bidders can only remove their own bids
pub fn execute_remove_bid(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    token_id: TokenId,
) -> Result<Response, ContractError> {
    nonpayable(&info)?;
    let bidder = info.sender;

    let key = bid_key(token_id.clone(), &bidder);
    let bid = bids().load(deps.storage, key.clone())?;
    bids().remove(deps.storage, key)?;

    let mut response = Response::new();
    transfer_token(bid.price, bid.bidder.to_string(), "refund-bidder", &mut response)?;

    let event = Event::new("remove-bid")
        .add_attribute("token_id", token_id.clone())
        .add_attribute("bidder", bidder);
    response.events.push(event);

    Ok(response)
}

/// Seller can accept a bid which transfers funds as well as the token. The bid may or may not be associated with an ask.
pub fn execute_accept_bid(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    token_id: TokenId,
    bidder: Addr,
) -> Result<Response, ContractError> {
    nonpayable(&info)?;

    let bid_key = bid_key(token_id.clone(), &bidder);
    let bid = bids().load(deps.storage, bid_key.clone())?;
    if bid.is_expired(&env.block.time) {
        return Err(ContractError::BidExpired {});
    }

    let params = PARAMS.load(deps.storage)?;
    let existing_ask = asks().may_load(deps.storage, token_id.clone())?;

    only_owner_or_seller(
        deps.as_ref(),
        &info,
        &params.cw721_address,
        &token_id,
        &existing_ask.clone().map_or(None, |a| Some(a.seller)),
    )?;

    // Remove ask if it exists, define recipient
    let payment_recipient = match existing_ask {
        Some(ask) => {
            asks().remove(deps.storage, ask.token_id.clone())?;
            ask.get_recipient()
        },
        None => info.sender,
    };

    let mut response = Response::new();

    // Transfer funds and NFT
    finalize_sale(
        deps.as_ref(),
        &bid.bidder,
        &token_id,
        bid.price.amount,
        &payment_recipient,
        &params,
        &mut response,
    )?;

    // Remove accepted bid
    bids().remove(deps.storage, bid_key)?;

    let event = Event::new("accept-bid")
        .add_attribute("token_id", token_id.to_string())
        .add_attribute("bidder", bidder)
        .add_attribute("price", bid.price.to_string())
        .add_attribute("expires_at", bid.expires_at.to_string());
    response.events.push(event);

    Ok(response)
}

/// Place a collection bid (limit order) across an entire collection
pub fn execute_set_collection_bid(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    collection_bid: CollectionBid
) -> Result<Response, ContractError> {
    let params = PARAMS.load(deps.storage)?;
    
    // Escrows the amount (price * units)
    let payment_amount = must_pay(&info, &params.denom)?;
    price_validate(&coin(collection_bid.total_cost(), &params.denom), &params)?;
    if Uint128::from(collection_bid.total_cost()) != payment_amount  {
        return Err(ContractError::IncorrectBidPayment(
            Uint128::from(collection_bid.total_cost()),
            payment_amount,
        ));
    }
    params.bid_expiry.is_valid(&env.block, collection_bid.expires_at)?;

    let collection_bid_key = collection_bid.bidder.clone();
    let mut response = Response::new();

    // If collection bid exists, refund the escrowed tokens
    if let Some(existing_bid) = collection_bids().may_load(deps.storage, collection_bid_key.clone())? {
        collection_bids().remove(deps.storage, collection_bid_key.clone())?;
        transfer_token(
            existing_bid.price,
            existing_bid.bidder.to_string(),
            "refund-collection-bidder",
            &mut response,
        )?;
    }
    collection_bids().save(deps.storage, collection_bid_key, &collection_bid)?;

    let event = Event::new("set-collection-bid")
        .add_attribute("bidder", collection_bid.bidder)
        .add_attribute("price", collection_bid.price.to_string())
        .add_attribute("units", collection_bid.units.to_string())
        .add_attribute("expires_at", collection_bid.expires_at.to_string());
    response.events.push(event);

    Ok(response)
}

/// Remove an existing collection bid (limit order)
pub fn execute_remove_collection_bid(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    nonpayable(&info)?;
    let mut response = Response::new();
    
    let collection_bid_key = info.sender.clone();

    let collection_bid = collection_bids().load(deps.storage, collection_bid_key.clone())?;

    collection_bids().remove(deps.storage, collection_bid_key)?;
    transfer_token(
        collection_bid.price,
        collection_bid.bidder.to_string(),
        "refund-collection-bidder",
        &mut response,
    )?;

    let event = Event::new("remove-collection-bid")
        .add_attribute("bidder", collection_bid.bidder);
    response.events.push(event);

    Ok(response)
}

/// Owner/seller of an item in a collection can accept a collection bid which transfers funds as well as a token
pub fn execute_accept_collection_bid(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    token_id: TokenId,
    bidder: Addr,
) -> Result<Response, ContractError> {
    nonpayable(&info)?;

    let collection_bid_key = bidder.clone();
    let mut collection_bid = collection_bids().load(deps.storage, collection_bid_key.clone())?;
    if collection_bid.is_expired(&env.block.time) {
        return Err(ContractError::BidExpired {});
    }

    let params = PARAMS.load(deps.storage)?;
    let existing_ask = asks().may_load(deps.storage, token_id.clone())?;
    only_owner_or_seller(
        deps.as_ref(),
        &info,
        &params.cw721_address,
        &token_id,
        &existing_ask.clone().map_or(None, |a| Some(a.seller)),
    )?;

    // Remove ask if it exists, define recipient
    let payment_recipient = match existing_ask {
        Some(ask) => {
            asks().remove(deps.storage, ask.token_id.clone())?;
            ask.get_recipient()
        },
        None => info.sender,
    };

    match collection_bid.units {
        1 => {
            // Remove accepted bid
            collection_bids().remove(deps.storage, collection_bid_key)?;
        },
        _ => {
            // Remove accepted bid
            collection_bid.units -= 1;
            store_collection_bid(deps.storage, &collection_bid)?;
        }
    }

    let mut response = Response::new();

    // Transfer funds and NFT
    finalize_sale(
        deps.as_ref(),
        &collection_bid.bidder,
        &token_id,
        collection_bid.price.amount,
        &payment_recipient,
        &params,
        &mut response,
    )?;

    let event = Event::new("accept-collection-bid")
        .add_attribute("bidder", collection_bid.bidder)
        .add_attribute("price", collection_bid.price.to_string())
        .add_attribute("units", collection_bid.units.to_string())
        .add_attribute("expires_at", collection_bid.expires_at.to_string());
    response.events.push(event);

    Ok(response)
}

/// Owner of an NFT can create auction to begin accepting bids
pub fn execute_set_auction(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    auction: Auction,
) -> Result<Response, ContractError> {
    nonpayable(&info)?;
    
    let params = PARAMS.load(deps.storage)?;
    params.auction_expiry.is_valid(&env.block, auction.expires_at)?;

    only_owner(deps.as_ref(), &info, &params.cw721_address.clone(), &auction.token_id)?;
    
    price_validate(&auction.starting_price, &params)?;
    if let Some(_reserve_price) = &auction.reserve_price {
        price_validate(&_reserve_price, &params)?;
        if _reserve_price.amount < auction.starting_price.amount {
            return Err(ContractError::InvalidReservePrice(_reserve_price.amount, auction.starting_price.amount));
        }
    }

    let existing_auction = auctions().may_load(deps.storage, auction.token_id.clone())?;
    if let Some(_existing_auction) = existing_auction {
        return Err(ContractError::AuctionAlreadyExists(auction.token_id.clone()));
    }

    auctions().save(deps.storage, auction.token_id.clone(), &auction)?;

    let mut response = Response::new();

    transfer_nft(&auction.token_id, &env.contract.address, &params.cw721_address, &mut response)?;

    let event = Event::new("set-auction")
        .add_attribute("collection", params.cw721_address.to_string())
        .add_attribute("token_id", auction.token_id.to_string())
        .add_attribute("seller", auction.seller)
        .add_attribute("price", auction.starting_price.to_string())
        .add_attribute("expires_at", auction.expires_at.to_string());

    Ok(response.add_event(event))
}

/// Creator of an auction can close it and transfer the NFT to the buyer
pub fn execute_close_auction(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    token_id: TokenId,
    accept_highest_bid: bool,
) -> Result<Response, ContractError> {
    nonpayable(&info)?;

    let params = PARAMS.load(deps.storage)?;

    // Validate auction exists, and if it exists, that it is being closed by the seller
    let auction = auctions().load(deps.storage, token_id.clone())?;
    only_seller(&info, &auction.seller)?;
    
    if auction.is_expired(&env.block.time) {
        return Err(ContractError::AuctionExpired {});
    }

    // Fetch the highest matching bid that has not expired
    let bids_response = query_bids_token_price(
        deps.as_ref(),
        token_id.clone(),
        &QueryOptions {
            descending: Some(true),
            filter_expiry: Some(env.block.time),
            start_after: None,
            limit: Some(1),
        }
    )?;
    let highest_bid = bids_response.bids.first();

    // Check if reserve price has been met
    let mut reserve_price_met = false;
    if let Some(bid) = highest_bid {
        if let Some(reserve_price) = &auction.reserve_price {
            reserve_price_met = bid.price.amount >= reserve_price.amount;
        }
    };

    // If reserve price has been met, seller cannot close auction
    if reserve_price_met {
        return Err(ContractError::ReservePriceRestriction(
            String::from("must accept highest bid when reserve price is met")
        ));
    }

    let mut response = Response::new();
    
    let is_sale = highest_bid.is_some() && accept_highest_bid;
    match is_sale {
        true => {
            // If sale has occurred, finalize
            let bid = highest_bid.unwrap();
            finalize_sale(
                deps.as_ref(),
                &bid.bidder,
                &auction.token_id,
                bid.price.amount,
                &auction.get_recipient(),
                &params,
                &mut response,
            )?;
        },
        false => {
            // If sale has not occurred, transfer NFT back to seller, do not transfer funds to seller
            transfer_nft(&auction.token_id, &info.sender, &params.cw721_address, &mut response)?;
        }
    };

    let event = Event::new("close-auction")
        .add_attribute("collection", params.cw721_address.to_string())
        .add_attribute("token_id", auction.token_id.to_string())
        .add_attribute("is_sale", is_sale.to_string());
    
    Ok(response.add_event(event))
}