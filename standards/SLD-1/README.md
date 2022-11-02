# `SLD-1`: Token Standard

| Status |
|:------:|
| Draft  |

The SLD-1 is a standard for Non-Fungible Tokens on the Internet Computer based on the [ICRC-1](https://github.com/dfinity/ICRC-1/blob/main/standards/ICRC-1/README.md) standard.

## Data

### account

A `principal` can have multiple accounts. Each account of a `principal` is identified by a 32-byte string called `subaccount`. Therefore an account corresponds to a pair `(principal, subaccount)`.

The account identified by the subaccount with all bytes set to 0 is the _default account_ of the `principal`.

```candid "Type definitions" +=
type Subaccount = blob;
type Account = record { owner : principal; subaccount : opt Subaccount; };
```

## Methods

### sld1_name <span id="name_method"></span>

Returns the name of the token (e.g., `ICSnakes`).

```candid "Methods" +=
icrc1_name : () -> (text) query;
```

### sld1_symbol <span id="symbol_method"></span>

Returns the symbol of the token (e.g., `SNAKE`).

```candid "Methods" +=
sld1_symbol : () -> (text) query;
```

### sld1_metadata <span id="metadata_method"></span>

Returns the list of metadata entries for this ledger.  
See the "Metadata" section below.

```candid "Type definitions" +=
type Value = variant { Nat : nat; Int : int; Text : text; Blob : blob };
```

```candid "Methods" +=
sld1_metadata : () -> (vec record { text; Value }) query;
```

### sld1_total_supply

Returns the total number of tokens on all accounts except for the [minting account](#minting_account).

```candid "Methods" +=
sld1_total_supply : () -> (nat) query;
```

### icrc1_minting_account

Returns the [minting account](#minting_account) if this ledger supports minting and burning tokens.

```candid "Methods" +=
sld1_minting_account : () -> (opt Account) query;
```

### sld1_balance_of

Returns the balance of the account given as an argument.

```candid "Methods" +=
sld1_balance_of : (Account) -> (nat) query;
```

### sld1_owner_of

Returns the owner of the token id given as an argument.

```candid "Methods" +=
sld1_owner_of : (TokenId) -> (opt Account) query;
```

### sld1_tokens

Returns token ids for the page given as an argument, burned tokens are not included.

```candid "Methods" +=
sld1_tokens: (nat) -> (vec TokenId) query;
```

### sld1_tokens_of

Returns token ids for the account and page given as an argument.

```candid "Methods" +=
sld1_tokens_of: (Account, nat) -> (vec TokenId) query;
```

### sld1_metadata_of

Returns the list of metadata entries for this token id.  
See the "Metadata" section below.

```candid "Methods" +=
sld1_metadata_of: (TokenId) -> (opt vec record { text; Value }) query;
```

### sld1_transfer <span id="transfer_method"></span>

Transfers `token_id` from account `record { of = caller; subaccount = from_subaccount }` to the `to` account.

```candid "Type definitions" +=
type TransferArgs = record {
    from_subaccount: opt Subaccount;
    to: Account;
    token_id: TokenId;
    memo: opt blob;
    created_at_time: opt nat64;
};

type TransferError = variant {
    NotFound;
    NotOwner;
    TooOld;
    CreatedInFuture : record { ledger_time: nat64 };
    TemporarilyUnavailable;
    GenericError: record {
        error_code: nat;
        message: text
    };
};
```

```candid "Methods" +=
icrc1_transfer : (TransferArgs) -> (variant { Ok: nat; Err: TransferError; });
```

The `memo` parameter is an arbitrary blob that has no meaning to the ledger.
The ledger SHOULD allow memos of at least 32 bytes in length.

The `created_at_time` parameter indicates the time (as nanoseconds since the UNIX epoch in the UTC timezone) at which the client constructed the transaction.
The ledger SHOULD reject transactions that have `created_at_time` argument too far in the past or the future, returning `variant { TooOld }` and `variant { CreatedInFuture = record { ledger_time = ... } }` errors correspondingly.

The result is either the transaction index of the transfer or an error.

### icrc1_supported_standards

Returns the list of standards this ledger implements.
See the ["Extensions"](#extensions) section below.

```candid "Methods" +=
sld1_supported_standards : () -> (vec record { name : text; url : text }) query;
```

The result of the call should always have at least one entry,

```candid
record { name = "SLD-1"; url = "https://github.com/slide-computer/slide-token" }
```

## Extensions <span id="extensions"></span>

The base standard intentionally excludes some ledger functions essential for building a rich NFT ecosystem, for example:

- Reliable transaction notifications for smart contracts.
- The block structure and the interface for fetching blocks.
- Pre-signed transactions.

The standard defines the `sld1_supported_standards` endpoint to accommodate these and other future extensions.
This endpoint returns names of all specifications (e.g., `"sld-2"` or `"DIP-721-V2"`) implemented by the ledger.

## Metadata

A ledger can expose metadata to simplify integration with wallets and improve user experience. The client can use the sld1_metadata method to fetch the metadata entries. All the metadata entries are optional.

### Key format

The metadata keys are arbitrary Unicode strings and must follow the pattern `<namespace>:<key>`, where `<namespace>` is a string not containing colons.
Namespace `sld1` is reserved for keys defined in this standard.

### Standard metadata entries

| Key           | Example value                                                               | Semantics                                                                                                                                                                           |
|---------------|-----------------------------------------------------------------------------|-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `sld1:symbol` | `variant { Text = "XTKN" }`                                                 | The token currency code (see [ISO-4217](https://en.wikipedia.org/wiki/ISO_4217)). When present, should be the same as the result of the [`sld1_symbol`](#symbol_method) query call. |
| `sld1:name`   | `variant { Text = "Test Token" }`                                           | The name of the token. When present, should be the same as the result of the [`sld1_name`](#name_method) query call.                                                                |
| `sld1:logo`   | `variant { Text = "https://uscpd-maaaa-aaaak-abvqa-cai.ic0.app/logo.svg" }` | The logo of the token.                                                                                                                                                              |

## Minting account <span id="minting_account"></span>

The minting account is a unique account that can create new tokens and acts as the receiver of burnt tokens.

Transfers _from_ the minting account act as _mint_ transactions depositing fresh tokens on the destination account.
Mint transactions have no fee.

Transfers _to_ the minting account act as _burn_ transactions, removing tokens from the token supply.

Minting and burning NFT tokens is out of the scope of this standard, see SLD-4 and SLD-5 for standards regarding this.

## Textual representation of accounts

See the _canonical textual format_ defined in [ICRC-1](https://github.com/dfinity/ICRC-1/blob/main/standards/ICRC-1/README.md#textual-representation-of-accounts)