// SPDX-License-Identifier: CC0-1.0
pragma solidity 0.8.28;

/// @title ZikaronRevocation: the author's notice of withdrawal.
/// @notice `revoke(entry, grant)` emits two logs and nothing else, in this
/// order: `Anchored(sender, entry)`, the anchor of the revocation entry in
/// the one form zikaron/2 §9.1 reads (three topics, empty data, topic 1 the
/// transaction's sender, topic 2 a word of the sender's own calldata, here
/// at offset 4), and `Revoked(sender, grant)`, the notice zikaron.kit/2 §6.3
/// reads (three topics, empty data, topic 1 the sender, topic 2 a word of
/// the calldata, here at offset 36). No storage, no owner, no upgrade path,
/// no difference between callers. Both events name `msg.sender`; a contract
/// that calls in is charged as itself, and both laws read only logs whose
/// topic 1 is the transaction's sender. An account carrying an EIP-7702
/// designator that calls in is charged as itself and is its transaction's
/// sender: its logs meet both forms, and the codeless test of zikaron/2
/// §9.3 voids them, which zikaron.kit/2 §7.2 reads as no acquittal. A
/// withdrawal is therefore the key's own top-level transaction to this
/// contract from a key carrying no code; a call through a wallet contract
/// or a batcher names that contract in topic 1, which is no transaction's
/// sender, and is no notice, and a delegated account's own call is a VOID
/// notice, which the kit law reads as no acquittal. The registry that anchors
/// ordinary entries is another contract and is not replaced by this one: a
/// reader lists both in its basis, and reads a revocation entry's anchor
/// here and the notice of the grant it withdraws beside it, in one
/// transaction.
contract ZikaronRevocation {
    event Anchored(address indexed author, bytes32 indexed hash);
    event Revoked(address indexed author, bytes32 indexed grant);

    /// @notice Anchor the revocation entry `entry` and notice the withdrawal
    /// of the grant `grant`, both under the caller's address, in one
    /// transaction. The contract tests nothing about either word: what the
    /// words are is the author's affair, and what they mean is the laws'.
    /// Call it directly, from a codeless key's own transaction: a call from a
    /// contract or a batcher produces logs neither law reads as this key's
    /// notice, and a delegated account's call produces a VOID one.
    function revoke(bytes32 entry, bytes32 grant) external {
        emit Anchored(msg.sender, entry);
        emit Revoked(msg.sender, grant);
    }
}
