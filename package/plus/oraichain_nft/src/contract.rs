use cosmwasm_std::{
    attr, coins, to_binary, Api, BankMsg, Binary, BlockInfo, Deps, DepsMut, Env, HandleResponse,
    HumanAddr, InitResponse, MessageInfo, Order, StdError, StdResult, KV,
};

use cw0::maybe_canonical;
use cw2::set_contract_version;
use cw721::{
    AllNftInfoResponse, ApprovedForAllResponse, ContractInfoResponse, Cw721ReceiveMsg, Expiration,
    NftInfoResponse, NumTokensResponse, OwnerOfResponse, TokensResponse,
};

use crate::check_size;
use crate::error::ContractError;
use crate::msg::{HandleMsg, InitMsg, MintMsg, MinterResponse, QueryMsg};
use crate::state::{
    increment_tokens, num_tokens, tokens, Approval, TokenInfo, CONTRACT_INFO, MINTER, OPERATORS,
    OWNER,
};
use cw_storage_plus::Bound;

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:oraichain_nft";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");
const DENOM: &str = "orai";
const DEFAULT_LIMIT: u32 = 10;
const MAX_LIMIT: u32 = 30;
const MAX_CHARS_SIZE: usize = 1024;

pub fn init(
    deps: DepsMut,
    _env: Env,
    msg_info: MessageInfo,
    msg: InitMsg,
) -> StdResult<InitResponse> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let info = ContractInfoResponse {
        name: msg.name,
        symbol: msg.symbol,
    };
    CONTRACT_INFO.save(deps.storage, &info)?;
    let minter = deps.api.canonical_address(&msg.minter)?;
    let owner = deps.api.canonical_address(&msg_info.sender)?;
    MINTER.save(deps.storage, &minter)?;
    OWNER.save(deps.storage, &owner)?;
    Ok(InitResponse::default())
}

pub fn handle(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: HandleMsg,
) -> Result<HandleResponse, ContractError> {
    match msg {
        HandleMsg::Mint(msg) => handle_mint(deps, env, info, msg),
        HandleMsg::Approve {
            spender,
            token_id,
            expires,
        } => handle_approve(deps, env, info, spender, token_id, expires),
        HandleMsg::Revoke { spender, token_id } => {
            handle_revoke(deps, env, info, spender, token_id)
        }
        HandleMsg::ApproveAll { operator, expires } => {
            handle_approve_all(deps, env, info, operator, expires)
        }
        HandleMsg::RevokeAll { operator } => handle_revoke_all(deps, env, info, operator),
        HandleMsg::TransferNft {
            recipient,
            token_id,
        } => handle_transfer_nft(deps, env, info, recipient, token_id),
        HandleMsg::SendNft {
            contract,
            token_id,
            msg,
        } => handle_send_nft(deps, env, info, contract, token_id, msg),
        HandleMsg::UpdateNft {
            token_id,
            name,
            description,
            image,
        } => handle_update_nft(deps, env, info, token_id, name, description, image),
        HandleMsg::ChangeMinter { minter } => handle_change_minter(deps, env, info, minter),
        HandleMsg::WithdrawFees { address, fees } => {
            handle_withdraw_fees(deps, env, info, address, fees)
        }
    }
}

pub fn handle_mint(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: MintMsg,
) -> Result<HandleResponse, ContractError> {
    let minter = MINTER.load(deps.storage)?;
    let sender_raw = deps.api.canonical_address(&info.sender)?;

    if sender_raw != minter {
        return Err(ContractError::Unauthorized {});
    }

    let name = msg.name;
    check_size!(name, MAX_CHARS_SIZE);
    let description = msg.description.unwrap_or_default();
    check_size!(description, MAX_CHARS_SIZE);
    let image = msg.image;
    check_size!(image, MAX_CHARS_SIZE);

    // create the token
    let token = TokenInfo {
        owner: deps.api.canonical_address(&msg.owner)?,
        approvals: vec![],
        name,
        description,
        image,
    };
    tokens().update(deps.storage, &msg.token_id, |old| match old {
        Some(_) => Err(ContractError::Claimed {}),
        None => Ok(token),
    })?;

    increment_tokens(deps.storage)?;

    Ok(HandleResponse {
        messages: vec![],
        attributes: vec![
            attr("action", "mint"),
            attr("minter", info.sender),
            attr("token_id", msg.token_id),
        ],
        data: None,
    })
}

