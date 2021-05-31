use std::ops::Add;

use crate::error::ContractError;
use crate::msg::{HandleMsg, InitMsg, Input, Output, QueryMsg};
use crate::state::{config, config_read, State};
use cosmwasm_std::{
    from_slice, to_binary, Binary, Deps, DepsMut, Env, HandleResponse, InitResponse, MessageInfo,
    StdResult,
};

// make use of the custom errors
pub fn init(deps: DepsMut, _env: Env, info: MessageInfo, msg: InitMsg) -> StdResult<InitResponse> {
    let state = State {
        ai_data_source: msg.ai_data_source,
        testcase: msg.testcase,
        owner: deps.api.canonical_address(&info.sender)?,
    };
    config(deps.storage).save(&state)?;

    Ok(InitResponse::default())
}

// And declare a custom Error variant for the ones where you will want to make use of it
// And declare a custom Error variant for the ones where you will want to make use of it
pub fn handle(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: HandleMsg,
) -> Result<HandleResponse, ContractError> {
    match msg {
        HandleMsg::UpdateDatasource { name } => try_update_datasource(deps, info, name),
        HandleMsg::UpdateTestcase { name } => try_update_testcase(deps, info, name),
    }
}

pub fn try_update_datasource(
    deps: DepsMut,
    info: MessageInfo,
    name: Vec<String>,
) -> Result<HandleResponse, ContractError> {
    let api = &deps.api;
    config(deps.storage).update(|mut state| -> Result<_, ContractError> {
        if api.canonical_address(&info.sender)? != state.owner {
            return Err(ContractError::Unauthorized {});
        }
        state.ai_data_source = name;
        Ok(state)
    })?;
    Ok(HandleResponse::default())
}

pub fn try_update_testcase(
    deps: DepsMut,
    info: MessageInfo,
    name: Vec<String>,
) -> Result<HandleResponse, ContractError> {
    let api = &deps.api;
    config(deps.storage).update(|mut state| -> Result<_, ContractError> {
        if api.canonical_address(&info.sender)? != state.owner {
            return Err(ContractError::Unauthorized {});
        }
        state.testcase = name;
        Ok(state)
    })?;
    Ok(HandleResponse::default())
}

pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetDatasource {} => to_binary(&query_datasources(deps)?),
        QueryMsg::GetTestcase {} => to_binary(&query_testcases(deps)?),
        QueryMsg::Aggregate { results } => query_aggregation(deps, results),
    }
}

fn query_datasources(deps: Deps) -> StdResult<Binary> {
    let state = config_read(deps.storage).load()?;
    to_binary(&state.ai_data_source)
}

fn query_testcases(deps: Deps) -> StdResult<Binary> {
    let state = config_read(deps.storage).load()?;
    to_binary(&state.testcase)
}

fn query_aggregation(_deps: Deps, results: Vec<String>) -> StdResult<Binary> {
    println!("Hello, world!");
    let mut aggregation_result: Vec<Output> = Vec::new();
    let price_data: Vec<Input> = from_slice(results[0].as_bytes()).unwrap();
    for res in price_data {
        // split to calculate largest precision of the price
        let mut largest_precision: usize = 0;
        for mut price in res.prices.clone() {
            let dot_pos = price.find('.').unwrap();
            price = price[dot_pos..].to_string();
            println!("price to find large precision: {}", price);
            if price.len() > largest_precision {
                largest_precision = price.len();
            }
        }
        let mut sum: u128 = 0;
        let mut count = 0;
        for mut price in res.prices {
            println!("original price: {}", price);
            let dot_pos = price.find('.').unwrap();
            // plus one because postiion starts at 0
            let dot_add = dot_pos.add(largest_precision + 1);
            if price.len() > dot_add {
                price.insert(dot_add, '.');
                price = price[..dot_add].to_string();
            } else {
                while price.len() < dot_add {
                    price.push('0');
                }
            }
            price.remove(dot_pos);
            let price_int: u128 = price.parse().unwrap();
            println!("price: {}", price_int);
            sum += price_int;
            count += 1;
        }
        println!("sum: {}", sum);
        let mean = sum / count;
        let mut mean_price = mean.to_string();
        while mean_price.len() <= largest_precision {
            mean_price.insert(0, '0');
        }
        println!("mean price len: {}", mean_price);
        mean_price.insert(mean_price.len().wrapping_sub(largest_precision), '.');
        println!("mean price: {}", mean_price);

        let data: Output = Output {
            name: res.name,
            price: mean_price,
        };
        aggregation_result.push(data.clone());
    }
    let result_bin = to_binary(&aggregation_result).unwrap();
    Ok(result_bin)
}

#[test]
fn assert_aggregate() {
    let mut aggregation_result: Vec<Output> = Vec::new();
    let resp = format!(
        "[{{\"name\":\"ETH\",\"prices\":[\"{}\",\"{}\",\"{}\"]}},{{\"name\":\"BTC\",\"prices\":[\"{}\",\"{}\"]}}]",
        "0.00000000000018900", "0.00000001305", "0.00000000006", "2801.2341", "200.1"
    );
    let price_data: Vec<Input> = from_slice(resp.as_bytes()).unwrap();
    for res in price_data {
        // split to calculate largest precision of the price
        let mut largest_precision: usize = 0;
        for mut price in res.prices.clone() {
            let dot_pos = price.find('.').unwrap();
            price = price[dot_pos..].to_string();
            println!("price to find large precision: {}", price);
            if price.len() > largest_precision {
                largest_precision = price.len();
            }
        }
        let mut sum: u128 = 0;
        let mut count = 0;
        for mut price in res.prices {
            println!("original price: {}", price);
            let dot_pos = price.find('.').unwrap();
            // plus one because postiion starts at 0
            let dot_add = dot_pos.add(largest_precision + 1);
            if price.len() > dot_add {
                price.insert(dot_add, '.');
                price = price[..dot_add].to_string();
            } else {
                while price.len() < dot_add {
                    price.push('0');
                }
            }
            price.remove(dot_pos);
            let price_int: u128 = price.parse().unwrap();
            println!("price: {}", price_int);
            sum += price_int;
            count += 1;
        }
        println!("sum: {}", sum);
        let mean = sum / count;
        let mut mean_price = mean.to_string();
        while mean_price.len() <= largest_precision {
            mean_price.insert(0, '0');
        }
        println!("mean price len: {}", mean_price);
        mean_price.insert(mean_price.len().wrapping_sub(largest_precision), '.');
        println!("mean price: {}", mean_price);

        let data: Output = Output {
            name: res.name,
            price: mean_price,
        };
        aggregation_result.push(data.clone());
    }
    for result in aggregation_result {
        println!("name: {}", result.name);
        println!("result: {}", result.price);
    }
}