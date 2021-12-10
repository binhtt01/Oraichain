use std::env::current_dir;
use std::fs::create_dir_all;

use cosmwasm_schema::{export_schema, remove_schemas, schema_for};

use aioracle_members_storage::msg::{HandleMsg, InitMsg, QueryMsg};
use aioracle_members_storage::state::Config;

fn main() {
    let mut out_dir = current_dir().unwrap();
    out_dir.push("artifacts/schema");
    create_dir_all(&out_dir).unwrap();
    remove_schemas(&out_dir).unwrap();

    // messages
    export_schema(&schema_for!(InitMsg), &out_dir);
    export_schema(&schema_for!(HandleMsg), &out_dir);
    export_schema(&schema_for!(QueryMsg), &out_dir);
    // state
    export_schema(&schema_for!(Config), &out_dir);
}
