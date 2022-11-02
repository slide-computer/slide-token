use std::collections::{HashMap, HashSet};
use std::convert::TryFrom;

use candid::Nat;
use ic_cdk::{caller, id, print, trap};
use ic_cdk::api::{canister_balance, data_certificate, set_certified_data, time};
use ic_cdk::api::stable::stable_size;
use ic_cdk::export::candid::CandidType;
use ic_cdk::export::Principal;
use ic_certified_map::{AsHashTree, Hash, labeled, labeled_hash, RbTree};
use num_traits::ToPrimitive;
use serde::{Deserialize, Serialize};
use serde_bytes::ByteBuf;
use sha2::Digest;

use crate::{ApproveArgs, ApproveError, HistoryEntry, HttpRequest, HttpResponse, SetCustodianArgs, SetCustodiansError, StableReader, StableWriter, TransferArgs, TransferFromArgs, TransferFromError};
use crate::rc_bytes::RcBytes;
use crate::types::{Account, BlockOrBucket, Event, EventOrBucket, GenericError, HISTORY_ENTRY_BYTES, Offset, StableBytes, SupportedStandard, Time, Token, TokenId, Value};

// pub const HISTORY_PAGE_SIZE: usize = 5;

#[derive(Default)]
pub struct State {
    pub metadata: HashMap<String, Value>,
    pub name: String,
    pub symbol: String,
    pub tokens: HashMap<TokenId, Token>,
    pub current_block: Vec<Event>,
    pub block_indexes: Vec<usize>,
    pub tx_total: Nat,
    pub custodians_tx: Nat,
    pub custodians: HashSet<Principal>,
    pub hash_tree: RbTree<String, Hash>,
}

// #[derive(CandidType, Clone, Default, Deserialize)]
// pub struct StableState {
//     pub name: String,
//     pub symbol: String,
//     // pub tokens: Vec<Token>,
//     pub approved: HashMap<Principal, Vec<Principal>>,
//     pub extensions: HashMap<Principal, Vec<String>>,
//     pub custodians: Vec<Principal>,
//     pub offset_hashes: HashMap<u32, Hash>,
//     pub history_hash: Option<Hash>,
// }

impl State {
    pub fn init(&mut self, name: String, symbol: String, custodian: Principal) {
        self.name = name;
        self.symbol = symbol;
        self.custodians = HashSet::from([custodian]);

        // Call custodian setter so history entry is written, setter will not an throw error
        // since custodian has already been added to the approved list above
        self.set_custodian(SetCustodianArgs {
            custodian,
            approved: true,
        }).unwrap();

        // Set initial http certified data
        // self.hash_tree.insert("/name".into(), sha2::Sha256::digest(format!("\"{}\"", self.name).as_bytes()).into());
        // self.hash_tree.insert("/symbol".into(), sha2::Sha256::digest(format!("\"{}\"", self.symbol).as_bytes()).into());
        // self.hash_tree.insert("/total_supply".into(), sha2::Sha256::digest("0".as_bytes()).into());
        // self.hash_tree.insert("/tokens".into(), sha2::Sha256::digest("[]".as_bytes()).into());
        // self.hash_tree.insert("/history/0".into(), sha2::Sha256::digest("[]".as_bytes()).into());
        // self.hash_tree.insert("/history_total".into(), sha2::Sha256::digest("0".as_bytes()).into());
        // set_certified_data(&labeled_hash(b"http_assets", &self.hash_tree.root_hash()));
    }

    pub fn total_supply(&self) -> Nat {
        let minter_account = Account::minter();
        Nat::from(self.tokens.iter().filter(|(_, token)| token.account != minter_account).count())
    }

    pub fn balance_of(&self, account: &Account) -> Nat {
        Nat::from(self.tokens.iter().filter(|(_, token)| token.account == *account).count())
    }

    pub fn owner_of(&self, token_id: &TokenId) -> Option<Account> {
        self.tokens.get(token_id).map(|token| token.account)
    }

    pub fn tokens(&self, page: &Nat) -> Vec<&TokenId> {
        let minter_account = Account::minter();
        page.0.to_usize().map_or(vec![], |page| self.tokens
            .iter()
            .filter(|(_, token)| token.account != minter_account)
            .skip(page * 100_000)
            .take(100_000)
            .map(|(token_id, _)| token_id)
            .collect())
    }

