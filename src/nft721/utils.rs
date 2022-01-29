use std::io::Write;

use ic_kit::ic;
use sha2::{Digest, Sha224};
use uuid::Uuid;

use crate::{ErrorCode, Principal, State, User};
use crate::ErrorCode::{ManagementCanisterUnreachable, UuidInvalidByteLength};

static ACCOUNT_DOMAIN_SEPARATOR: &[u8] = b"\x0Aaccount-id";

pub fn state<'a>() -> &'a mut State {
    ic::get_mut::<State>()
}

pub async fn uuid_v4() -> Result<String, ErrorCode> {
    let result: Result<(Vec<u8>, ), _> = ic_cdk::api::call::call(
        ic_cdk::export::Principal::management_canister(),
        "raw_rand",
        (),
    ).await;
    let bytes = result.map(|v| v.0).map_err(|_| ManagementCanisterUnreachable)?;
    Uuid::from_slice(&bytes[..16]).map(|uuid| uuid.to_string()).map_err(|_| UuidInvalidByteLength)
}

pub fn caller_to_user(caller: Principal, sub_account: Option<[u8; 32]>) -> User {
    match sub_account {
        Some(s) => User::Address(create_address(caller, s)),
        None => User::Principal(caller)
    }
}

pub fn create_address(principal: Principal, sub_account: [u8; 32]) -> String {
    let mut sha224 = Sha224::new();
    let _ = sha224.write(ACCOUNT_DOMAIN_SEPARATOR);
    let _ = sha224.write(principal.as_slice());
    let _ = sha224.write(&sub_account);
    let hash = sha224.result();
    let mut crc32 = crc32fast::Hasher::new();
    crc32.update(&hash[..]);
    hex::encode([&crc32.finalize().to_be_bytes()[..], &hash[..]].concat())
}
