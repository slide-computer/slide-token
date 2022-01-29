use std::collections::HashMap;

use ic_cdk::export::candid::CandidType;
use ic_cdk::export::Principal;
use serde::Deserialize;

#[derive(CandidType, Clone, Deserialize, PartialEq, Eq, Hash)]
pub enum User {
    Principal(Principal),
    Address(String),
}

#[derive(CandidType, Clone, Deserialize)]
pub struct Asset {
    pub uri: String,
    pub mime: String,
    pub extension: Option<Extension>,
}

#[derive(CandidType, Clone, Deserialize)]
pub struct Token {
    pub id: String,
    pub mint: usize,
    pub asset: Asset,
    pub owner: User,
}

#[derive(CandidType, Clone, Deserialize)]
pub struct Extension {
    pub principal: Principal,
    pub types: Vec<String>,
}

#[derive(CandidType, Clone, Default, Deserialize)]
pub struct State {
    pub symbol: String,
    pub name: String,
    pub tokens: Vec<Token>,
    pub approved: HashMap<String, User>,
    pub operators: HashMap<User, Vec<User>>,
    pub controllers: Vec<Principal>,
    pub extension: Option<Extension>,
}

#[derive(CandidType, Clone, Deserialize)]
pub enum ErrorCode {
    ManagementCanisterUnreachable,
    UuidInvalidByteLength,
    TokenNotFound,
    IsAlreadyOperator,
    OperatorNotFound,
    IsAlreadyController,
    ControllerNotFound,
}
