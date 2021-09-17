use crate::contract::{handle, init, query};
use crate::error::ContractError;
use crate::msg::*;
use crate::state::ContractInfo;
use cosmwasm_std::testing::{mock_info, MockApi, MockQuerier as StdMockQuerier, MockStorage};
use cosmwasm_std::{
    coin, coins, from_binary, from_slice, to_binary, Binary, ContractResult, CosmosMsg, Decimal,
    DepsMut, Env, HandleResponse, HumanAddr, MessageInfo, Order, OwnedDeps, QuerierResult,
    SystemError, SystemResult, Uint128, WasmMsg, WasmQuery,
};
use cw1155::Cw1155ReceiveMsg;
use market::mock::{mock_dependencies, mock_dependencies_wasm, mock_env, MockQuerier, StorageImpl};
use market_1155::{OfferingQueryMsg, OfferingQueryResponse, OfferingsResponse};
use std::cell::RefCell;
use std::ops::{Add, Mul};

const CREATOR: &str = "owner";
const MARKET_ADDR: &str = "market_addr";
const HUB_ADDR: &str = "hub_addr";
const OFFERING_ADDR: &str = "offering_addr";
const OW_1155_ADDR: &str = "1155_addr";
const CONTRACT_NAME: &str = "Auction Marketplace";
const DENOM: &str = "orai";
pub const OFFERING_STORAGE: &str = "datahub_offering";

struct Storage {
    // using RefCell to both support borrow and borrow_mut for & and &mut
    hub_storage: RefCell<OwnedDeps<MockStorage, MockApi, StdMockQuerier>>,
    offering_storage: RefCell<OwnedDeps<MockStorage, MockApi, StdMockQuerier>>,
    ow1155_storage: RefCell<OwnedDeps<MockStorage, MockApi, StdMockQuerier>>,
}
impl Storage {
    fn new() -> Storage {
        let info = mock_info(CREATOR, &[]);
        let mut hub_storage = mock_dependencies(HumanAddr::from(HUB_ADDR), &[]);
        let _res = market_hub::contract::init(
            hub_storage.as_mut(),
            mock_env(HUB_ADDR),
            info.clone(),
            market_hub::msg::InitMsg {
                admins: vec![HumanAddr::from(CREATOR)],
                mutable: true,
                storages: vec![(OFFERING_STORAGE.to_string(), HumanAddr::from(OFFERING_ADDR))],
                implementations: vec![HumanAddr::from(MARKET_ADDR)],
            },
        )
        .unwrap();

        let mut offering_storage = mock_dependencies(HumanAddr::from(OFFERING_ADDR), &[]);
        let _res = market_1155_storage::contract::init(
            offering_storage.as_mut(),
            mock_env(OFFERING_ADDR),
            info.clone(),
            market_1155_storage::msg::InitMsg {
                governance: HumanAddr::from(HUB_ADDR),
            },
        )
        .unwrap();

        let mut ow1155_storage = mock_dependencies(HumanAddr::from(OW_1155_ADDR), &[]);
        let _res = ow1155::contract::init(
            ow1155_storage.as_mut(),
            mock_env(OW_1155_ADDR),
            info.clone(),
            ow1155::msg::InstantiateMsg {
                minter: OW_1155_ADDR.to_string(),
            },
        )
        .unwrap();

        // init storage
        Storage {
            hub_storage: RefCell::new(hub_storage),
            offering_storage: RefCell::new(offering_storage),
            ow1155_storage: RefCell::new(ow1155_storage),
        }
    }

    fn handle_wasm(&self, res: &mut Vec<HandleResponse>, ret: HandleResponse) {
        for msg in &ret.messages {
            // only clone required properties
            if let CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr, msg, ..
            }) = msg
            {
                let result = match contract_addr.as_str() {
                    HUB_ADDR => market_hub::contract::handle(
                        self.hub_storage.borrow_mut().as_mut(),
                        mock_env(MARKET_ADDR),
                        mock_info(MARKET_ADDR, &[]),
                        from_slice(msg).unwrap(),
                    )
                    .ok(),
                    OFFERING_ADDR => market_1155_storage::contract::handle(
                        self.offering_storage.borrow_mut().as_mut(),
                        mock_env(HUB_ADDR),
                        mock_info(HUB_ADDR, &[]),
                        from_slice(msg).unwrap(),
                    )
                    .ok(),
                    OW_1155_ADDR => ow1155::contract::handle(
                        self.ow1155_storage.borrow_mut().as_mut(),
                        mock_env(OW_1155_ADDR),
                        mock_info(OW_1155_ADDR, &[]),
                        from_slice(msg).unwrap(),
                    )
                    .ok(),
                    _ => continue,
                };
                if let Some(result) = result {
                    self.handle_wasm(res, result);
                }
            }
        }
        res.push(ret);
    }

    fn handle(
        &self,
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        msg: HandleMsg,
    ) -> Result<Vec<HandleResponse>, ContractError> {
        let first_res = handle(deps, env, info, msg.clone())?;
        let mut res: Vec<HandleResponse> = vec![];
        self.handle_wasm(&mut res, first_res);
        Ok(res)
    }
}