    pub fn tokens_of(&self, account: &Account, page: &Nat) -> Vec<&TokenId> {
        page.0.to_usize().map_or(vec![], |page| self.tokens
            .iter()
            .filter(|(_, token)| token.account == *account)
            .skip(page * 100_000)
            .take(100_000)
            .map(|(token_id, _)| token_id)
            .collect())
    }

    pub fn approve(&mut self, args: ApproveArgs) -> Result<Nat, ApproveError> {
        let caller = caller();
        if args.spender == caller {
            return Err(ApproveError::NotSelf);
        }
        let mut token = self.tokens.get(&args.token_id).map(|token| token.clone()).ok_or(ApproveError::NotFound)?;
        let from = Account::new(caller, args.from_subaccount);
        let is_owner = from == token.account;
        if !is_owner {
            return Err(ApproveError::NotOwner);
        }
        match args.approved {
            true => {
                if token.approved.len() == 256 {
                    return Err(ApproveError::MaxApprovals(Nat::from(256)));
                }
                token.approved.insert(args.spender);
            }
            false => {
                token.approved.remove(&args.spender);
            }
        }

        let mut event = Event {
            caller,
            operation: "sld2:approve".into(),
            time: time(),
            details: HashMap::from([
                ("token_id".into(), Value::Nat(args.token_id.clone())),
                ("spender".into(), Value::Text(args.spender.to_string())),
                ("approved".into(), Value::Nat(Nat::from(if args.approved { 1 } else { 0 }))),
                ("from_tx".into(), Value::Nat(token.tx_id.clone())),
            ]),
        };
        if let Some(memo) = args.memo {
            event.details.insert("memo".into(), Value::Blob(Vec::from(memo)));
        }
        if let Some(created_at_time) = args.created_at_time {
            event.details.insert("time".into(), Value::Nat(Nat::from(created_at_time)));
        }
        self.write_tx(event);
        let tx_id: Nat = self.tx_total.clone() - 1;
        token.tx_id = tx_id.clone();
        self.tokens.insert(args.token_id, token);

        Ok(tx_id)
    }

    pub fn transfer_from(&mut self, args: TransferFromArgs) -> Result<Nat, TransferFromError> {
        let minter_account = Account::minter();
        let caller = caller();
        let caller_is_custodian = self.custodians.contains(&caller);
        let transfer_is_burn = args.to == minter_account;
        let mut transfer_is_mint = false;
        let mut token = self.tokens.get(&args.token_id).map_or_else(|| {
            if !caller_is_custodian {
                return Err(TransferFromError::NotFound);
            }
            transfer_is_mint = true;
            Ok(Token {
                account: minter_account,
                tx_id: Offset::from(0),
                approved: HashSet::default(),
            })
        }, |token| Ok(token.clone()))?;
        let caller_is_from = args.from.owner == caller;
        let from_is_owner = token.account == args.from || (caller_is_custodian && token.account == minter_account);
        let caller_is_approved = token.approved.contains(&caller);

        if !from_is_owner {
            return Err(TransferFromError::NotOwner);
        }

        if transfer_is_burn && !caller_is_custodian {
            return Err(TransferFromError::GenericError(GenericError {
                error_code: Nat::from(403),
                message: "Caller is not custodian".into(),
            }));
        }

        if !caller_is_from && !caller_is_approved {
            return Err(TransferFromError::NotApproved);
        }

        if args.to == token.account {
            return Err(TransferFromError::NotSelf);
        }

        token.account = args.to.clone();
        let mut event = Event {
            caller,
            operation: (
                if transfer_is_mint {
                    "sld1:mint"
                } else if transfer_is_burn {
                    "sld1:burn"
                } else if caller_is_from {
                    "sld1:transfer"
                } else {
                    "sld2:transfer_from"
                }
            ).into(),
            time: time(),
            details: HashMap::from([
                ("token_id".into(), Value::Nat(args.token_id.clone())),
                ("time".into(), Value::Nat(Nat::from(time()))),
                ("from_tx".into(), Value::Nat(token.tx_id.clone())),
            ]),
        };
        if let Some(memo) = args.memo {
            event.details.insert("memo".into(), Value::Blob(Vec::from(memo)));
        }
        if let Some(created_at_time) = args.created_at_time {
            event.details.insert("time".into(), Value::Nat(Nat::from(created_at_time)));
        }
        self.write_tx(event);
        token.tx_id = self.tx_total.clone() - 1;
        token.approved = HashSet::default();
        self.tokens.insert(args.token_id.clone(), token);

        // Update http certified data
        // self.hash_tree.insert(format!("/token/{}", token.id.to_string()), sha2::Sha256::digest(serde_json::to_vec(&token).unwrap().as_slice()).into());
        // self.hash_tree.insert("/tokens".into(), sha2::Sha256::digest(serde_json::to_vec(&self.tokens).unwrap().as_slice()).into());
        // self.hash_tree.insert("/total_supply".into(), sha2::Sha256::digest(self.total_supply().to_string().as_bytes()).into());
        // let page = if self.history_total as usize % HISTORY_PAGE_SIZE == 0 {
        //     (self.history_total as usize / HISTORY_PAGE_SIZE) - 1
        // } else {
        //     self.history_total as usize / HISTORY_PAGE_SIZE
        // };
        // self.hash_tree.insert(format!("/history/{}", page), sha2::Sha256::digest(serde_json::to_vec(&self.read_history(page)).unwrap().as_slice()).into());
        // self.hash_tree.insert("/history_total".into(), sha2::Sha256::digest(self.history_total.to_string().as_bytes()).into());
        // set_certified_data(&labeled_hash(b"http_assets", &self.hash_tree.root_hash()));

        Ok(self.tx_total.clone() - 1)
    }

