use std::env::current_dir;
use std::fs::create_dir_all;

use cosmwasm_schema::{export_schema, remove_schemas, schema_for};

use ds_farming::msg::{HandleMsg, InitMsg, QueryMsg, SpecialQuery};

fn main() {
    let mut out_dir = current_dir().unwrap();
    out_dir.push("artifacts/schema");
    create_dir_all(&out_dir).unwrap();
    remove_schemas(&out_dir).unwrap();

    export_schema(&schema_for!(InitMsg), &out_dir);
    export_schema(&schema_for!(HandleMsg), &out_dir);
    export_schema(&schema_for!(QueryMsg), &out_dir);
    export_schema(&schema_for!(SpecialQuery), &out_dir);
}
