// SPDX-License-Identifier: CC0-1.0
pragma solidity 0.8.28;

/// @title ZikaronRegistry: the author's horn.
/// @notice Emits `Anchored(sender, hash)` and nothing else. No storage, no
/// owner, no upgrade path, no difference between callers. The wire law
/// (zikaron/1 §9.1) reads the log by its shape alone: exactly three topics,
/// empty data, topic 1 the transaction sender, topic 2 a hash that also
/// sits in the sender's own calldata at a 32-aligned offset. Both entry
/// points satisfy that under a canonical ABI encoding: `anchor` carries
/// the hash at calldata offset 4, `anchorMany` carries each element at 4
/// plus a multiple of 32. A caller who encodes `anchorMany` with a
/// non-canonical head offset still gets the event and loses the anchor,
/// since the law declines to read a hash at any other offset. The event
/// names `msg.sender`; a contract that calls in is charged as itself, and
/// the law reads only logs whose topic 1 is the transaction's sender.
contract ZikaronRegistry {
    event Anchored(address indexed author, bytes32 indexed hash);

    /// @notice Anchor one hash under the caller's address.
    function anchor(bytes32 hash) external {
        emit Anchored(msg.sender, hash);
    }

    /// @notice Anchor several hashes under the caller's address, one event
    /// per element and none for an empty array.
    function anchorMany(bytes32[] calldata hashes) external {
        uint256 n = hashes.length;
        for (uint256 i = 0; i < n; ++i) {
            emit Anchored(msg.sender, hashes[i]);
        }
    }
}
