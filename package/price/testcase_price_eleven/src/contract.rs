use crate::msg::{Data, DataSourceQueryMsg, HandleMsg, InitMsg, QueryMsg};
use crate::{error::ContractError, msg::Response};
use cosmwasm_std::{
    to_binary, Api, Binary, Deps, DepsMut, Env, HandleResponse, HumanAddr, InitResponse,
    MessageInfo, Querier, StdResult, Storage,
};

pub fn init(deps: DepsMut, _env: Env, _info: MessageInfo, msg: InitMsg) -> StdResult<InitResponse> {
    Ok(InitResponse::default())
}

// And declare a custom Error variant for the ones where you will want to make use of it
pub fn handle(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: HandleMsg,
) -> Result<HandleResponse, ContractError> {
    Ok(HandleResponse::default())
}

pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Test {
            input,
            output,
            contract,
        } => test_price(deps, &contract, input, output),
    }
}

fn test_price(
    deps: Deps,
    contract: &HumanAddr,
    input: String,
    _output: String,
) -> StdResult<Binary> {
    let msg = DataSourceQueryMsg::Get { input };
    let data_sources: Vec<Data> = deps.querier.query_wasm_smart(contract, &msg)?;
    let response = Response {
        name: String::from(""),
        result: data_sources,
        status: String::from("success"),
    };
    let resp_bin: Binary = to_binary(&response).unwrap();
    Ok(resp_bin)
}
