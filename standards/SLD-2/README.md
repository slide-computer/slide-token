# `SLD-2`: Approve and Transfer From

| Status |
|:------:|
| Draft  |

## Abstract

`SLD-2` is an extension of the base `SLD-1` standard.
`SLD-2` specifies a way for an account owner to delegate token transfers to a third party on the owner's behalf.

The approve and transfer-from flow is a 2-step process.
1. Account owner Alice entitles principal Bob to transfer token X from her account A by calling the `sld2_set_approval` method on the ledger.
2. Bob can transfer token X from account A to any account by calling the `sld2_transfer_from` method on the ledger.

## Motivation

The approve-transfer-from pattern became popular in the Ethereum ecosystem thanks to the [ERC-20](https://ethereum.org/en/developers/docs/standards/tokens/erc-20/) token standard.
This interface enables new application capabilities:

1. Alice can approve a token transfer to a service in advance, allowing the service to transfer the token at a later date.
   Real-world examples include NFT marketplaces.

2. Alice can approve token transfers for her whole account to a service in advance, allowing the service to transfer any token at a later date.

## Specification

> The keywords "MUST", "MUST NOT", "REQUIRED", "SHALL", "SHALL NOT", "SHOULD", "SHOULD NOT", "RECOMMENDED", "MAY", and "OPTIONAL" in this document are to be interpreted as described in RFC 2119.

**Canisters implementing the `SLD-2` standard MUST implement all the functions in the `SLD-1` interface**

**Canisters implementing the `SLD-2` standard MUST include `SLD-2` in the list returned by the `sld1_supported_standards` method**

## Methods

```candid "Type definitions" +=
type Account = record {
    owner : principal;
    subaccount : opt blob;
};
```

### sld2_set_approval

Entitles `spender` to transfer the provided `token_id` on behalf of the caller from account `{ owner = caller; subaccount = from_subaccount }`.
The `token_id` can be approved to multiple `spender` at the same time to e.g. allow listing on multiple marketplaces without the need to approve these marketplaces as `operator`.

```candid "Methods" +=
sld2_set_approval : (SetApprovalArgs) -> (variant { Ok : nat; Err : SetApprovalError });
```

```candid "Type definitions" +=
type SetApprovalArgs = record {
    from_subaccount: opt blob;
    spender: principal;
    token_id: nat;
    approved: bool;
    memo: opt blob;
    created_at_time: opt nat64;
};

type SetApprovalError = variant {
    NotFound;
    NotOwner;
    MaxApprovals: nat;
    TemporarilyUnavailable;
    GenericError: record {
        error_code: nat;
        message: text
    };
};
```

#### Preconditions

* The caller is the owner of `token_id`.

#### Postconditions

* `spender` is approved to transfer `token_id` from the `{ owner = caller; subaccount = from_subaccount }` account.

### sld2_set_approval_for_all

Entitles `spender` to transfer any token on behalf of the caller from account `{ owner = caller; subaccount = from_subaccount }`.
The `account` can be approved to multiple `spender` at the same time to e.g. allow multiple services to manage the tokens at the same time.

```candid "Methods" +=
sld2_set_approval_for_all : (SetApprovalForAllArgs) -> (variant { Ok : nat; Err : SetApprovalForAllError });
```

```candid "Type definitions" +=
type SetApprovalForAllArgs = record {
    from_subaccount: opt blob;
    spender: principal;
    approved: bool;
    memo: opt blob;
    created_at_time: opt nat64;
};

type SetApprovalForAllError = variant {
    MaxApprovals: nat;
    TemporarilyUnavailable;
    GenericError: record {
        error_code: nat;
        message: text
    };
};
```

#### Preconditions

* The max number of `operator` approvals for the `account` has not been reached.
  Otherwise, the ledger MUST return an `MaxApprovals` error.
  This limit is optional. If there is no limit, a `MaxApprovals` error MUST NOT be returned. 

#### Postconditions

* `spender` is approved to any token from the `{ owner = caller; subaccount = from_subaccount }` account.

### sld2_transfer_from

Transfers a token from between two accounts.

```candid "Methods" +=
sld2_transfer_from : (TransferFromArgs) -> (variant { Ok : nat; Err : TransferFromError });
```

```candid "Type definitions" +=
type TransferFromArgs = record {
    from: Account;
    to: Account;
    token_id: nat;
    memo: opt blob;
    created_at_time: opt nat64;
};

type TransferFromError = variant {
    NotFound;
    NotOwner;
    NotApproved;
    TemporarilyUnavailable;
    GenericError: record {
        error_code: nat;
        message: text
    };
};
```

#### Preconditions

* `spender` is approved to transfer `token_id` from the `from` account.  
  Otherwise, the ledger MUST return an `NotApproved` error.

* The `from` account is the owner of `token_id`.
  Otherwise, the ledger MUST return an `NotOwner` error.
* 
* The `token_id` exists in the ledger.
  Otherwise, the ledger MUST return an `NotFound` error.

#### Postconditions

* All `spender` are removed from `token_id`.
* The ledger transfers `token_id` from the `from` account to the `to` account.

### sld2_get_approved

Returns the approved `spenders` that can transfer `token_id` from the specified `account`.
If there is are no active approvals, the ledger MUST return an empty list.

```candid "Methods" +=
sld2_get_approved: (nat) -> (vec principal) query;
```

### sld2_get_approved_for_all

Returns the approved `operators` that can transfer any token from the specified `account`.
If there is are no active operators, the ledger MUST return an empty list.

```candid "Methods" +=
sld2_get_approved_for_all: (Account) -> (vec principal) query;
```

### sld1_supported_standards

Returns the list of standards this ledger supports.
Any ledger supporting `SLD-2` MUST include a record with the `name` field equal to `"SLD-2"` in that list.

```candid "Methods" +=
sld1_supported_standards : () -> (vec record { name : text; url : text }) query;
```