pub fn handle_transfer_nft(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    recipient: HumanAddr,
    token_id: String,
) -> Result<HandleResponse, ContractError> {
    _transfer_nft(deps, &env, &info, &recipient, &token_id)?;

    Ok(HandleResponse {
        messages: vec![],
        attributes: vec![
            attr("action", "transfer_nft"),
            attr("sender", info.sender),
            attr("recipient", recipient),
            attr("token_id", token_id),
        ],
        data: None,
    })
}

pub fn handle_update_nft(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    token_id: String,
    name: String,
    description: Option<String>,
    image: Option<String>,
) -> Result<HandleResponse, ContractError> {
    let sender_raw = deps.api.canonical_address(&info.sender)?;

    // update name and description if existed
    tokens().update(deps.storage, &token_id, |old| match old {
        Some(mut token) => {
            // only owner can update token
            if !token.owner.eq(&sender_raw) {
                return Err(ContractError::Unauthorized {});
            }
            check_size!(name, MAX_CHARS_SIZE);
            token.name = name;
            if let Some(description_val) = description {
                check_size!(description_val, MAX_CHARS_SIZE);
                token.description = description_val;
            }
            if let Some(image_val) = image {
                check_size!(image_val, MAX_CHARS_SIZE);
                token.image = image_val;
            }
            Ok(token)
        }
        None => Err(ContractError::TokenNotFound {}),
    })?;

    Ok(HandleResponse::default())
}

pub fn handle_send_nft(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    contract: HumanAddr,
    token_id: String,
    msg: Option<Binary>,
) -> Result<HandleResponse, ContractError> {
    // Transfer token
    _transfer_nft(deps, &env, &info, &contract, &token_id)?;

    let send = Cw721ReceiveMsg {
        sender: info.sender.clone(),
        token_id: token_id.clone(),
        msg,
    };

    // Send message
    Ok(HandleResponse {
        messages: vec![send.into_cosmos_msg(contract.clone())?],
        attributes: vec![
            attr("action", "send_nft"),
            attr("sender", info.sender),
            attr("recipient", contract),
            attr("token_id", token_id),
        ],
        data: None,
    })
}

pub fn _transfer_nft(
    deps: DepsMut,
    env: &Env,
    info: &MessageInfo,
    recipient: &HumanAddr,
    token_id: &str,
) -> Result<TokenInfo, ContractError> {
    let mut token = tokens().load(deps.storage, &token_id)?;
    // ensure we have permissions
    check_can_send(deps.as_ref(), env, info, &token)?;
    // set owner and remove existing approvals
    token.owner = deps.api.canonical_address(recipient)?;
    token.approvals = vec![];
    tokens().save(deps.storage, &token_id, &token)?;
    Ok(token)
}

pub fn handle_approve(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    spender: HumanAddr,
    token_id: String,
    expires: Option<Expiration>,
) -> Result<HandleResponse, ContractError> {
    _update_approvals(deps, &env, &info, &spender, &token_id, true, expires)?;

    Ok(HandleResponse {
        messages: vec![],
        attributes: vec![
            attr("action", "approve"),
            attr("sender", info.sender),
            attr("spender", spender),
            attr("token_id", token_id),
        ],
        data: None,
    })
}

pub fn handle_revoke(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    spender: HumanAddr,
    token_id: String,
) -> Result<HandleResponse, ContractError> {
    _update_approvals(deps, &env, &info, &spender, &token_id, false, None)?;

    Ok(HandleResponse {
        messages: vec![],
        attributes: vec![
            attr("action", "revoke"),
            attr("sender", info.sender),
            attr("spender", spender),
            attr("token_id", token_id),
        ],
        data: None,
    })
}

