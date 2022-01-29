extern crate core;

use std::cmp::min;
use ic_cdk::export::Principal;
use ic_cdk_macros::{init, inspect_message, post_upgrade, pre_upgrade, query, update};
use ic_kit::ic;

use crate::ErrorCode::{ControllerNotFound, IsAlreadyController, IsAlreadyOperator, OperatorNotFound, TokenNotFound};
use crate::types::{Asset, ErrorCode, Extension, State, Token, User};
use crate::utils::{caller_to_user, state, uuid_v4};

mod utils;
mod types;

static PUBLIC_METHODS: &[&str] = &[
    "balance_of",
    "owner_of",
    "get_approved",
    "is_approved_for_all",
    "total_supply",
    "token_by_index",
    "token_of_owner_by_index",
    "token_by_offset",
    "token_of_owner_by_offset",
    "get_extension",
];

static CONTROLLER_METHODS: &[&str] = &[
    "mint",
    "burn",
    "set_asset",
    "set_controller",
    "set_extension",
];

#[query]
fn symbol() -> String {
    state().symbol.clone()
}

#[query]
fn name() -> String {
    state().name.clone()
}

#[query]
fn balance_of(user: User) -> usize {
    state().tokens.iter().filter(|token| token.owner == user).count()
}

#[query]
fn owner_of(token_id: String, user: User) -> Result<bool, ErrorCode> {
    let token = state().tokens
        .iter_mut()
        .find(|token| token.id == token_id)
        .ok_or(TokenNotFound)?;
    Ok(token.owner == user)
}

#[query]
fn get_approved(token_id: String) -> Result<User, ErrorCode> {
    state().approved.get(&token_id).ok_or(TokenNotFound).map(|user| user.clone())
}

#[query]
fn is_approved_for_all(user: User, sub_account: Option<[u8; 32]>) -> bool {
    let caller = ic_cdk::api::caller();
    let user_from_caller = caller_to_user(caller, sub_account);
    if let Some(o) = state().operators.get(&user) {
        return o.contains(&user_from_caller);
    }
    false
}

#[query]
fn total_supply() -> usize {
    state().tokens.len()
}

#[query]
fn tokens(offset: Option<usize>, length: Option<usize>) -> Vec<Token> {
    let start = offset.unwrap_or(0);
    let end = start + length.unwrap_or(state().tokens.iter().count());
    state().tokens
        .iter()
        .enumerate()
        .filter(|(index, _)| index >= &start && index < &end)
        .map(|(_, token)| token.clone())
        .collect::<Vec<Token>>()
}

#[query]
fn tokens_of_owner(user: User, offset: Option<usize>, length: Option<usize>) -> Vec<Token> {
    let start = offset.unwrap_or(0);
    let end = start + length.unwrap_or(state().tokens.iter().count());
    state().tokens
        .iter()
        .enumerate()
        .filter(|(index, token)| token.owner == user && index >= &start && index < &end)
        .map(|(_, token)| token.clone())
        .collect::<Vec<Token>>()
}

#[query]
fn get_extension() -> Option<Extension> {
    state().extension.clone()
}

#[update]
async fn mint(asset: Asset, to: Option<User>) -> Result<Token, ErrorCode> {
    let token = Token {
        id: uuid_v4().await?,
        mint: state().tokens.len() + 1,
        owner: to.unwrap_or(User::Address("0".to_string())),
        asset,
    };
    state().tokens.push(token.clone());
    Ok(token)
}

#[update]
async fn burn(count: usize) -> usize {
    let supply = state().tokens.len();
    for i in 0..(min(count, supply) + 1) {
        let index = supply - i;
        let token = state().tokens.get(index);
        if let Some(t) = token {
            if t.owner != User::Address("0".to_string()) {
                return i - 1;
            }
            state().tokens.remove(index);
        }
    }
    count
}

#[update]
async fn set_asset(token_id: String, asset: Asset) -> Result<Token, ErrorCode> {
    let token = state().tokens
        .iter_mut()
        .find(|token| token.id == token_id)
        .ok_or(TokenNotFound)?;
    token.asset = asset;
    Ok(token.clone())
}

#[update]
async fn transfer_from(from: User, to: User, token_id: String, _: Option<[u8; 32]>) -> Result<(), ErrorCode> {
    let token = state().tokens
        .iter_mut()
        .find(|token| token.owner == from && token.id == token_id)
        .ok_or(TokenNotFound)?;
    token.owner = to.clone();
    state().approved.remove(&token.id);
    if state().tokens.iter().filter(|token| token.owner == from).count() == 0 {
        state().operators.remove(&from);
    }
    if let Some(extension) = &state().extension {
        let _: Result<(), _> = ic_cdk::api::call::call(
            extension.principal,
            "transfer_from_event",
            (from, to, token_id),
        ).await;
    }
    Ok(())
}