    pub fn get_approved(&self, token_id: &TokenId) -> HashSet<&Principal> {
        self.tokens.get(token_id).map_or(HashSet::default(), |token| token.approved.iter().collect())
    }

    pub fn set_custodian(&mut self, args: SetCustodianArgs) -> Result<Nat, SetCustodiansError> {
        let caller = caller();
        if !self.custodians.contains(&caller) {
            return Err(SetCustodiansError::NotAllowed);
        }
        match args.approved {
            true => {
                if self.custodians.len() == 256 {
                    return Err(SetCustodiansError::MaxCustodians(Nat::from(256)));
                }
                self.custodians.insert(args.custodian);
            }
            false => {
                self.custodians.remove(&args.custodian);
            }
        }

        let event = Event {
            caller,
            operation: "sld4:set_custodian".into(),
            time: time(),
            details: HashMap::from([
                ("custodian".into(), Value::Text(args.custodian.to_string())),
                ("approved".into(), Value::Nat(Nat::from(if args.approved { 1 } else { 0 }))),
                ("from_tx".into(), Value::Nat(self.custodians_tx.clone())),
            ]),
        };
        self.write_tx(event);

        self.custodians_tx = self.tx_total.clone() - 1;

        Ok(self.tx_total.clone() - 1)
    }

    pub fn write_tx(&mut self, event: Event) {
        self.current_block.push(event);
        self.tx_total += 1;
        // TODO: Check if block size has been reached, write to stable memory when reached


        // let entry = HistoryEntry {
        //     token_id: token_id.clone(),
        //     account: token.account,
        //     time: Time::from(time()),
        //     // Refer to self if offset is 0 (mint)
        //     from_offset: if token.tx_id == 0 {
        //         Offset::from(self.tx_total)
        //     } else {
        //         token.tx_id.clone()
        //     },
        // };
        // self.history_entries.push(entry);
        // self.tx_total += 1;
        //
        // if self.history_entries.len() == HISTORY_PAGE_SIZE {
        //     // Create stable memory writer instance
        //     let mut writer = StableWriter {
        //         offset: 0,
        //         capacity: stable_size(),
        //     };
        //
        //     // Create bytes for all entries
        //     let mut bytes = [0u8; HISTORY_ENTRY_BYTES * HISTORY_PAGE_SIZE];
        //     for i in 0..HISTORY_PAGE_SIZE {
        //         self.history_entries[i].to_stable_bytes(<&mut [u8; HISTORY_ENTRY_BYTES]>::try_from(&mut bytes[i * HISTORY_ENTRY_BYTES..(i + 1) * HISTORY_ENTRY_BYTES]).unwrap());
        //     }
        //
        //     // Write to stable memory
        //     writer.offset = (self.tx_total as usize - HISTORY_PAGE_SIZE) * HISTORY_ENTRY_BYTES;
        //     writer.write(&bytes).map_err(|_| "An error occurred when writing to stable memory").unwrap();
        //
        //     // Clear current history
        //     self.history_entries = vec![];
        // }
    }

    pub fn read_tx(&self, tx_id: Nat) -> Option<EventOrBucket> {
        tx_id.0
            .to_usize()
            .map(|index| self.current_block
                .get(index)
                .map(|event| EventOrBucket::Event(event.clone()))
            )
            .flatten()
        // TODO: read from from stable memory if not in current block
    }

