use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use candid::Nat;
use ic_cdk::api::call::{ManualReply, msg_cycles_accept128, msg_cycles_available128};
use ic_cdk::api::canister_balance128;
use ic_cdk::caller;
use ic_cdk::export::candid::candid_method;
use ic_cdk::export::Principal;
use ic_cdk_macros::{init, post_upgrade, pre_upgrade, query, update};
use num_traits::ToPrimitive;

use crate::stable::{StableReader, StableWriter};
use crate::state::State;
use crate::types::{Account, ApproveArgs, ApproveError, BlockOrBucket, EventOrBucket, HistoryEntry, HttpRequest, HttpResponse, Offset, SetCustodianArgs, SetCustodiansError, SupportedStandard, TokenId, TransferArgs, TransferError, TransferFromArgs, TransferFromError, Value};

mod stable;
mod types;
mod state;
mod rc_bytes;

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

#[query(manual_reply = true)]
#[candid_method(query)]
fn sld1_metadata() -> ManualReply<HashMap<String, Value>> {
    STATE.with(|s| ManualReply::one(&s.borrow().metadata))
}

#[query(manual_reply = true)]
#[candid_method(query)]
fn sld1_name() -> ManualReply<String> {
    STATE.with(|s| ManualReply::one(&s.borrow().name))
}

#[query(manual_reply = true)]
#[candid_method(query)]
fn sld1_symbol() -> ManualReply<String> {
    STATE.with(|s| ManualReply::one(&s.borrow().symbol))
}

#[query]
#[candid_method(query)]
fn sld1_total_supply() -> Nat {
    STATE.with(|s| s.borrow().total_supply())
}

#[query]
#[candid_method(query)]
fn sld1_balance_of(account: Account) -> Nat {
    STATE.with(|s| s.borrow().balance_of(&account))
}

#[query]
#[candid_method(query)]
fn sld1_owner_of(token_id: TokenId) -> Option<Account> {
    STATE.with(|s| s.borrow().owner_of(&token_id))
}

#[query(manual_reply = true)]
#[candid_method(query)]
fn sld1_tokens(page: Nat) -> ManualReply<Vec<TokenId>> {
    STATE.with(|s| ManualReply::one(&s.borrow().tokens(&page)))
}

#[query(manual_reply = true)]
#[candid_method(query)]
fn sld1_tokens_of(account: Account, page: Nat) -> ManualReply<Vec<TokenId>> {
    STATE.with(|s| ManualReply::one(s.borrow().tokens_of(&account, &page)))
}

#[update]
#[candid_method(update)]
fn sld1_transfer(args: TransferArgs) -> Result<Nat, TransferError> {
    STATE.with(|s| s.borrow_mut().transfer_from(
        TransferFromArgs {
            from: Account::new(caller(), args.from_subaccount),
            to: Account::new(args.to.owner, args.to.subaccount),
            token_id: args.token_id,
            memo: args.memo,
            created_at_time: args.created_at_time,
        }
    ).map_err(|err| err.to_transfer_error()))
}

#[query]
#[candid_method(query)]
fn sld1_supported_standards() -> [SupportedStandard; 4] {
    [
        SupportedStandard {
            name: "SLD-1".into(),
            url: "https://github.com/slide-computer/slide-token".into(),
        },
        SupportedStandard {
            name: "SLD-2".into(),
            url: "https://github.com/slide-computer/slide-token".into(),
        },
        SupportedStandard {
            name: "SLD-3".into(),
            url: "https://github.com/slide-computer/slide-token".into(),
        },
        SupportedStandard {
            name: "SLD-4".into(),
            url: "https://github.com/slide-computer/slide-token".into(),
        }
    ]
}

#[update]
#[candid_method(update)]
fn sld2_approve(args: ApproveArgs) -> Result<Nat, ApproveError> {
    STATE.with(|s| s.borrow_mut().approve(args))
}

#[update]
#[candid_method(update)]
fn sld2_transfer_from(args: TransferFromArgs) -> Result<Nat, TransferFromError> {
    STATE.with(|s| s.borrow_mut().transfer_from(
        TransferFromArgs {
            from: Account::new(args.from.owner, args.from.subaccount),
            to: Account::new(args.to.owner, args.to.subaccount),
            token_id: args.token_id,
            memo: args.memo,
            created_at_time: args.created_at_time,
        }
    ))
}

