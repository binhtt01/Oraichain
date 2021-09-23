use crate::contract::*;
use crate::error::ContractError;
use crate::msg::*;
use crate::state::State;
use cosmwasm_std::testing::{
    mock_dependencies, mock_env, mock_info, MockApi, MockQuerier, MockStorage,
};
use cosmwasm_std::{coins, from_binary, BlockInfo, OwnedDeps};
use cosmwasm_std::{Env, HumanAddr};

const OWNER: &str = "orai1up8ct7kk2hr6x9l37ev6nfgrtqs268tdrevk3d";

fn setup_contract() -> OwnedDeps<MockStorage, MockApi, MockQuerier> {
    let mut deps = mock_dependencies(&coins(100000, "orai"));
    deps.api.canonical_length = 54;
    let msg = InitMsg {};
    let info = mock_info(OWNER, &[]);
    let res = init(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(0, res.messages.len());
    deps
}

#[test]
fn proper_initialization() {
    let mut deps = setup_contract();

    // init ping
    for i in 0..22 {
        let msg = HandleMsg::Ping {};
        let info = mock_info(i.to_string(), &[]);
        handle(deps.as_mut(), mock_env(), info, msg).unwrap();
    }

    // query ping
    let query_ping = QueryMsg::GetRounds {
        offset: Some(0),
        limit: Some(30),
        order: None,
    };
    let query_result: Vec<QueryRoundsResponse> =
        from_binary(&query(deps.as_ref(), mock_env(), query_ping).unwrap()).unwrap();
    for result in query_result {
        println!("result: {:?}", result);
    }

    // update ping
    for i in 0..10 {
        let msg = HandleMsg::Ping {};
        let info = mock_info(i.to_string(), &[]);
        handle(
            deps.as_mut(),
            Env {
                block: BlockInfo {
                    height: 12_645,
                    time: 1_571_797_419,
                    time_nanos: 879305533,
                    chain_id: "cosmos-testnet-14002".to_string(),
                },
                ..mock_env()
            },
            info,
            msg,
        )
        .unwrap();
    }

    // query ping
    let query_ping = QueryMsg::GetRounds {
        offset: Some(0),
        limit: Some(30),
        order: None,
    };
    println!("Query ping 2nd time");
    println!();
    let query_result: Vec<QueryRoundsResponse> =
        from_binary(&query(deps.as_ref(), mock_env(), query_ping).unwrap()).unwrap();
    for result in query_result {
        println!("result: {:?}", result);
    }
}

#[test]
fn update_ping_too_soon() {
    let mut deps = setup_contract();

    // init ping
    for i in 0..22 {
        let msg = HandleMsg::Ping {};
        let info = mock_info(i.to_string(), &[]);
        handle(deps.as_mut(), mock_env(), info, msg).unwrap();
    }

    // query ping
    let query_ping = QueryMsg::GetRounds {
        offset: Some(0),
        limit: Some(30),
        order: None,
    };
    let query_result: Vec<QueryRoundsResponse> =
        from_binary(&query(deps.as_ref(), mock_env(), query_ping).unwrap()).unwrap();
    for result in query_result {
        println!("result: {:?}", result);
    }

    // update ping
    for i in 0..10 {
        let msg = HandleMsg::Ping {};
        let info = mock_info(i.to_string(), &[]);
        assert!(matches!(
            handle(
                deps.as_mut(),
                Env {
                    block: BlockInfo {
                        height: 12_545,
                        time: 1_571_797_419,
                        time_nanos: 879305533,
                        chain_id: "cosmos-testnet-14002".to_string(),
                    },
                    ..mock_env()
                },
                info,
                msg,
            ),
            Err(ContractError::PingTooEarly {})
        ));
    }

    // query ping
    let query_ping = QueryMsg::GetRounds {
        offset: Some(0),
        limit: Some(30),
        order: None,
    };
    println!("Query ping 2nd time");
    println!();
    let query_result: Vec<QueryRoundsResponse> =
        from_binary(&query(deps.as_ref(), mock_env(), query_ping).unwrap()).unwrap();
    for result in query_result {
        println!("result: {:?}", result);
    }
}

#[test]
fn change_owner() {
    let mut deps = setup_contract();

    // unauthorized change owner
    let msg = HandleMsg::ChangeOwner(HumanAddr("new owner".to_string()));
    let info = mock_info(HumanAddr("someone".to_string()), &[]);
    assert!(matches!(
        handle(deps.as_mut(), mock_env(), info.clone(), msg.clone()),
        Err(ContractError::Unauthorized {})
    ));

    // authorized reset
    let info = mock_info(HumanAddr(OWNER.to_string()), &[]);
    handle(deps.as_mut(), mock_env(), info, msg).unwrap();

    // query new state
    let state_query: State =
        from_binary(&query(deps.as_ref(), mock_env(), QueryMsg::GetState {}).unwrap()).unwrap();
    println!("state: {:?}", state_query);
}