    pub fn read_block(&self, block_id: Nat) -> Option<BlockOrBucket> {
        block_id.0
            .to_usize()
            .map(|_| BlockOrBucket::Block(self.current_block.clone()))
        // TODO: read from from stable memory if not current block
    }

    // pub fn read_history(&self, page: usize) -> Vec<HistoryEntry> {
    //     if (page + 1) * HISTORY_PAGE_SIZE > self.tx_total as usize {
    //         // Return current history if page is not out of bounds else return empty history
    //         if page * HISTORY_PAGE_SIZE < self.tx_total as usize {
    //             self.history_entries.clone()
    //         } else {
    //             vec![]
    //         }
    //     } else {
    //         // Create stable memory reader instance
    //         let mut reader = StableReader {
    //             offset: page as usize * HISTORY_PAGE_SIZE * HISTORY_ENTRY_BYTES,
    //             capacity: stable_size(),
    //         };
    //
    //         // Read all bytes of entries range, create entries from bytes and return them with total count
    //         let mut buf = vec![0u8; HISTORY_PAGE_SIZE * HISTORY_ENTRY_BYTES];
    //         reader.read(buf.as_mut_slice()).map_err(|_| "An error occurred when reading from stable memory").unwrap();
    //         let mut entries: Vec<HistoryEntry> = Vec::with_capacity(HISTORY_PAGE_SIZE);
    //         for i in 0..HISTORY_PAGE_SIZE {
    //             let entry_offset = i * HISTORY_ENTRY_BYTES;
    //             let bytes = &buf[entry_offset..entry_offset + HISTORY_ENTRY_BYTES];
    //             entries.push(HistoryEntry::from_stable_bytes(<&[u8; HISTORY_ENTRY_BYTES]>::try_from(bytes).unwrap()));
    //         }
    //         entries
    //     }
    // }

    // pub fn http_request(&self, req: HttpRequest) -> HttpResponse {
    //     // Create certification header, should always be returned for every request including
    //     // requests that return 404 not found. Else the receiving client does not know if the 404
    //     // not found is sent by the canister or a malicious middle man.
    //     let certificate = data_certificate().unwrap_or_else(|| trap("No data certificate available"));
    //     let hash_tree = labeled(b"http_assets", self.hash_tree.witness(req.url.as_bytes()));
    //     let mut serializer = serde_cbor::ser::Serializer::new(vec![]);
    //     serializer.self_describe().unwrap();
    //     hash_tree.serialize(&mut serializer).unwrap();
    //     let headers = vec![
    //         ("Content-Type".to_string(), "application/json".to_string()),
    //         (
    //             "IC-Certificate".to_string(),
    //             String::from("certificate=:")
    //                 + &base64::encode(certificate)
    //                 + ":, tree=:"
    //                 + &base64::encode(&serializer.into_inner())
    //                 + ":",
    //         ),
    //     ];
    //
    //     // No certified hash found for request url and method, return 404 response with absence proof
    //     if self.hash_tree.get(req.url.as_bytes()).is_none() || req.method != "GET" {
    //         return HttpResponse {
    //             status_code: 404,
    //             headers,
    //             body: RcBytes::default(),
    //             streaming_strategy: None,
    //         };
    //     }
    //
    //     // Certified hash was found for request, from here on we can assume
    //     // the url is valid and simply unwrap everything without checks.
    //     let mut parts = req.url.split("/").skip(1).fuse();
    //     let body = RcBytes::from(ByteBuf::from(match parts.next() {
    //         Some("name") => format!("\"{}\"", self.name),
    //         Some("symbol") => format!("\"{}\"", self.symbol),
    //         Some("total_supply") => self.total_supply().to_string(),
    //         Some("token") => {
    //             "hmm".to_string()
    //             // let token_id = Principal::from_text(parts.next().unwrap()).unwrap();
    //             // let token = self.token(token_id).unwrap();
    //             // serde_json::to_string(token).unwrap()
    //         }
    //         Some("tokens") => serde_json::to_string(&self.tokens).unwrap(),
    //         Some("history") => serde_json::to_string(&self.read_history(parts.next().unwrap().parse::<usize>().unwrap())).unwrap(),
    //         Some("history_total") => self.tx_total.to_string(),
    //         _ => unreachable!()
    //     }));
    //
    //     HttpResponse {
    //         status_code: 200,
    //         headers,
    //         body,
    //         streaming_strategy: None,
    //     }
    // }
}