pub fn _update_approvals(
    deps: DepsMut,
    env: &Env,
    info: &MessageInfo,
    spender: &HumanAddr,
    token_id: &str,
    // if add == false, remove. if add == true, remove then set with this expiration
    add: bool,
    expires: Option<Expiration>,
) -> Result<TokenInfo, ContractError> {
    let mut token = tokens().load(deps.storage, &token_id)?;
    // ensure we have permissions
    check_can_approve(deps.as_ref(), env, info, &token)?;

    // update the approval list (remove any for the same spender before adding)
    let spender_raw = deps.api.canonical_address(&spender)?;
    token.approvals = token
        .approvals
        .into_iter()
        .filter(|apr| apr.spender != spender_raw)
        .collect();

    // only difference between approve and revoke
    if add {
        // reject expired data as invalid
        let expires = expires.unwrap_or_default();
        if expires.is_expired(&env.block) {
            return Err(ContractError::Expired {});
        }
        let approval = Approval {
            spender: spender_raw,
            expires,
        };
        token.approvals.push(approval);
    }

    tokens().save(deps.storage, &token_id, &token)?;

    Ok(token)
}

pub fn handle_approve_all(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    operator: HumanAddr,
    expires: Option<Expiration>,
) -> Result<HandleResponse, ContractError> {
    // reject expired data as invalid
    let expires = expires.unwrap_or_default();
    if expires.is_expired(&env.block) {
        return Err(ContractError::Expired {});
    }

    // set the operator for us
    let sender_raw = deps.api.canonical_address(&info.sender)?;
    let operator_raw = deps.api.canonical_address(&operator)?;
    OPERATORS.save(deps.storage, (&sender_raw, &operator_raw), &expires)?;

    Ok(HandleResponse {
        messages: vec![],
        attributes: vec![
            attr("action", "approve_all"),
            attr("sender", info.sender),
            attr("operator", operator),
        ],
        data: None,
    })
}

pub fn handle_revoke_all(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    operator: HumanAddr,
) -> Result<HandleResponse, ContractError> {
    let sender_raw = deps.api.canonical_address(&info.sender)?;
    let operator_raw = deps.api.canonical_address(&operator)?;
    OPERATORS.remove(deps.storage, (&sender_raw, &operator_raw));

    Ok(HandleResponse {
        messages: vec![],
        attributes: vec![
            attr("action", "revoke_all"),
            attr("sender", info.sender),
            attr("operator", operator),
        ],
        data: None,
    })
}

pub fn handle_change_minter(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    minter: HumanAddr,
) -> Result<HandleResponse, ContractError> {
    let owner_raw = deps.api.canonical_address(&info.sender)?;
    let owner = OWNER.load(deps.storage)?;
    if !owner.eq(&owner_raw) {
        return Err(ContractError::Unauthorized {});
    }
    let minter = deps.api.canonical_address(&minter)?;
    MINTER.save(deps.storage, &minter)?;
    Ok(HandleResponse {
        messages: vec![],
        attributes: vec![
            attr("action", "change_minter"),
            attr("minter", minter),
            attr("owner", info.sender),
        ],
        data: None,
    })
}

/// returns true iff the sender can execute approve or reject on the contract
fn check_can_approve(
    deps: Deps,
    env: &Env,
    info: &MessageInfo,
    token: &TokenInfo,
) -> Result<(), ContractError> {
    // owner can approve
    let sender_raw = deps.api.canonical_address(&info.sender)?;
    if token.owner == sender_raw {
        return Ok(());
    }
    // operator can approve
    let op = OPERATORS.may_load(deps.storage, (&token.owner, &sender_raw))?;
    match op {
        Some(ex) => {
            if ex.is_expired(&env.block) {
                Err(ContractError::Unauthorized {})
            } else {
                Ok(())
            }
        }
        None => Err(ContractError::Unauthorized {}),
    }
}

