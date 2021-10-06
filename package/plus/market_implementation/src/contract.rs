use std::fmt;

use crate::auction::{
    handle_ask_auction, query_auction, try_bid_nft, try_cancel_bid, try_claim_winner,
    try_emergency_cancel_auction,
};

use crate::offering::{
    handle_sell_nft, query_ai_royalty, query_offering, try_buy, try_handle_mint, try_withdraw,
};

use crate::error::ContractError;
use crate::msg::{
    AskNftMsg, HandleMsg, InitMsg, ProxyHandleMsg, ProxyQueryMsg, QueryMsg, SellNft,
    UpdateContractMsg,
};
use crate::state::{ContractInfo, CONTRACT_INFO};
use cosmwasm_std::HumanAddr;
use cosmwasm_std::{
    attr, from_binary, to_binary, BankMsg, Binary, Coin, CosmosMsg, Deps, DepsMut, Empty, Env,
    HandleResponse, InitResponse, MessageInfo, StdResult, WasmMsg,
};
use cw721::{Cw721HandleMsg, Cw721ReceiveMsg};
use market::{StorageHandleMsg, StorageQueryMsg};
use market_ai_royalty::sanitize_royalty;
use schemars::JsonSchema;
use serde::Serialize;

pub const MAX_ROYALTY_PERCENT: u64 = 50;
pub const MAX_FEE_PERMILLE: u64 = 100;
pub const CREATOR_NAME: &str = "creator";

fn sanitize_fee(fee: u64, limit: u64, name: &str) -> Result<u64, ContractError> {
    if fee > limit {
        return Err(ContractError::InvalidArgument {
            arg: name.to_string(),
        });
    }
    Ok(fee)
}

// Note, you can use StdResult in some functions where you do not
// make use of the custom errors
pub fn init(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InitMsg,
) -> Result<InitResponse, ContractError> {
    let info = ContractInfo {
        name: msg.name,
        creator: info.sender.to_string(),
        denom: msg.denom,
        fee: sanitize_fee(msg.fee, MAX_FEE_PERMILLE, "fee")?,
        auction_duration: msg.auction_duration,
        step_price: msg.step_price,
        governance: msg.governance,
        max_royalty: sanitize_royalty(msg.max_royalty, MAX_ROYALTY_PERCENT, "max_royalty")?,
    };
    CONTRACT_INFO.save(deps.storage, &info)?;
    Ok(InitResponse::default())
}

// And declare a custom Error variant for the ones where you will want to make use of it
pub fn handle(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: HandleMsg,
) -> Result<HandleResponse, ContractError> {
    match msg {
        // auction
        HandleMsg::BidNft { auction_id } => try_bid_nft(deps, info, env, auction_id),
        HandleMsg::ClaimWinner { auction_id } => try_claim_winner(deps, info, env, auction_id),
        // HandleMsg::WithdrawNft { auction_id } => try_withdraw_nft(deps, info, env, auction_id),
        HandleMsg::EmergencyCancelAuction { auction_id } => {
            try_emergency_cancel_auction(deps, info, env, auction_id)
        }
        HandleMsg::ReceiveNft(msg) => try_receive_nft(deps, info, env, msg),
        HandleMsg::CancelBid { auction_id } => try_cancel_bid(deps, info, env, auction_id),
        HandleMsg::WithdrawFunds { funds } => try_withdraw_funds(deps, info, env, funds),
        HandleMsg::UpdateInfo(msg) => try_update_info(deps, info, env, msg),
        // royalty
        HandleMsg::MintNft(msg) => try_handle_mint(deps, info, msg),
        HandleMsg::WithdrawNft { offering_id } => try_withdraw(deps, info, offering_id),
        HandleMsg::BuyNft { offering_id } => try_buy(deps, info, env, offering_id),
        HandleMsg::MigrateVersion {
            nft_contract_addr,
            token_ids,
            new_marketplace,
        } => try_migrate(
            deps,
            info,
            env,
            token_ids,
            nft_contract_addr,
            new_marketplace,
        ),
        // HandleMsg::UpdateRoyalties { royalty } => try_update_royalties(deps, info, env, royalty),
        // HandleMsg::UpdateOfferingRoyalties { royalty } => {
        //     try_update_offering_royalties(deps, info, env, royalty)
        // }
    }
}

// ============================== Query Handlers ==============================

pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetContractInfo {} => to_binary(&query_contract_info(deps)?),
        QueryMsg::Auction(auction_msg) => query_auction(deps, auction_msg),
        QueryMsg::Offering(offering_msg) => query_offering(deps, offering_msg),
        QueryMsg::AiRoyalty(ai_royalty_msg) => query_ai_royalty(deps, ai_royalty_msg),
    }
}

// ============================== Message Handlers ==============================

pub fn try_withdraw_funds(
    deps: DepsMut,
    _info: MessageInfo,
    env: Env,
    fund: Coin,
) -> Result<HandleResponse, ContractError> {
    let contract_info = CONTRACT_INFO.load(deps.storage)?;
    let bank_msg: CosmosMsg = BankMsg::Send {
        from_address: env.contract.address,
        to_address: HumanAddr::from(contract_info.creator.clone()), // as long as we send to the contract info creator => anyone can help us withdraw the fees
        amount: vec![fund.clone()],
    }
    .into();

    Ok(HandleResponse {
        messages: vec![bank_msg],
        attributes: vec![
            attr("action", "withdraw_funds"),
            attr("denom", fund.denom),
            attr("amount", fund.amount),
            attr("receiver", contract_info.creator),
        ],
        data: None,
    })
}

