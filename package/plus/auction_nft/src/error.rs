use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("No data in ReceiveMsg")]
    NoData {},

    #[error("Unauthorized")]
    Unauthorized {},
    // Add any other custom errors you like here.
    // Look at https://docs.rs/thiserror/1.0.21/thiserror/ for details.
    #[error("Insufficient funds")]
    InsufficientFunds {},

    #[error("Invalid zero amount")]
    InvalidZeroAmount {},

    #[error("Invalid denom amount")]
    InvalidDenomAmount {},

    #[error("Sent funds amount is empty")]
    InvalidSentFundAmount {},

    #[error("There is an error while collecting the auction")]
    InvalidGetAuction {},

    #[error("The auction asker address is invalid")]
    InvalidSellerAddr {},

    #[error("The auction contract address is invalid")]
    InvalidContractAddr {},

    #[error("The argument {arg} are invalid")]
    InvalidArgument { arg: String },

    #[error("Token Id from the original contract is already on sale")]
    TokenOnSale {},

    #[error("The start {start} and end {end} are invalid")]
    InvalidBlockNumberArgument { start: u64, end: u64 },
}