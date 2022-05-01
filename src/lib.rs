extern crate core;

use std::cell::RefCell;
use std::collections::HashMap;
use std::convert::TryInto;

use candid::Nat;
use ic_cdk::{caller, id, storage, trap};
use ic_cdk::api::canister_balance;
use ic_cdk::api::call::{msg_cycles_accept, msg_cycles_available};
use ic_cdk::export::candid::CandidType;
use ic_cdk::export::Principal;
use ic_cdk_macros::{init, post_upgrade, pre_upgrade, query, update};
use ic_ledger_types::{AccountIdentifier, Subaccount};
use serde::{Deserialize, Serialize};

#[derive(CandidType, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
enum User {
    Principal(Principal),
    Account(AccountIdentifier),
    Null,
}

#[derive(CandidType, Clone, Serialize, Deserialize)]
struct Token {
    id: Principal,
    owner: User,
}

#[derive(CandidType, Clone, Default, Serialize, Deserialize)]
struct State {
    name: String,
    symbol: String,
    tokens: Vec<Token>,
    approved: HashMap<Principal, Vec<Principal>>,
    extensions: HashMap<Principal, Vec<String>>,
    custodians: Vec<Principal>,
    events: Option<Principal>,
}

trait FromBytes: Sized {
    fn from_be_bytes_(bytes: &[u8]) -> Option<Self>;
}

