use crate::contract::{
    get_handle_msg, get_storage_addr, query_ai_royalty, query_datahub, AI_ROYALTY_STORAGE,
    DATAHUB_STORAGE,
};
use crate::error::ContractError;
use crate::msg::{ProxyQueryMsg, SellNft};
use crate::state::{ContractInfo, CONTRACT_INFO};
use cosmwasm_std::HumanAddr;
use cosmwasm_std::{
    attr, coins, from_binary, to_binary, BankMsg, CosmosMsg, Decimal, Deps, DepsMut, Env,
    HandleResponse, MessageInfo, StdResult, Uint128, WasmMsg,
};
use cw1155::{Cw1155ExecuteMsg, Cw1155ReceiveMsg};
use market_1155::{DataHubHandleMsg, DataHubQueryMsg, Offering};
use market_ai_royalty::{AiRoyaltyHandleMsg, AiRoyaltyQueryMsg, MintMsg, Royalty, RoyaltyMsg};
use std::ops::{Mul, Sub};

pub fn add_msg_royalty(
    sender: &str,
    governance: &str,
    msg: RoyaltyMsg,
) -> StdResult<Vec<CosmosMsg>> {
    let mut cosmos_msgs: Vec<CosmosMsg> = vec![];
    // update ai royalty provider
    cosmos_msgs.push(get_handle_msg(
        governance,
        AI_ROYALTY_STORAGE,
        AiRoyaltyHandleMsg::UpdateRoyalty(msg.clone()),
    )?);

    // update creator as the caller of the mint tx
    cosmos_msgs.push(get_handle_msg(
        governance,
        AI_ROYALTY_STORAGE,
        AiRoyaltyHandleMsg::UpdateRoyalty(RoyaltyMsg {
            contract_addr: msg.contract_addr,
            token_id: msg.token_id,
            royalty_owner: HumanAddr(sender.to_string()),
        }),
    )?);
    Ok(cosmos_msgs)
}

pub fn try_handle_mint(
    deps: DepsMut,
    info: MessageInfo,
    contract: HumanAddr,
    msg: MintMsg,
) -> Result<HandleResponse, ContractError> {
    let mint_msg = WasmMsg::Execute {
        contract_addr: contract.clone(),
        msg: msg.msg.clone(),
        send: vec![],
    }
    .into();
    let ContractInfo { governance, .. } = CONTRACT_INFO.load(deps.storage)?;

    let mut cosmos_msgs = add_msg_royalty(info.sender.as_str(), &governance, msg.royalty_msg)?;
    cosmos_msgs.push(mint_msg);

    let response = HandleResponse {
        messages: cosmos_msgs,
        attributes: vec![attr("action", "mint_nft"), attr("invoker", info.sender)],
        data: None,
    };

    Ok(response)
}

pub fn try_buy(
    deps: DepsMut,
    info: MessageInfo,
    env: Env,
    offering_id: u64,
) -> Result<HandleResponse, ContractError> {
    let ContractInfo { governance, .. } = CONTRACT_INFO.load(deps.storage)?;

    // check if offering exists, when return StdError => it will show EOF while parsing a JSON value.
    let off: Offering = get_offering(deps.as_ref(), offering_id)?;
    let seller_addr = off.seller;

    let mut cosmos_msgs = vec![];
    // check for enough coins, if has price then payout to all participants
    if !off.per_price.is_zero() {
        let contract_info = CONTRACT_INFO.load(deps.storage)?;
        // find the desired coin to process
        if let Some(sent_fund) = info
            .sent_funds
            .iter()
            .find(|fund| fund.denom.eq(&contract_info.denom))
        {
            let price = off
                .per_price
                .mul(Decimal::from_ratio(off.amount.u128(), 1u128));
            if sent_fund.amount.lt(&price) {
                return Err(ContractError::InsufficientFunds {});
            }

            let mut seller_amount = sent_fund.amount;

            // pay for the owner of this minter contract if there is fee set in marketplace
            if contract_info.fee > 0 {
                let fee_amount = price.mul(Decimal::permille(contract_info.fee));
                // Rust will automatically floor down the value to 0 if amount is too small => error
                if fee_amount.gt(&Uint128::from(0u128)) {
                    seller_amount = seller_amount.sub(fee_amount)?;
                    cosmos_msgs.push(
                        BankMsg::Send {
                            from_address: env.contract.address.clone(),
                            to_address: HumanAddr::from(contract_info.creator),
                            amount: coins(fee_amount.u128(), &contract_info.denom),
                        }
                        .into(),
                    );
                }
            }
            // pay for creator, ai provider and others
            let royalties: Result<Vec<Royalty>, ContractError> =
                get_royalties(deps.as_ref(), &off.token_id);
            if royalties.is_ok() {
                println!("Ready to pay for the creator and provider");
                for royalty in royalties.unwrap() {
                    // royalty = total price * royalty percentage
                    let creator_amount = price.mul(Decimal::percent(royalty.royalty));
                    if creator_amount.gt(&Uint128::from(0u128)) {
                        seller_amount = seller_amount.sub(creator_amount)?;
                        cosmos_msgs.push(
                            BankMsg::Send {
                                from_address: env.contract.address.clone(),
                                to_address: royalty.royalty_owner,
                                amount: coins(creator_amount.u128(), &contract_info.denom),
                            }
                            .into(),
                        );
                    }
                }
            }

            // pay the left to the seller
            cosmos_msgs.push(
                BankMsg::Send {
                    from_address: env.contract.address.clone(),
                    to_address: seller_addr.clone(),
                    amount: coins(seller_amount.u128(), &contract_info.denom),
                }
                .into(),
            );
        } else {
            return Err(ContractError::InvalidSentFundAmount {});
        }
    }

    // create transfer cw721 msg
    let transfer_cw721_msg = Cw1155ExecuteMsg::SendFrom {
        token_id: off.token_id.clone(),
        from: env.contract.address.to_string(),
        to: info.sender.clone().to_string(),
        value: off.amount,
        msg: None,
    };

    // if everything is fine transfer NFT token to buyer
    cosmos_msgs.push(
        WasmMsg::Execute {
            contract_addr: off.contract_addr,
            msg: to_binary(&transfer_cw721_msg)?,
            send: vec![],
        }
        .into(),
    );

    // remove offering in the offering storage
    cosmos_msgs.push(get_handle_msg(
        governance.as_str(),
        DATAHUB_STORAGE,
        DataHubHandleMsg::RemoveOffering { id: offering_id },
    )?);

    Ok(HandleResponse {
        messages: cosmos_msgs,
        attributes: vec![
            attr("action", "buy_nft"),
            attr("buyer", info.sender),
            attr("seller", seller_addr),
            attr("token_id", off.token_id),
            attr("offering_id", offering_id),
        ],
        data: None,
    })
}

