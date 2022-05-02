# 🐍 SLD721

[SLD721](SPEC.md) is a non-fungible token standard based on ERC-721 adapted for the Internet Computer. The purpose of this standard is to only implement the minimal required funcionality of a ledger: transfer, mint, burn and approval. Other functionality like returning media assets that represent NFT tokens is out of scope and should be implemented in other canisters. A SLD721 token can refer to other canisters by setting extensions.

Event history tracking is not part of the NFT standard but can be implemented by setting a canister that handles events. This canister can then implement it's own history and/or use a service like [CAP](https://cap.ooo/).

Canisters that implement the SLD721 token standard should have no controllers (immutable) and run the same WASM code (verifiable). This guarantees to the end-user that the canister always behaves as described in the SLD721 token standard.

## 🧑‍💻 Specification

The specification for the SLD721 token standard is available [here](SPEC.md).

## ⛏️ Minting your collection

Instructions on minting your own NFT canister and a self mint tool at [Slide Market](https://slide.computer) will be made available at a later point.

## 💎 Slide asset canister standard

The slide asset canister standard will be made available at a later point.

## 💾 Official WASM binary

An official verifiable WASM binary will be made available at a later point.
