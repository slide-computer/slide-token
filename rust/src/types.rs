use std::collections::{HashMap, HashSet};
use std::convert::TryInto;
use std::fmt;
use std::fmt::Write;

use candid::{Func, Int, Nat, Principal};
use ic_cdk::export::candid::CandidType;
use ic_cdk::id;
use num_traits::ToPrimitive;
use serde::{Deserialize, Serialize, Serializer};
use serde_bytes::ByteBuf;

use crate::rc_bytes::RcBytes;

pub type TokenId = Nat;

#[derive(Clone, Debug, CandidType, Deserialize)]
pub enum Value {
    Nat(Nat),
    Int(Int),
    Text(String),
    Blob(Vec<u8>),
}

/// Subaccount is an arbitrary 32-byte byte array.
#[derive(
CandidType, Serialize, Deserialize, Clone, Copy, Hash, Debug, PartialEq, Eq, PartialOrd, Ord,
)]
pub struct Subaccount(pub [u8; 32]);

/// Account follow ICRC-1 standard
#[derive(
CandidType, Deserialize, Clone, Copy, Hash, Debug, Eq, PartialOrd, Ord,
)]
pub struct Account {
    pub owner: Principal,
    pub subaccount: Option<Subaccount>,
}

/// Subaccount that is used by default.
pub const DEFAULT_SUBACCOUNT: Subaccount = Subaccount([0; 32]);

#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct TransferArgs {
    pub from_subaccount: Option<Subaccount>,
    pub to: Account,
    pub token_id: TokenId,
    pub memo: Option<[u8; 32]>,
    pub created_at_time: Option<u64>,
}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct GenericError {
    pub error_code: Nat,
    pub message: String,
}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub enum TransferError {
    NotFound,
    NotOwner,
    NotSelf,
    TemporarilyUnavailable,
    GenericError(GenericError),
}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct SupportedStandard {
    pub name: String,
    pub url: String,
}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct ApproveArgs {
    pub from_subaccount: Option<Subaccount>,
    pub spender: Principal,
    pub token_id: TokenId,
    pub approved: bool,
    pub memo: Option<[u8; 32]>,
    pub created_at_time: Option<u64>,
}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub enum ApproveError {
    NotFound,
    NotOwner,
    NotSelf,
    MaxApprovals(Nat),
    TemporarilyUnavailable,
    GenericError(GenericError),
}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct TransferFromArgs {
    pub from: Account,
    pub to: Account,
    pub token_id: TokenId,
    pub memo: Option<[u8; 32]>,
    pub created_at_time: Option<u64>,
}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub enum TransferFromError {
    NotFound,
    NotOwner,
    NotSelf,
    NotApproved,
    TemporarilyUnavailable,
    GenericError(GenericError),
}

impl TransferFromError {
    pub fn to_transfer_error(&self) -> TransferError {
        match self {
            TransferFromError::NotFound => TransferError::NotFound,
            TransferFromError::NotOwner => TransferError::NotOwner,
            TransferFromError::NotSelf => TransferError::NotSelf,
            TransferFromError::NotApproved => TransferError::GenericError(GenericError {
                error_code: Nat::from(403),
                message: "Caller is not approved".into(),
            }),
            TransferFromError::TemporarilyUnavailable => TransferError::TemporarilyUnavailable,
            TransferFromError::GenericError(generic_error) => TransferError::GenericError(generic_error.clone())
        }
    }
}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct Event {
    pub caller: Principal,
    pub operation: String,
    pub time: u64,
    pub details: HashMap<String, Value>,
}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub enum EventOrBucket {
    Event(Event),
    Bucket(Principal)
}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub enum BlockOrBucket {
    Block(Vec<Event>),
    Bucket(Principal)
}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct SetCustodianArgs {
    pub custodian: Principal,
    pub approved: bool,
}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub enum SetCustodiansError {
    NotAllowed,
    MaxCustodians(Nat),
    TemporarilyUnavailable,
    GenericError(GenericError),
}

/// Internal Token state
#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct Token {
    pub account: Account,
    pub tx_id: Nat,
    pub approved: HashSet<Principal>,
}

pub type Offset = Nat;
pub type Time = u64;

/// History entry has a token id, account, time and offset of previous entry for this token id.
///
/// A token is (pre-)minted when previous offset is equal to current offset or if previous
/// history entry was a burn. It's either a mint or pre-mint depending on the receiving
/// account, it's a pre-mint if the account is the minter account. In case of a pre-mint,
/// the next transfer to an account other than the minter account is considered a mint.
///
/// A burn occurs when the previous offset is not equal to current offset
/// and the current account is equal to the minter account.
///
/// The minter account is always the canister principal.
#[derive(CandidType, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub token_id: TokenId,
    pub account: Account,
    pub time: Time,
    pub from_offset: Offset,
}

// #[derive(Clone, Debug, CandidType, Deserialize)]
// pub struct MintArgs {
//     pub to: Account,
//     pub token_id: TokenId,
// }
impl Account {
    /// Account with default subaccount should always default to account without subaccount
    pub fn new(owner: Principal, subaccount: Option<Subaccount>) -> Self {
        Account {
            owner,
            subaccount: subaccount.map(|s| if s == DEFAULT_SUBACCOUNT { None } else { Some(s) }).flatten(),
        }
    }

