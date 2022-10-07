# SLD721 interface specification

SLD721 is a non-fungible token standard based on ERC-721 adapted for the Internet Computer. The purpose of this standard is to only implement the minimal required funcionality of a ledger e.g. transfer, mint, burn and approval. Other funcionality like returning media assets that represent an NFT token is out of the scope of this standard and should be implemented in other canisters. A SLD721 token can refer to other canisters by setting extensions. 

Event history tracking is not part of the NFT standard but can be implemented by setting a canister that handles events. This canister can then implement it's own history and/or use a service like [CAP](https://cap.ooo/).

Canisters that implement the SLD721 token standard should have no controllers (immutable) and run the same WASM code (verifiable). This guarantees to the end-user that the canister always behaves as described in the SLD721 token standard.


#### init
---
Canister initializer (name, symbol, custodian).
```
service: (text, text, principal) -> sld721
```

#### balance_of
---
Returns the number of non-fungible tokens owner by the user.
```
balance_of: (User) -> (nat) query;
```

#### owner_of
---
Returns the user that owns the specified token id.
```
owner_of: (principal) -> (UserResult) query;
```

#### name
---
Returns the name of the SLD721 token.
```
name: () -> (text) query;
```

#### symbol
---
Returns the symbol of the SLD721 token.
```
symbol: () -> (text) query;
```

#### total_supply
---
Returns total number of tokens.
```
total_supply: () -> (nat) query;
```

#### approve
---
Add or remove approval to a principal for a specific token id.  
Caller must be owner of specified token id.
```
approve: (opt SubAccount, principal, principal, bool) -> (Result);
```


#### is_approved
---
Returns if caller is approved for specified token id.
```
is_approved: (principal) -> (bool) query;
```

#### get_approved
---
Returns all approved principals for specified token id.
```
get_approved: (principal) -> (vec principal) query;
```

#### transfer
---
Transfer token from caller to specified user.  
Caller must be owner of specified token id or caller must be custodian and token id has no owner yet.
```
transfer: (opt SubAccount, User, principal) -> (Result);
```

#### transfer_from
---
Transfer token from specified user to another specified user.  
Caller must be approved for specified token id.
```
transfer_from: (User, User, principal) -> (Result);
```

#### mint
---
Returns minted token.  
Caller must be custodian.
```
Mint and return new token.
```

#### burn
---
Burn specified token id.  
Caller must be custodian, token id can't be burned if it already has an owner.
```
burn: (principal) -> (Result);
```


#### tokens
---
Returns all token ids and their owners.
```
tokens: () -> (vec Token) query;
```

#### tokens_of
---
Returns all tokens owned by specified user.
```
tokens_of: (User) -> (vec Token) query;
```

#### extensions
---
Returns all extensions of SLD721 token. An extension is a canister principal and a list of it's types.  
Extension example: principal `3ifmd-wqaaa-aaaah-qckda-cai` and types `@ext/common, @ext/nonfungible` for a SLD721 canister that refers to assets from a EXT canister. 
```
extensions: () -> (vec record { principal; vec text }) query;
```

#### set_extension
---
Add or remove extension, extension is removed when types is empty.  
Caller must be custodian.
```
set_extension: (principal, vec text) -> (Result);
```

#### custodians
---
Returns custodians of SLD721 token.
```
custodians: () -> (vec principal) query;
```

#### set_custodian
---
Add or remove custodian, make sure that you have access with another custodian before removing a custodian!
Caller must be custodian.
```
set_custodian: (principal, bool) -> (Result);
```

#### events
---
Returns principal of canister that will be called for each event (transfer, mint, burn and approve).
```
events: () -> (opt principal) query;
```

#### set_events
---
Set principal of canister that will be called for each event (transfer, mint, burn and approve).  
Caller must be custodian.
```
set_events: (opt principal) -> ();
```

#### cycles
---
Returns cycles balance of canister.
```
cycles: () -> (nat) query;
```