pub fn try_update_info(
    deps: DepsMut,
    info: MessageInfo,
    _env: Env,
    msg: UpdateContractMsg,
) -> Result<HandleResponse, ContractError> {
    let new_contract_info = CONTRACT_INFO.update(deps.storage, |mut contract_info| {
        // Unauthorized
        if !info.sender.to_string().eq(&contract_info.creator) {
            return Err(ContractError::Unauthorized {
                sender: info.sender.to_string(),
            });
        }
        if let Some(name) = msg.name {
            contract_info.name = name;
        }
        if let Some(creator) = msg.creator {
            contract_info.creator = creator;
        }
        if let Some(fee) = msg.fee {
            contract_info.fee = sanitize_fee(fee, MAX_FEE_PERMILLE, "fee")?;
        }
        if let Some(auction_duration) = msg.auction_duration {
            contract_info.auction_duration = auction_duration
        }
        if let Some(step_price) = msg.step_price {
            contract_info.step_price = step_price
        }
        if let Some(governance) = msg.governance {
            contract_info.governance = governance;
        }
        Ok(contract_info)
    })?;

    Ok(HandleResponse {
        messages: vec![],
        attributes: vec![attr("action", "update_info")],
        data: to_binary(&new_contract_info).ok(),
    })
}

// when user sell NFT to
pub fn try_receive_nft(
    deps: DepsMut,
    info: MessageInfo,
    env: Env,
    rcv_msg: Cw721ReceiveMsg,
) -> Result<HandleResponse, ContractError> {
    if let Some(msg) = rcv_msg.msg.clone() {
        if let Ok(ask_msg) = from_binary::<AskNftMsg>(&msg) {
            return handle_ask_auction(deps, info, env, ask_msg, rcv_msg);
        }
        if let Ok(sell_msg) = from_binary::<SellNft>(&msg) {
            return handle_sell_nft(deps, info, sell_msg, rcv_msg);
        }
    }
    Err(ContractError::NoData {})
}

pub fn try_migrate(
    deps: DepsMut,
    info: MessageInfo,
    _env: Env,
    token_ids: Vec<String>,
    nft_contract_addr: HumanAddr,
    new_marketplace: HumanAddr,
) -> Result<HandleResponse, ContractError> {
    let ContractInfo { creator, .. } = CONTRACT_INFO.load(deps.storage)?;
    if info.sender.ne(&HumanAddr(creator.clone())) {
        return Err(ContractError::Unauthorized {
            sender: info.sender.to_string(),
        });
    }
    let mut cw721_transfer_cosmos_msg: Vec<CosmosMsg> = vec![];
    for token_id in token_ids.clone() {
        // check if token_id is currently sold by the requesting address
        // transfer token back to original owner
        let transfer_cw721_msg = Cw721HandleMsg::TransferNft {
            recipient: new_marketplace.clone(),
            token_id,
        };

        let exec_cw721_transfer = WasmMsg::Execute {
            contract_addr: nft_contract_addr.clone(),
            msg: to_binary(&transfer_cw721_msg)?,
            send: vec![],
        }
        .into();
        cw721_transfer_cosmos_msg.push(exec_cw721_transfer);
    }
    Ok(HandleResponse {
        messages: cw721_transfer_cosmos_msg,
        attributes: vec![
            attr("action", "migrate_marketplace"),
            attr("nft_contract_addr", nft_contract_addr),
            attr("token_ids", format!("{:?}", token_ids)),
            attr("new_marketplace", new_marketplace),
        ],
        data: None,
    })
}

pub fn query_contract_info(deps: Deps) -> StdResult<ContractInfo> {
    CONTRACT_INFO.load(deps.storage)
}

// remove recursive by query storage_addr first, then call query_proxy
pub fn get_storage_addr(deps: Deps, contract: HumanAddr, name: &str) -> StdResult<HumanAddr> {
    deps.querier.query_wasm_smart(
        contract,
        &ProxyQueryMsg::Storage(StorageQueryMsg::QueryStorageAddr {
            name: name.to_string(),
        }),
    )
}

pub fn get_handle_msg<T>(addr: &str, name: &str, msg: T) -> StdResult<CosmosMsg>
where
    T: Clone + fmt::Debug + PartialEq + JsonSchema + Serialize,
{
    let offering_msg = to_binary(&ProxyHandleMsg::Msg(msg))?;
    let proxy_msg: ProxyHandleMsg<Empty> =
        ProxyHandleMsg::Storage(StorageHandleMsg::UpdateStorageData {
            name: name.to_string(),
            msg: offering_msg,
        });

    Ok(WasmMsg::Execute {
        contract_addr: HumanAddr::from(addr),
        msg: to_binary(&proxy_msg)?,
        send: vec![],
    }
    .into())
}