pub fn try_withdraw(
    deps: DepsMut,
    info: MessageInfo,
    env: Env,
    offering_id: u64,
) -> Result<HandleResponse, ContractError> {
    let ContractInfo {
        governance,
        creator,
        ..
    } = CONTRACT_INFO.load(deps.storage)?;
    // check if token_id is currently sold by the requesting address
    // check if offering exists, when return StdError => it will show EOF while parsing a JSON value.
    let off: Offering = get_offering(deps.as_ref(), offering_id)?;
    if off.seller.eq(&info.sender) || creator.eq(&info.sender.to_string()) {
        // check if token_id is currently sold by the requesting address
        // transfer token back to original owner
        let transfer_cw721_msg = Cw1155ExecuteMsg::SendFrom {
            from: env.contract.address.to_string(),
            to: off.seller.to_string(),
            token_id: off.token_id.clone(),
            value: off.amount,
            msg: None,
        };

        let exec_cw721_transfer = WasmMsg::Execute {
            contract_addr: off.contract_addr,
            msg: to_binary(&transfer_cw721_msg)?,
            send: vec![],
        };

        let mut cw721_transfer_cosmos_msg: Vec<CosmosMsg> = vec![exec_cw721_transfer.into()];

        // remove offering
        cw721_transfer_cosmos_msg.push(get_handle_msg(
            governance.as_str(),
            DATAHUB_STORAGE,
            DataHubHandleMsg::RemoveOffering { id: offering_id },
        )?);

        return Ok(HandleResponse {
            messages: cw721_transfer_cosmos_msg,
            attributes: vec![
                attr("action", "withdraw_nft"),
                attr("seller", info.sender),
                attr("offering_id", offering_id),
                attr("token_id", off.token_id),
            ],
            data: None,
        });
    }
    Err(ContractError::Unauthorized {})
}

pub fn handle_sell_nft(
    deps: DepsMut,
    info: MessageInfo,
    msg: SellNft,
    rcv_msg: Cw1155ReceiveMsg,
) -> Result<HandleResponse, ContractError> {
    let ContractInfo { governance, .. } = CONTRACT_INFO.load(deps.storage)?;
    // check if same token Id form same original contract is already on sale

    // TODO: This should be commented when we allow multiple owners to sell this nft
    let offering_result: Result<Offering, ContractError> = deps
        .querier
        .query_wasm_smart(
            get_storage_addr(deps.as_ref(), governance.clone(), DATAHUB_STORAGE)?,
            &ProxyQueryMsg::Msg(DataHubQueryMsg::GetOfferingByContractTokenId {
                contract: info.sender.clone(),
                token_id: rcv_msg.token_id.clone(),
            }),
        )
        .map_err(|_| ContractError::InvalidGetOffering {});
    if offering_result.is_ok() {
        return Err(ContractError::TokenOnSale {});
    }

    let offering = Offering {
        id: None,
        token_id: rcv_msg.token_id,
        contract_addr: info.sender.clone(),
        seller: HumanAddr::from(rcv_msg.operator.clone()),
        per_price: msg.per_price,
        amount: rcv_msg.amount,
    };

    let mut cosmos_msgs = vec![];
    // push save message to datahub storage
    cosmos_msgs.push(get_handle_msg(
        governance.as_str(),
        DATAHUB_STORAGE,
        DataHubHandleMsg::UpdateOffering {
            offering: offering.clone(),
        },
    )?);

    Ok(HandleResponse {
        messages: cosmos_msgs,
        attributes: vec![
            attr("action", "sell_nft"),
            attr("original_contract", info.sender),
            attr("seller", rcv_msg.operator),
            attr("per price", offering.per_price.to_string()),
        ],
        data: None,
    })
}

fn get_offering(deps: Deps, offering_id: u64) -> Result<Offering, ContractError> {
    let offering: Offering = from_binary(&query_datahub(
        deps,
        DataHubQueryMsg::GetOfferingState { offering_id },
    )?)
    .map_err(|_| ContractError::InvalidGetOffering {})?;
    Ok(offering)
}

fn get_royalties(deps: Deps, token_id: &str) -> Result<Vec<Royalty>, ContractError> {
    let royalties: Vec<Royalty> = from_binary(&query_ai_royalty(
        deps,
        AiRoyaltyQueryMsg::GetRoyaltiesTokenId {
            token_id: token_id.to_string(),
            offset: None,
            limit: None,
            order: Some(1),
        },
    )?)
    .map_err(|_| ContractError::InvalidGetRoyaltiesTokenId {
        token_id: token_id.to_string(),
    })?;
    Ok(royalties)
}