    /// Minter account is equal to canister principal
    pub fn minter() -> Self {
        Account {
            owner: id(),
            subaccount: None,
        }
    }
}

/// Account should be ICRC-1 textual encoding when serialized to JSON
impl Serialize for Account {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

/// ICRC-1 textual encoding
impl fmt::Display for Account {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut bytes: Vec<u8> = Vec::default();
        bytes.extend_from_slice(self.owner.as_slice());

        // Add subaccount bytes without leading zero, the length
        // and the ICRC-1 non-default subaccount identifier
        if let Some(subaccount) = self.subaccount {
            let mut count: u8 = 0;
            for byte in subaccount.0 {
                if byte != 0 || count > 0 {
                    count += 1;
                    bytes.push(byte);
                }
            }
            if count > 0 {
                bytes.push(count);
                bytes.push(127);
            }
        }

        // calc checksum
        let checksum = crc32fast::hash(&bytes);

        // add checksum to bytes
        bytes.splice(0..0, checksum.to_be_bytes());

        // base32
        let mut s = data_encoding::BASE32_NOPAD.encode(&bytes);
        s.make_ascii_lowercase();

        // write out string with dashes
        let mut s = s.as_str();
        while s.len() > 5 {
            f.write_str(&s[..5])?;
            f.write_char('-')?;
            s = &s[5..];
        }
        f.write_str(s)
    }
}

/// Implement custom equality check so that accounts without a subaccount
/// are equal to accounts with the default subaccount as per ICRC-1 spec.
impl PartialEq for Account {
    fn eq(&self, other: &Self) -> bool {
        self.owner == other.owner && self.subaccount.unwrap_or(DEFAULT_SUBACCOUNT) == other.subaccount.unwrap_or(DEFAULT_SUBACCOUNT)
    }
}

pub trait MintIndex {
    fn from_mint_index(mint: u32) -> Self;
    fn to_mint_index(&self) -> u32;
}

/// Convert between SLD token id and mint index
impl MintIndex for Principal {
    fn from_mint_index(mint: u32) -> Self {
        Principal::from_slice([
            b"\x0Asld",
            id().as_slice(),
            mint.to_be_bytes().as_slice(),
            &[1] // Opaque identifier
        ].concat().as_slice())
    }

    fn to_mint_index(&self) -> u32 {
        let bytes = self.as_slice();
        let mint_bytes = &bytes[bytes.len() - 5..bytes.len() - 1];
        u32::from_be_bytes(mint_bytes.try_into().unwrap())
    }
}

pub trait StableBytes<T> {
    fn from_stable_bytes(bytes: &T) -> Self;
    fn to_stable_bytes(&self, buf: &mut T);
}


/// Fixed size of history entry in stable memory
pub const HISTORY_ENTRY_BYTES: usize = 78;

/// Convert history entry into fixed length bytes for storage in stable memory
impl StableBytes<[u8; HISTORY_ENTRY_BYTES]> for HistoryEntry {
    fn from_stable_bytes(bytes: &[u8; HISTORY_ENTRY_BYTES]) -> Self {
        let length = bytes[4] as usize;
        let subaccount = Subaccount((&bytes[34..66]).try_into().unwrap());
        HistoryEntry {
            token_id: TokenId::from(u32::from_le_bytes((&bytes[0..4]).try_into().unwrap())),
            account: Account {
                owner: Principal::from_slice((&bytes[5..5 + length]).try_into().unwrap()),
                subaccount: if subaccount == DEFAULT_SUBACCOUNT { None } else { Some(subaccount) },
            },
            time: Time::from(u64::from_le_bytes((&bytes[66..74]).try_into().unwrap())),
            from_offset: Offset::from(u32::from_le_bytes((&bytes[74..78]).try_into().unwrap())),
        }
    }

    fn to_stable_bytes(&self, buf: &mut [u8; HISTORY_ENTRY_BYTES]) {
        // buf[0..4].copy_from_slice(&self.token_id.0.to_u32().unwrap().to_le_bytes());
        // let length = self.account.owner.as_slice().len();
        // buf[4] = length as u8;
        // buf[5..5 + length].copy_from_slice(&self.account.owner.as_slice());
        // buf[34..66].copy_from_slice(&self.account.subaccount.unwrap_or(DEFAULT_SUBACCOUNT).0);
        // buf[66..74].copy_from_slice(&self.time.0.to_u64().unwrap().to_le_bytes());
        // buf[74..78].copy_from_slice(&self.from_offset.0.to_u32().unwrap().to_le_bytes());
    }
}

// HTTP interface

pub type HeaderField = (String, String);

#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct HttpRequest {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: ByteBuf,
}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct HttpResponse {
    pub status_code: u16,
    pub headers: Vec<HeaderField>,
    pub body: RcBytes,
    pub streaming_strategy: Option<StreamingStrategy>,
}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub enum StreamingStrategy {
    Callback {
        callback: Func,
        token: StreamingCallbackToken,
    },
}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct StreamingCallbackToken {
    pub key: String,
    pub content_encoding: String,
    pub index: Nat,
    // We don't care about the sha, we just want to be backward compatible.
    pub sha256: Option<ByteBuf>,
}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct StreamingCallbackHttpResponse {
    pub body: RcBytes,
    pub token: Option<StreamingCallbackToken>,
}