#[query(manual_reply = true)]
#[candid_method(query)]
fn sld2_get_approved(token_id: TokenId) -> ManualReply<HashSet<Principal>> {
    STATE.with(|s| ManualReply::one(s.borrow().get_approved(&token_id)))
}

#[query]
#[candid_method(query)]
fn sld3_get_tx(tx_id: Nat) -> Option<EventOrBucket> {
    STATE.with(|s| s.borrow().read_tx(tx_id))
}

#[query]
#[candid_method(query)]
fn sld3_get_block(block_id: Nat) -> Option<BlockOrBucket> {
    STATE.with(|s| s.borrow().read_block(block_id))
}

#[query]
#[candid_method(query)]
fn sld3_block_size() -> Nat {
    // TODO: implement
    Nat::from(0)
}

#[query(manual_reply = true)]
#[candid_method(query)]
fn sld3_tx_total() -> ManualReply<Nat> {
    STATE.with(|s| ManualReply::one(&s.borrow().tx_total))
}

#[query(manual_reply = true)]
#[candid_method(query)]
fn sld4_get_custodians() -> ManualReply<Vec<Principal>> {
    STATE.with(|s| ManualReply::one(&s.borrow().custodians))
}

#[update]
#[candid_method(update)]
fn sld4_set_custodian(args: SetCustodianArgs) -> Result<Nat, SetCustodiansError> {
    STATE.with(|s| s.borrow_mut().set_custodian(args))
}


// #[query]
// #[candid_method(query)]
// fn http_request(req: HttpRequest) -> HttpResponse {
//     STATE.with(|s| s.borrow().http_request(req))
// }

#[query]
#[candid_method(query)]
fn cycles() -> Nat {
    Nat::from(canister_balance128())
}

#[update]
#[candid_method(update)]
fn wallet_receive() {
    let amount = msg_cycles_available128();
    if amount > 0 {
        msg_cycles_accept128(amount);
    }
}

#[init]
#[candid_method(init)]
fn init(name: String, symbol: String, custodian: Principal) {
    STATE.with(|s| s.borrow_mut().init(name, symbol, custodian));
}

#[pre_upgrade]
fn pre_upgrade() {
    // STATE.with(|s| {
    //     // Transform state and hash tree into stable state
    //     let offset = HISTORY_COUNT_BYTES + history_count() as usize * HISTORY_ENTRY_BYTES;
    //     let state = s.take();
    //     let mut stable_state = StableState {
    //         name: state.name,
    //         symbol: state.symbol,
    //         tokens: state.tokens,
    //         approved: state.approved,
    //         extensions: state.extensions,
    //         custodians: state.custodians,
    //         offset_hashes: HashMap::default(),
    //         history_hash: state.hash_tree.get(b"history").map(|history_hash| history_hash.clone()),
    //     };
    //     for token in &stable_state.tokens {
    //         stable_state.offset_hashes.insert(token.offset, state.hash_tree.get(token.offset.to_string().as_bytes()).unwrap().clone());
    //     }
    //     if let Err(err) = stable_save::<(&StableState, )>((&stable_state, ), offset as usize) {
    //         trap(&format!("An error occurred when saving to stable memory (pre_upgrade): {:?}", err));
    //     }
    // });
}

#[post_upgrade]
fn post_upgrade() {
    // STATE.with(|s| {
    //     let offset = HISTORY_COUNT_BYTES + history_count() as usize * HISTORY_ENTRY_BYTES;
    //     match stable_restore::<(StableState, )>(offset as usize) {
    //         Ok((stable_state, )) => {
    //             // Reconstruct state with hash tree from stable state
    //             let mut state = State {
    //                 name: stable_state.name,
    //                 symbol: stable_state.symbol,
    //                 tokens: stable_state.tokens,
    //                 approved: stable_state.approved,
    //                 extensions: stable_state.extensions,
    //                 custodians: stable_state.custodians,
    //                 hash_tree: RbTree::default(),
    //             };
    //             if let Some(history_hash) = stable_state.history_hash {
    //                 state.hash_tree.insert("history".into(), history_hash);
    //             }
    //             for token in &state.tokens {
    //                 state.hash_tree.insert(token.offset.to_string(), stable_state.offset_hashes.get(&token.offset).unwrap().clone());
    //             }
    //             s.replace(state);
    //         }
    //         Err(err) => trap(&format!("An error occurred when restoring from stable memory (post_upgrade): {:?}", err))
    //     }
    // });
}

#[query(name = "__get_candid_interface_tmp_hack")]
fn export_candid() -> String {
    __export_service()
}

candid::export_service!();

