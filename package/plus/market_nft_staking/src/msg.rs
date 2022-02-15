use cosmwasm_std::{HumanAddr, Uint128};
use cw1155::Cw1155ReceiveMsg;
use cw721::Cw721ReceiveMsg;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::state::CollectionStakedTokenInfo;

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct InitMsg {
    pub verifier_pubkey_base64: String,
    pub nft_1155_contract_addr: HumanAddr,
    pub nft_721_contract_addr: HumanAddr,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub enum HandleMsg {
    UpdateContractInfo(UpdateContractInfoMsg),
    CreateCollectionPool(CreateCollectionPoolMsg),
    UpdateCollectionPool(UpdateCollectionPoolMsg),
    ReceiveNft(Cw721ReceiveMsg),
    Receive(Cw1155ReceiveMsg),
    Withdraw {
        collection_id: String,
        withdraw_rewards: bool,
    },
    Claim {
        collection_id: String,
    },
    ResetEarnedRewards {
        collection_id: String,
        staker: HumanAddr,
    },
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub struct UpdateContractInfoMsg {
    pub verifier_pubkey_base64: Option<String>,
    pub nft_1155_contract_addr: Option<HumanAddr>,
    pub nft_721_contract_addr: Option<HumanAddr>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub struct CreateCollectionPoolMsg {
    pub collection_id: String,
    pub reward_per_block: Uint128,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub struct UpdateCollectionPoolMsg {
    pub collection_id: String,
    pub reward_per_block: Option<Uint128>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub struct DepositeMsg {
    pub collection_id: String,
    pub withdraw_rewards: bool,
    pub signature_hash: String,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub struct StakeMsgDetail {
    pub collection_id: String,
    pub withdraw_rewards: bool,
    pub nft: CollectionStakedTokenInfo,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    GetContractInfo {},
    GetCollectionPoolInfo {
        collection_id: String,
    },
    GetUniqueCollectionStakerInfo {
        collection_id: String,
        staker_addr: HumanAddr,
    },
    GetCollectionStakerInfoByCollection {
        collection_id: String,
        limit: Option<u8>,
        offset: Option<u64>,
        order: Option<u8>,
    },
    GetCollectionStakerInfoByStaker {
        staker_addr: HumanAddr,
        limit: Option<u8>,
        offset: Option<u64>,
        order: Option<u8>,
    },
    //TestQuery {},
}