// for query, should use 2 time only, to prevent DDOS, with handler, it is ok for gas consumption
impl StorageImpl for Storage {
    fn query_wasm(&self, request: &WasmQuery) -> QuerierResult {
        match request {
            WasmQuery::Smart { contract_addr, msg } => {
                let result: Binary = match contract_addr.as_str() {
                    HUB_ADDR => market_hub::contract::query(
                        self.hub_storage.borrow().as_ref(),
                        mock_env(HUB_ADDR),
                        from_slice(msg).unwrap(),
                    )
                    .unwrap_or_default(),
                    OFFERING_ADDR => market_1155_storage::contract::query(
                        self.offering_storage.borrow().as_ref(),
                        mock_env(OFFERING_ADDR),
                        from_slice(msg).unwrap(),
                    )
                    .unwrap_or_default(),
                    OW_1155_ADDR => ow1155::contract::query(
                        self.ow1155_storage.borrow().as_ref(),
                        mock_env(OW_1155_ADDR),
                        from_slice(msg).unwrap(),
                    )
                    .unwrap_or_default(),
                    _ => Binary::default(),
                };

                SystemResult::Ok(ContractResult::Ok(result))
            }

            _ => SystemResult::Err(SystemError::UnsupportedRequest {
                kind: "Not implemented".to_string(),
            }),
        }
    }
}

fn setup_contract<'a>(
    storage: &'a Storage,
) -> (
    OwnedDeps<MockStorage, MockApi, MockQuerier<'a, Storage>>,
    Env,
) {
    let contract_env = mock_env(MARKET_ADDR);
    let mut deps =
        mock_dependencies_wasm(HumanAddr::from(MARKET_ADDR), &coins(100000, DENOM), storage);

    let msg = InitMsg {
        name: String::from(CONTRACT_NAME),
        denom: DENOM.into(),
        fee: 1, // 0.1%
        // creator can update storage contract
        governance: HumanAddr::from(HUB_ADDR),
        max_royalty: 20,
    };
    let info = mock_info(CREATOR, &[]);
    let res = init(deps.as_mut(), contract_env.clone(), info.clone(), msg).unwrap();
    assert_eq!(0, res.messages.len());

    (deps, contract_env)
}

#[test]
fn update_info_test() {
    let storage = Storage::new();
    let (mut deps, contract_env) = setup_contract(&storage);

    // update contract to set fees
    let update_info = UpdateContractMsg {
        name: None,
        creator: None,
        denom: Some(DENOM.to_string()),
        // 2.5% free
        fee: Some(5),
        auction_duration: None,
        step_price: None,
    };
    let update_info_msg = HandleMsg::UpdateInfo(update_info);

    // random account cannot update info, only creator
    let info_unauthorized = mock_info("anyone", &vec![coin(5, DENOM)]);

    let mut response = storage.handle(
        deps.as_mut(),
        contract_env.clone(),
        info_unauthorized.clone(),
        update_info_msg.clone(),
    );
    assert_eq!(response.is_err(), true);
    println!("{:?}", response.expect_err("msg"));

    // now we can update the info using creator
    let info = mock_info(CREATOR, &[]);
    response = storage.handle(
        deps.as_mut(),
        contract_env.clone(),
        info,
        update_info_msg.clone(),
    );
    assert_eq!(response.is_err(), false);

    let query_info = QueryMsg::GetContractInfo {};
    let res_info: ContractInfo =
        from_binary(&query(deps.as_ref(), contract_env.clone(), query_info).unwrap()).unwrap();
    assert_eq!(res_info.governance.as_str(), HUB_ADDR);
}

// test royalty

