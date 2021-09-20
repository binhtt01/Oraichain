use crate::offering::{
    handle_sell_nft, query_offering, sanitize_royalty, try_buy, try_handle_mint,
    try_update_royalty, try_withdraw, MAX_FEE_PERMILLE,
};

use crate::error::ContractError;
use crate::msg::{HandleMsg, InitMsg, ProxyQueryMsg, QueryMsg, SellNft, UpdateContractMsg};
use crate::state::{ContractInfo, CONTRACT_INFO};
use cosmwasm_std::{
    attr, from_binary, to_binary, BankMsg, Binary, Coin, CosmosMsg, Deps, DepsMut, Env,
    HandleResponse, InitResponse, MessageInfo, StdResult,
};
use cosmwasm_std::{HumanAddr, StdError};
use cw1155::Cw1155ReceiveMsg;
use market::StorageQueryMsg;

fn sanitize_fee(fee: u64, name: &str) -> Result<u64, ContractError> {
    if fee > MAX_FEE_PERMILLE {
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
        fee: sanitize_fee(msg.fee, "fee")?,
        governance: msg.governance,
        max_royalty: sanitize_royalty(msg.max_royalty, "max_royalty")?,
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
        HandleMsg::Receive(msg) => try_receive_nft(deps, info, env, msg),
        HandleMsg::WithdrawFunds { funds } => try_withdraw_funds(deps, info, env, funds),
        HandleMsg::UpdateInfo(msg) => try_update_info(deps, info, env, msg),
        // royalty
        HandleMsg::MintNft { contract, msg } => try_handle_mint(deps, info, contract, msg),
        HandleMsg::WithdrawNft { offering_id } => try_withdraw(deps, info, env, offering_id),
        HandleMsg::BuyNft { offering_id } => try_buy(deps, info, env, offering_id),
        HandleMsg::UpdateRoyalty(payout_msg) => try_update_royalty(deps, info, env, payout_msg),
    }
}

// ============================== Query Handlers ==============================

pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetContractInfo {} => to_binary(&query_contract_info(deps)?),
        QueryMsg::Offering(offering_msg) => query_offering(deps, offering_msg),
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
            return Err(ContractError::Unauthorized {});
        }
        if let Some(name) = msg.name {
            contract_info.name = name;
        }
        if let Some(creator) = msg.creator {
            contract_info.creator = creator;
        }
        if let Some(fee) = msg.fee {
            contract_info.fee = sanitize_fee(fee, "fee")?;
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
    _env: Env,
    rcv_msg: Cw1155ReceiveMsg,
) -> Result<HandleResponse, ContractError> {
    let msg_result_sell: Result<SellNft, StdError> = from_binary(&rcv_msg.msg);
    if !msg_result_sell.is_err() {
        return handle_sell_nft(deps, info, msg_result_sell.unwrap(), rcv_msg);
    }
    Err(ContractError::NoData {})
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