impl FromBytes for u32 {
    fn from_be_bytes_(bytes: &[u8]) -> Option<Self> {
        bytes.try_into().map(u32::from_be_bytes).ok()
    }
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

#[query]
fn balance_of(user: User) -> Nat {
    STATE.with(|s| Nat::from(s.borrow().tokens.iter().filter(|token| token.owner == user).count()))
}

#[query]
fn owner_of(token_id: Principal) -> Result<User, String> {
    STATE.with(|s| {
        let state = s.borrow();
        let token = state.tokens
            .iter()
            .find(|token| token.id == token_id)
            .ok_or("Token not found")?;
        Ok(token.owner)
    })
}

#[query]
fn name() -> String {
    STATE.with(|s| s.borrow().name.clone())
}

#[query]
fn symbol() -> String {
    STATE.with(|s| s.borrow().symbol.clone())
}

#[query]
fn total_supply() -> Nat {
    STATE.with(|s| Nat::from(s.borrow().tokens.len()))
}

#[update]
async fn approve(from_subaccount: Option<Subaccount>, approved: Principal, token_id: Principal, is_approved: bool) -> Result<(), String> {
    if approved == caller() {
        return Err("Caller cannot approve itself".to_string());
    }
    STATE.with(|s| {
        let mut state = s.borrow_mut();
        let token = state.tokens
            .iter()
            .find(|token| token.id == token_id)
            .ok_or("Token not found")?;
        let is_owner = match from_subaccount {
            Some(subaccount) => User::Account(AccountIdentifier::new(&caller(), &subaccount)),
            None => User::Principal(caller())
        } == token.owner;
        if !is_owner {
            return Err("Caller is not owner".to_string());
        }
        match is_approved {
            true => {
                match state.approved.get(&token_id) {
                    Some(a) => {
                        if a.contains(&approved) {
                            return Err("Already approved".to_string());
                        }
                        if a.len() == 256 {
                            return Err("Max approvals (256) has been reached".to_string());
                        }
                    }
                    None => {
                        state.approved.insert(token_id, vec![approved]);
                        return Ok(());
                    }
                }
                state.approved.get_mut(&token_id).unwrap().push(approved);
            }
            false => {
                let index = state.approved
                    .get(&token_id)
                    .and_then(|a| a.iter().position(|a| a == &approved))
                    .ok_or("Approved not found")?;
                state.approved.get_mut(&token_id).unwrap().remove(index);
            }
        }
        Ok(())
    })?;
    if let Some(events) = STATE.with(|s| s.borrow().events) {
        let _: Result<(), _> = ic_cdk::api::call::call(
            events,
            "approve_event",
            (from_subaccount, approved, token_id, is_approved, ),
        ).await;
    }
    Ok(())
}

#[query]
fn is_approved(token_id: Principal) -> bool {
    let spender = caller();
    STATE.with(|s| s.borrow().approved.get(&token_id).map_or(false, |approved| approved.contains(&spender)))
}

#[query]
fn get_approved(token_id: Principal) -> Vec<Principal> {
    STATE.with(|s| s.borrow().approved.get(&token_id).unwrap_or(&Vec::<Principal>::new()).clone())
}

#[update]
async fn transfer(from_subaccount: Option<Subaccount>, to: User, token_id: Principal) -> Result<(), String> {
    if to == User::Null {
        return Err("Cannot transfer to null".to_string());
    }
    let from = match from_subaccount {
        Some(subaccount) => User::Account(AccountIdentifier::new(&caller(), &subaccount)),
        None => User::Principal(caller())
    };
    STATE.with(|s| {
        let mut state = s.borrow_mut();
        let is_custodian = state.custodians.contains(&caller());
        let token = state.tokens
            .iter_mut()
            .find(|token| token.id == token_id)
            .ok_or("Token not found")?;
        let is_owner = from == token.owner;
        if !is_owner && !(token.owner == User::Null && is_custodian) {
            return Err("Caller is not owner or custodian".to_string());
        }
        token.owner = to.clone();
        Ok(())
    })?;
    STATE.with(|s| {
        s.borrow_mut().approved.remove(&token_id);
    });
    if let Some(events) = STATE.with(|s| s.borrow().events) {
        let _: Result<(), _> = ic_cdk::api::call::call(
            events,
            "transfer_event",
            (from, to, token_id, ),
        ).await;
    }
    Ok(())
}

#[update]
async fn transfer_from(from: User, to: User, token_id: Principal) -> Result<(), String> {
    if to == User::Null {
        return Err("Cannot transfer to null".to_string());
    }
    STATE.with(|s| {
        let mut state = s.borrow_mut();
        let is_approved = state.approved.get(&token_id).and_then(|approved| Some(approved.contains(&caller()))).unwrap_or(false);
        let token = state.tokens
            .iter_mut()
            .find(|token| token.id == token_id)
            .ok_or("Token not found")?;
        let is_owner = from == token.owner;
        if !is_owner {
            return Err("Token is not owned by user passed as from argument".to_string());
        }
        if !is_approved {
            return Err("Caller is not approved".to_string());
        }
        token.owner = to.clone();
        Ok(())
    })?;
    STATE.with(|s| {
        s.borrow_mut().approved.remove(&token_id);
    });
    if let Some(events) = STATE.with(|s| s.borrow().events) {
        let _: Result<(), _> = ic_cdk::api::call::call(
            events,
            "transfer_from_event",
            (caller(), from, to, token_id, ),
        ).await;
    }
    Ok(())
}

#[update]
async fn mint() -> Result<Token, String> {
    let token = STATE.with(|s| {
        let mut state = s.borrow_mut();
        if !state.custodians.contains(&caller()) {
            return Err("Caller is not a custodian".to_string());
        }
        let mint = state.tokens
            .get(state.tokens.len() - 1)
            .and_then(|token| {
                let id = token.id.as_slice().clone();
                u32::from_be_bytes_(&id[id.len() - 5..id.len() - 1]).and_then(|mint| Some(mint + 1))
            })
            .unwrap_or(0);
        if mint > 100000 {
            return Err("Max tokens (100000) has been reached".to_string());
        }
        let token = Token {
            id: Principal::from_slice([
                b"\x0Asld",
                id().as_slice(),
                mint.to_be_bytes().as_slice(),
                &[1] // Opaque identifier
            ].concat().as_slice()),
            owner: User::Null,
        };
        state.tokens.push(token.clone());
        Ok(token)
    })?;
    if let Some(events) = STATE.with(|s| s.borrow().events) {
        let _: Result<(), _> = ic_cdk::api::call::call(
            events,
            "mint_event",
            (token.id.clone(), ),
        ).await;
    }
    Ok(token)
}

#[update]
async fn burn(token_id: Principal) -> Result<(), String> {
    STATE.with(|s| {
        let index = s.borrow().tokens
            .iter()
            .position(|token| token.id == token_id)
            .ok_or("Token not found")?;
        let owner = s.borrow().tokens.get(index).unwrap().owner;
        let is_custodian = (owner == User::Null || owner == User::Principal(caller())) &&
            s.borrow().custodians.contains(&caller());
        if !is_custodian {
            return Err("Caller is not a custodian".to_string());
        }
        s.borrow_mut().tokens.remove(index);
        Ok(())
    })?;
    if let Some(events) = STATE.with(|s| s.borrow().events) {
        let _: Result<(), _> = ic_cdk::api::call::call(
            events,
            "burn_event",
            (token_id, ),
        ).await;
    }
    Ok(())
}

#[query]
fn tokens() -> Vec<Token> {
    STATE.with(|s| s.borrow().tokens.clone())
}

#[query]
fn tokens_of(owner: User) -> Vec<Token> {
    STATE.with(|s| s.borrow().tokens.iter().filter(|token| token.owner == owner).cloned().collect::<Vec<Token>>())
}

#[query]
fn extensions() -> HashMap<Principal, Vec<String>> {
    STATE.with(|s| s.borrow().extensions.clone())
}

#[update]
fn set_extension(principal: Principal, types: Vec<String>) -> Result<(), String> {
    STATE.with(|s| {
        let mut state = s.borrow_mut();
        if !state.custodians.contains(&caller()) {
            return Err("Caller is not a custodian".to_string());
        }
        match types.len() {
            0 => {
                state.extensions.remove(&principal);
            }
            l if l > 256 => {
                return Err("Max extension types (256) has been reached".to_string());
            }
            _ => {
                if state.extensions.keys().len() == 256 {
                    return Err("Max extensions (256) has been reached".to_string());
                }
                state.extensions.insert(principal, types);
            }
        }
        Ok(())
    })
}

#[query]
fn custodians() -> Vec<Principal> {
    STATE.with(|s| s.borrow().custodians.clone())
}

#[update]
fn set_custodian(custodian: Principal, is_custodian: bool) -> Result<(), String> {
    STATE.with(|s| {
        let mut state = s.borrow_mut();
        if !state.custodians.contains(&caller()) {
            return Err("Caller is not a custodian".to_string());
        }
        match is_custodian {
            true => {
                if state.custodians.contains(&custodian) {
                    return Err("Already a custodian".to_string());
                }
                if state.custodians.len() == 256 {
                    return Err("Max custodians (256) has been reached".to_string());
                }
                state.custodians.push(custodian);
            }
            false => {
                let index = state.custodians
                    .iter()
                    .position(|c| c == &custodian)
                    .ok_or("Custodian not found")?;
                state.custodians.remove(index);
            }
        }
        Ok(())
    })
}

#[query]
fn events() -> Option<Principal> {
    STATE.with(|s| s.borrow().events)
}

#[update]
fn set_events(events: Option<Principal>) {
    STATE.with(|s| s.borrow_mut().events = events)
}

#[query]
fn cycles() -> Nat {
    Nat::from(canister_balance())
}

#[update]
fn wallet_receive() {
    let amount = msg_cycles_available();
    if amount > 0 {
        msg_cycles_accept(amount);
    }
}

#[init]
fn init(name: String, symbol: String, custodian: Principal) {
    STATE.with(|s| {
        let mut state = s.borrow_mut();
        state.name = name;
        state.symbol = symbol;
        state.custodians = vec![custodian];
    });
}

#[pre_upgrade]
fn pre_upgrade() {
    STATE.with(|s| {
        if let Err(err) = storage::stable_save::<(&State, )>((&s.borrow(), )) {
            trap(&format!("An error occurred when saving to stable memory (pre_upgrade): {:?}", err));
        }
    });
}

#[post_upgrade]
fn post_upgrade() {
    STATE.with(|s| {
        match storage::stable_restore::<(State, )>() {
            Ok((state, )) => {
                s.replace(state);
            }
            Err(err) => trap(&format!("An error occurred when restoring from stable memory (post_upgrade): {:?}", err))
        }
    });
}

#[query(name = "__get_candid_interface_tmp_hack")]
fn export_candid() -> String {
    include_str!("sld721.did").to_string()
}