#[update]
async fn approve(token_id: String, approved: Option<User>, _: Option<[u8; 32]>) -> Result<(), ErrorCode> {
    let token = state().tokens
        .iter()
        .find(|token| token.id == token_id)
        .ok_or(TokenNotFound)?;
    if let Some(user) = approved.clone() {
        state().approved.insert(token.id.clone(), user);
    } else {
        state().approved.remove(&token.id);
    }
    if let Some(extension) = &state().extension {
        let _: Result<(), _> = ic_cdk::api::call::call(
            extension.principal,
            "approve_event",
            (token_id, approved),
        ).await;
    }
    Ok(())
}

#[update]
async fn set_approval_for_all(operator: User, approved: bool, sub_account: Option<[u8; 32]>) -> Result<(), ErrorCode> {
    let caller = ic_cdk::api::caller();
    let user = caller_to_user(caller, sub_account);
    let operators = state().operators
        .get_mut(&user)
        .unwrap_or_else(|| {
            state().operators.insert(user.clone(), vec![]);
            state().operators.get_mut(&user).unwrap()
        });
    let index = operators.iter().position(|o| *o == operator);
    if approved {
        if index.is_none() {
            operators.push(operator.clone());
        } else {
            return Err(IsAlreadyOperator);
        }
    } else {
        operators.remove(index.ok_or(OperatorNotFound)?);
    }
    if let Some(extension) = &state().extension {
        let _: Result<(), _> = ic_cdk::api::call::call(
            extension.principal,
            "operator_event",
            (operator, approved),
        ).await;
    }
    Ok(())
}

#[update]
fn set_controller(controller: Principal, approved: bool) -> Result<(), ErrorCode> {
    let index = state().controllers.iter().position(|c| *c == controller);
    if approved {
        if index.is_none() {
            state().controllers.push(controller.clone());
        } else {
            return Err(IsAlreadyController);
        }
    } else {
        state().controllers.remove(index.ok_or(ControllerNotFound)?);
    }
    Ok(())
}

#[update]
fn set_extension(extension: Option<Extension>) -> () {
    state().extension = extension;
}

#[inspect_message]
fn inspect_message() {
    let method = ic_cdk::api::call::method_name();
    let caller = ic_cdk::api::caller();
    match &method[..] {
        m if PUBLIC_METHODS.contains(&m) => {
            ic_cdk::api::call::accept_message()
        }
        m if CONTROLLER_METHODS.contains(&m) => {
            if state().controllers.contains(&caller) {
                ic_cdk::api::call::accept_message()
            }
        }
        "transfer_from" => {
            let (from, _, token_id, sub_account, ) = ic_cdk::api::call::arg_data::<(User, User, String, Option<[u8; 32]>, )>();
            let token = state().tokens
                .iter()
                .find(|token| token.id == token_id);
            if let Some(t) = token {
                let user = caller_to_user(caller, sub_account);
                if from == t.owner && (
                    t.owner == user ||
                        state().approved.get(&t.id) == Some(&user) ||
                        state().operators.get(&t.owner).unwrap_or(&vec![]).contains(&user) ||
                        (state().controllers.contains(&caller) && t.owner == User::Address("0".to_string()))
                ) {
                    ic_cdk::api::call::accept_message()
                }
            }
        }
        "approve" => {
            let (from, _, token_id, sub_account, ) = ic_cdk::api::call::arg_data::<(User, User, String, Option<[u8; 32]>, )>();
            let token = state().tokens
                .iter()
                .find(|token| token.id == token_id);
            if let Some(t) = token {
                let user = caller_to_user(caller, sub_account);
                if from == t.owner && (
                    t.owner == user ||
                        state().operators.get(&t.owner).unwrap_or(&vec![]).contains(&user)
                ) {
                    ic_cdk::api::call::accept_message()
                }
            }
        }
        "set_approval_for_all" => {
            let (_, _, sub_account, ) = ic_cdk::api::call::arg_data::<(User, bool, Option<[u8; 32]>, )>();
            let user = caller_to_user(caller, sub_account);
            if state().tokens.iter().filter(|token| token.owner == user).count() > 0 {
                ic_cdk::api::call::accept_message()
            }
        }
        _ => {
            ic_cdk::println!("Method doesn't exist.");
        }
    }
}

#[init]
fn init(controller: Principal, symbol: String, name: String) {
    state().controllers.push(controller);
    state().symbol = symbol;
    state().name = name;
}

#[pre_upgrade]
fn pre_upgrade() {
    ic::stable_store((state(), )).expect("Failed");
}

#[post_upgrade]
fn post_upgrade() {
    let (state, ): (State, ) = ic::stable_restore().expect("Failed");
    ic::store(state);
}