pub fn handle_withdraw_fees(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    address: HumanAddr,
    fees: u128,
) -> Result<HandleResponse, ContractError> {
    let owner_raw = deps.api.canonical_address(&info.sender)?;
    let owner = OWNER.load(deps.storage)?;
    if !owner.eq(&owner_raw) {
        return Err(ContractError::Unauthorized {});
    }

    let withdraw_msg = BankMsg::Send {
        from_address: env.contract.address.clone(),
        to_address: address.clone(),
        amount: coins(fees, DENOM),
    }
    .into();
    let res = HandleResponse {
        messages: vec![withdraw_msg],
        attributes: vec![
            attr("function_type", "withdraw_fees"),
            attr("to_address", address),
            attr("amount", fees),
        ],
        data: None,
    };

    Ok(res)
}

/// returns true if the sender can transfer ownership of the token
fn check_can_send(
    deps: Deps,
    env: &Env,
    info: &MessageInfo,
    token: &TokenInfo,
) -> Result<(), ContractError> {
    // owner can send
    let sender_raw = deps.api.canonical_address(&info.sender)?;
    if token.owner == sender_raw {
        return Ok(());
    }

    // any non-expired token approval can send
    if token
        .approvals
        .iter()
        .any(|apr| apr.spender == sender_raw && !apr.expires.is_expired(&env.block))
    {
        return Ok(());
    }

    // operator can send
    let op = OPERATORS.may_load(deps.storage, (&token.owner, &sender_raw))?;
    match op {
        Some(ex) => {
            if ex.is_expired(&env.block) {
                Err(ContractError::Unauthorized {})
            } else {
                Ok(())
            }
        }
        None => Err(ContractError::Unauthorized {}),
    }
}

pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Minter {} => to_binary(&query_minter(deps)?),
        QueryMsg::ContractInfo {} => to_binary(&query_contract_info(deps)?),
        QueryMsg::NftInfo { token_id } => to_binary(&query_nft_info(deps, token_id)?),
        QueryMsg::OwnerOf {
            token_id,
            include_expired,
        } => to_binary(&query_owner_of(
            deps,
            env,
            token_id,
            include_expired.unwrap_or(false),
        )?),
        QueryMsg::AllNftInfo {
            token_id,
            include_expired,
        } => to_binary(&query_all_nft_info(
            deps,
            env,
            token_id,
            include_expired.unwrap_or(false),
        )?),
        QueryMsg::ApprovedForAll {
            owner,
            include_expired,
            start_after,
            limit,
        } => to_binary(&query_all_approvals(
            deps,
            env,
            owner,
            include_expired.unwrap_or(false),
            start_after,
            limit,
        )?),
        QueryMsg::NumTokens {} => to_binary(&query_num_tokens(deps)?),
        QueryMsg::Tokens {
            owner,
            start_after,
            limit,
        } => to_binary(&query_tokens(deps, owner, start_after, limit)?),
        QueryMsg::AllTokens { start_after, limit } => {
            to_binary(&query_all_tokens(deps, start_after, limit)?)
        }
    }
}

fn query_minter(deps: Deps) -> StdResult<MinterResponse> {
    let minter_raw = MINTER.load(deps.storage)?;
    let minter = deps.api.human_address(&minter_raw)?;
    Ok(MinterResponse { minter })
}

fn query_contract_info(deps: Deps) -> StdResult<ContractInfoResponse> {
    CONTRACT_INFO.load(deps.storage)
}

fn query_num_tokens(deps: Deps) -> StdResult<NumTokensResponse> {
    let count = num_tokens(deps.storage)?;
    Ok(NumTokensResponse { count })
}

fn query_nft_info(deps: Deps, token_id: String) -> StdResult<NftInfoResponse> {
    let info = tokens().load(deps.storage, &token_id)?;
    Ok(NftInfoResponse {
        name: info.name,
        description: info.description,
        image: info.image,
    })
}