#[test]
fn test_royalties() {
    let storage = Storage::new();
    let (mut deps, contract_env) = setup_contract(&storage);

    let mint_msg = cw1155::Cw1155ExecuteMsg::Mint {
        to: "creator".to_string(),
        token_id: String::from("SellableNFT"),
        value: Uint128::from(100u64),
        msg: None,
    };

    let info_mint = mock_info(OW_1155_ADDR, &vec![coin(50, DENOM)]);

    storage
        .handle(
            deps.as_mut(),
            mock_env(OW_1155_ADDR),
            info_mint.clone(),
            HandleMsg::MintNft {
                contract: HumanAddr::from(OW_1155_ADDR),
                msg: to_binary(&mint_msg).unwrap(),
            },
        )
        .unwrap();

    let query_data = storage
        .query_wasm(&WasmQuery::Smart {
            contract_addr: HumanAddr::from(OW_1155_ADDR),
            msg: to_binary(&cw1155::Cw1155QueryMsg::CreatorOf {
                token_id: String::from("SellableNFT"),
            })
            .unwrap(),
        })
        .unwrap()
        .unwrap();
    let res: HumanAddr = from_binary(&query_data).unwrap();
    println!("res query creator of: {:?}", res);

    // beneficiary can release it
    let info_sell = mock_info(OW_1155_ADDR, &vec![coin(50, DENOM)]);
    let msg = HandleMsg::ReceiveNft(Cw1155ReceiveMsg {
        operator: "creator".to_string(),
        token_id: String::from("SellableNFT"),
        from: None,
        amount: Uint128::from(10u64),
        msg: to_binary(&SellNft {
            per_price: Uint128(50),
            royalty: Some(10),
        })
        .unwrap(),
    });
    storage
        .handle(deps.as_mut(), contract_env.clone(), info_sell.clone(), msg)
        .unwrap();

    // latest offering seller as seller
    let offering_bin_first = query(
        deps.as_ref(),
        contract_env.clone(),
        QueryMsg::Offering(OfferingQueryMsg::GetOffering { offering_id: 1 }),
    )
    .unwrap();
    let offering_first: OfferingQueryResponse = from_binary(&offering_bin_first).unwrap();

    println!("offering: {:?}", offering_first);

    let result: OfferingsResponse = from_binary(
        &query(
            deps.as_ref(),
            contract_env.clone(),
            QueryMsg::Offering(OfferingQueryMsg::GetOfferings {
                offset: None,
                limit: None,
                order: None,
            }),
        )
        .unwrap(),
    )
    .unwrap();
    println!("result {:?}", result);

    let buy_msg = HandleMsg::BuyNft { offering_id: 1 };
    let info_buy = mock_info("seller", &coins(500, DENOM));

    storage
        .handle(deps.as_mut(), contract_env.clone(), info_buy, buy_msg)
        .unwrap();

    let info_sell = mock_info(OW_1155_ADDR, &vec![coin(50, DENOM)]);
    let msg = HandleMsg::ReceiveNft(Cw1155ReceiveMsg {
        operator: "seller".to_string(),
        token_id: String::from("SellableNFT"),
        from: None,
        amount: Uint128::from(10u64),
        msg: to_binary(&SellNft {
            per_price: Uint128(50),
            royalty: None,
        })
        .unwrap(),
    });
    storage
        .handle(deps.as_mut(), contract_env.clone(), info_sell.clone(), msg)
        .unwrap();

    // latest offering seller as seller
    let offering_bin = query(
        deps.as_ref(),
        contract_env.clone(),
        QueryMsg::Offering(OfferingQueryMsg::GetOffering { offering_id: 2 }),
    )
    .unwrap();
    let offering: OfferingQueryResponse = from_binary(&offering_bin).unwrap();

    println!("offering 2nd sell: {:?}", offering);

    // buy again to let seller != creator
    let buy_msg = HandleMsg::BuyNft { offering_id: 2 };
    let info_buy = mock_info("buyer1", &coins(500, DENOM));

    let results = storage
        .handle(deps.as_mut(), contract_env.clone(), info_buy, buy_msg)
        .unwrap();

    let mut total_payment = Uint128::from(0u128);
    let mut royalty_creator = Uint128::from(0u128);
    for result in results {
        for message in result.clone().messages {
            if let CosmosMsg::Bank(msg) = message {
                match msg {
                    cosmwasm_std::BankMsg::Send {
                        from_address,
                        to_address,
                        amount,
                    } => {
                        println!("from address: {}", from_address);
                        println!("to address: {}", to_address);
                        println!("amount: {:?}", amount);
                        let amount = amount[0].amount;
                        if to_address.eq(&HumanAddr::from("creator")) {
                            royalty_creator = amount;
                        }
                        // check royalty sent to creator
                        if offering.royalty.is_some()
                            && to_address.eq(&offering.clone().royalty.clone().unwrap().owner)
                        {
                            println!("in here offering royalty");
                            // royalty_creator = amount;
                            let price = offering
                                .offering
                                .per_price
                                .mul(Decimal::from_ratio(offering.offering.amount.u128(), 1u128));
                            assert_eq!(
                                price.mul(Decimal::percent(
                                    offering.royalty.clone().unwrap().per_royalty
                                )),
                                amount
                            );
                        }
                        // check royalty sent to seller
                        if to_address.eq(&offering.clone().offering.seller) {
                            total_payment = total_payment + amount;
                            println!("in here to increment total payment: {:?}", total_payment);
                        }
                    }
                }
            } else {
            }
        }
    }
    assert_eq!(total_payment + royalty_creator, Uint128::from(500u128));

    // // Offering should be listed
    // let res = String::from_utf8(
    //     query(
    //         deps.as_ref(),
    //         contract_env.clone(),
    //         QueryMsg::Offering(OfferingQueryMsg::G {
    //             contract: "nft_contract".into(),
    //             token_id: "SellableNFT".into(),
    //         }),
    //     )
    //     .unwrap()
    //     .to_vec(),
    // )
    // .unwrap();

    // println!("res: {}", res);

    // // when the creator buys again the nft and re-sell, the royalty should reset
}

