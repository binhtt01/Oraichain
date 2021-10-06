use cosmwasm_std::HumanAddr;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::state::RoundInfo;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InitMsg {
    pub members: Vec<HumanAddr>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandleMsg {
    ChangeState {
        owner: Option<HumanAddr>,
        round_jump: Option<u64>,
        members: Option<Vec<HumanAddr>>,
    },
    Ping {},
    ResetCount {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    GetRound(HumanAddr),
    GetRounds {
        offset: Option<HumanAddr>,
        limit: Option<u8>,
        order: Option<u8>,
    },
    GetState {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct QueryRoundsResponse {
    pub executor: HumanAddr,
    pub round_info: RoundInfo,
}
