use cosmwasm_std::{HumanAddr, Uint128};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct Offering {
    pub id: Option<u64>,
    pub token_id: String,
    pub contract_addr: HumanAddr,
    pub seller: HumanAddr,
    pub per_price: Uint128,
    pub amount: Uint128,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct Annotation {
    pub id: Option<u64>,
    pub token_id: String,
    pub contract_addr: HumanAddr,
    pub requester: HumanAddr,
    pub annotators: Vec<HumanAddr>,
    pub per_price: Uint128,
    pub amount: Uint128,
    pub deposited: bool,
    pub expired_block: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct MintMsg {
    pub contract_addr: HumanAddr,
    pub creator: HumanAddr,
    pub creator_type: String,
    pub royalty: Option<u64>,
    pub mint: MintIntermediate,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct MintIntermediate {
    pub mint: MintStruct,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct MintStruct {
    pub to: String,
    pub token_id: String,
    pub value: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DataHubHandleMsg {
    // this allow implementation contract to update the storage
    UpdateOffering { offering: Offering },
    RemoveOffering { id: u64 },
    UpdateAnnotation { annotation: Annotation },
    RemoveAnnotation { id: u64 },
}