#[test]
fn withdraw_offering() {
    let storage = Storage::new();
    let (mut deps, contract_env) = setup_contract(&storage);

    // beneficiary can release it
    let info = mock_info("offering", &coins(2, DENOM));

    let sell_msg = SellNft {
        per_price: Uint128(50),
        royalty: Some(10),
    };

    println!("msg :{}", to_binary(&sell_msg).unwrap());

    let msg = HandleMsg::ReceiveNft(Cw1155ReceiveMsg {
        operator: "seller".to_string(),
        token_id: String::from("SellableNFT"),
        from: None,
        amount: Uint128::from(10u64),
        msg: to_binary(&SellNft {
            per_price: Uint128(90),
            royalty: Some(10),
        })
        .unwrap(),
    });
    let _res = storage
        .handle(deps.as_mut(), contract_env.clone(), info, msg)
        .unwrap();

    // Offering should be listed
    let res: OfferingsResponse = from_binary(
        &query(
            deps.as_ref(),
            contract_env.clone(),
            QueryMsg::Offering(OfferingQueryMsg::GetOfferings {
                offset: None,
                limit: None,
                order: None,
            }),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(1, res.offerings.len());

    // withdraw offering
    let withdraw_info = mock_info("seller", &coins(2, DENOM));
    let withdraw_info_unauthorized = mock_info("sellerr", &coins(2, DENOM));
    let withdraw_msg = HandleMsg::WithdrawNft {
        offering_id: res.offerings[0].offering.id.clone().unwrap(),
    };

    // unhappy path
    let _res_unhappy = storage.handle(
        deps.as_mut(),
        contract_env.clone(),
        withdraw_info_unauthorized,
        withdraw_msg.clone(),
    );
    assert_eq!(_res_unhappy.is_err(), true);

    // happy path
    let _res = storage
        .handle(
            deps.as_mut(),
            contract_env.clone(),
            withdraw_info,
            withdraw_msg,
        )
        .unwrap();

    // Offering should be removed
    let res2: OfferingsResponse = from_binary(
        &query(
            deps.as_ref(),
            contract_env.clone(),
            QueryMsg::Offering(OfferingQueryMsg::GetOfferings {
                offset: None,
                limit: None,
                order: None,
            }),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(0, res2.offerings.len());
}

// #[test]
// fn creator_update_royalty_happy_path() {
//     let storage = Storage::new();
//     let (mut deps, contract_env) = setup_contract(&storage);

//     // beneficiary can release it
//     let info = mock_info("offering", &coins(2, DENOM));

//     let sell_msg = SellNft {
//         off_price: Uint128(50),
//         royalty: Some(10),
//     };

//     println!("msg :{}", to_binary(&sell_msg).unwrap());

//     let msg = HandleMsg::ReceiveNft(Cw721ReceiveMsg {
//         sender: HumanAddr::from("seller"),
//         token_id: String::from("SellableNFT"),
//         msg: to_binary(&sell_msg).ok(),
//     });
//     let _res = storage
//         .handle(deps.as_mut(), contract_env.clone(), info.clone(), msg)
//         .unwrap();

//     // Offering should be listed
//     let res: OfferingsResponse = from_binary(
//         &query(
//             deps.as_ref(),
//             contract_env.clone(),
//             QueryMsg::Offering(OfferingQueryMsg::GetOfferings {
//                 offset: None,
//                 limit: None,
//                 order: None,
//             }),
//         )
//         .unwrap(),
//     )
//     .unwrap();
//     assert_eq!(1, res.offerings.len());

//     let mut buy_msg = HandleMsg::BuyNft { offering_id: 1 };
//     let info_buy = mock_info("buyer", &coins(50, DENOM));
//     storage
//         .handle(
//             deps.as_mut(),
//             contract_env.clone(),
//             info_buy,
//             buy_msg.clone(),
//         )
//         .unwrap();

//     // sell again
//     let msg = HandleMsg::ReceiveNft(Cw721ReceiveMsg {
//         sender: HumanAddr::from("buyer"),
//         token_id: String::from("SellableNFT"),
//         msg: to_binary(&SellNft {
//             off_price: Uint128(70),
//             royalty: Some(10),
//         })
//         .ok(),
//     });
//     storage
//         .handle(deps.as_mut(), contract_env.clone(), info.clone(), msg)
//         .unwrap();

//     let result: OfferingsResponse = from_binary(
//         &query(
//             deps.as_ref(),
//             contract_env.clone(),
//             QueryMsg::Offering(OfferingQueryMsg::GetOfferings {
//                 offset: None,
//                 limit: None,
//                 order: None,
//             }),
//         )
//         .unwrap(),
//     )
//     .unwrap();
//     println!("token belongs to buyer now {:?}", result);

//     // beneficiary can release it
//     let info_buy_2 = mock_info("seller", &coins(999, DENOM));
//     // now the creator buys again
//     buy_msg = HandleMsg::BuyNft { offering_id: 2 };
//     storage
//         .handle(deps.as_mut(), contract_env.clone(), info_buy_2, buy_msg)
//         .unwrap();

//     // finally, creator sells again to reset royalty
//     let msg = HandleMsg::ReceiveNft(Cw721ReceiveMsg {
//         sender: HumanAddr::from("seller"),
//         token_id: String::from("SellableNFT"),
//         msg: to_binary(&SellNft {
//             off_price: Uint128(70),
//             royalty: Some(20),
//         })
//         .ok(),
//     });
//     storage
//         .handle(deps.as_mut(), contract_env.clone(), info.clone(), msg)
//         .unwrap();

//     let offering_result: QueryOfferingsResult = from_binary(
//         &query(
//             deps.as_ref(),
//             contract_env.clone(),
//             QueryMsg::Offering(OfferingQueryMsg::GetOffering { offering_id: 3 }),
//         )
//         .unwrap(),
//     )
//     .unwrap();
//     println!("token belongs to creator now {:?}", offering_result);
//     let royalty = offering_result.royalty_creator.unwrap().royalty;
//     let owner_royalty = offering_result.royalty_owner;
//     assert_eq!(royalty, 20);
//     assert_eq!(owner_royalty, None);
// }

#[test]
fn test_royalties_unhappy() {
    let storage = Storage::new();
    let (mut deps, contract_env) = setup_contract(&storage);

    // beneficiary can release it
    let info = mock_info("offering", &coins(2, DENOM));

    let sell_msg = SellNft {
        per_price: Uint128(50),
        royalty: Some(10),
    };

    println!("msg :{}", to_binary(&sell_msg).unwrap());

    let msg = HandleMsg::ReceiveNft(Cw1155ReceiveMsg {
        operator: "seller".to_string(),
        token_id: String::from("SellableNFT"),
        from: None,
        amount: Uint128::from(10u64),
        msg: to_binary(&SellNft {
            per_price: Uint128(90),
            royalty: Some(10),
        })
        .unwrap(),
    });
    let _res = storage
        .handle(
            deps.as_mut(),
            contract_env.clone(),
            info.clone(),
            msg.clone(),
        )
        .unwrap();

    // already on sale case
    let _res_already_sale = storage.handle(deps.as_mut(), contract_env.clone(), info.clone(), msg);
    assert_eq!(_res_already_sale.is_err(), true);

    // insufficient funds
    let buy_msg = HandleMsg::BuyNft { offering_id: 1 };
    let info_buy = mock_info("buyer", &coins(49, DENOM));
    let _res_insufficient_funds = storage.handle(
        deps.as_mut(),
        contract_env.clone(),
        info_buy,
        buy_msg.clone(),
    );
    assert_eq!(_res_insufficient_funds.is_err(), true);
}