fn query_owner_of(
    deps: Deps,
    env: Env,
    token_id: String,
    include_expired: bool,
) -> StdResult<OwnerOfResponse> {
    let info = tokens().load(deps.storage, &token_id)?;
    Ok(OwnerOfResponse {
        owner: deps.api.human_address(&info.owner)?,
        approvals: humanize_approvals(deps.api, &env.block, &info, include_expired)?,
    })
}

fn query_all_approvals(
    deps: Deps,
    env: Env,
    owner: HumanAddr,
    include_expired: bool,
    start_after: Option<HumanAddr>,
    limit: Option<u32>,
) -> StdResult<ApprovedForAllResponse> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let start_canon = maybe_canonical(deps.api, start_after)?;
    let start = start_canon.map(Bound::exclusive);

    let owner_raw = deps.api.canonical_address(&owner)?;
    let res: StdResult<Vec<_>> = OPERATORS
        .prefix(&owner_raw)
        .range(deps.storage, start, None, Order::Ascending)
        .filter(|r| include_expired || r.is_err() || !r.as_ref().unwrap().1.is_expired(&env.block))
        .take(limit)
        .map(|item| parse_approval(deps.api, item))
        .collect();
    Ok(ApprovedForAllResponse { operators: res? })
}

fn parse_approval(api: &dyn Api, item: StdResult<KV<Expiration>>) -> StdResult<cw721::Approval> {
    item.and_then(|(k, expires)| {
        let spender = api.human_address(&k.into())?;
        Ok(cw721::Approval { spender, expires })
    })
}

fn query_tokens(
    deps: Deps,
    owner: HumanAddr,
    start_after: Option<String>,
    limit: Option<u32>,
) -> StdResult<TokensResponse> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let start = start_after.map(Bound::exclusive);

    let owner_raw = deps.api.canonical_address(&owner)?;
    let tokens: Result<Vec<String>, _> = tokens()
        .idx
        .owner
        .pks(deps.storage, &owner_raw, start, None, Order::Ascending)
        .take(limit)
        .map(String::from_utf8)
        .collect();
    let tokens = tokens.map_err(StdError::invalid_utf8)?;
    Ok(TokensResponse { tokens })
}

fn query_all_tokens(
    deps: Deps,
    start_after: Option<String>,
    limit: Option<u32>,
) -> StdResult<TokensResponse> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let start = start_after.map(Bound::exclusive);

    let tokens: StdResult<Vec<String>> = tokens()
        .range(deps.storage, start, None, Order::Ascending)
        .take(limit)
        .map(|item| item.map(|(k, _)| String::from_utf8_lossy(&k).to_string()))
        .collect();
    Ok(TokensResponse { tokens: tokens? })
}

fn query_all_nft_info(
    deps: Deps,
    env: Env,
    token_id: String,
    include_expired: bool,
) -> StdResult<AllNftInfoResponse> {
    let info = tokens().load(deps.storage, &token_id)?;
    Ok(AllNftInfoResponse {
        access: OwnerOfResponse {
            owner: deps.api.human_address(&info.owner)?,
            approvals: humanize_approvals(deps.api, &env.block, &info, include_expired)?,
        },
        info: NftInfoResponse {
            name: info.name,
            description: info.description,
            image: info.image,
        },
    })
}

fn humanize_approvals(
    api: &dyn Api,
    block: &BlockInfo,
    info: &TokenInfo,
    include_expired: bool,
) -> StdResult<Vec<cw721::Approval>> {
    let iter = info.approvals.iter();
    iter.filter(|apr| include_expired || !apr.expires.is_expired(block))
        .map(|apr| humanize_approval(api, apr))
        .collect()
}

fn humanize_approval(api: &dyn Api, approval: &Approval) -> StdResult<cw721::Approval> {
    Ok(cw721::Approval {
        spender: api.human_address(&approval.spender)?,
        expires: approval.expires,
    })
}
