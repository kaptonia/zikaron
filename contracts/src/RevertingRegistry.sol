// SPDX-License-Identifier: CC0-1.0
pragma solidity 0.8.28;

/// @title RevertingRegistry: a registry that passes gas estimation and reverts when really sent.
/// @notice Same ABI as ZikaronRegistry (`anchor`, `anchorMany`, event `Anchored`).
/// The only difference is one check: the call reverts unless more than `FLOOR` gas is
/// still available on entry. `eth_estimateGas` searches for the smallest gas limit
/// that succeeds and therefore finds one above the floor, so estimation passes; the
/// app's anchor layer sends every anchoring transaction with a fixed gas limit of
/// 200000 (below the floor once intrinsic gas is paid), so the real send reverts
/// with receipt status 0. This is the case law §9.1 names: a transaction whose
/// status is not 1 is not an anchor, whatever the node echoed at broadcast.
/// It lets tests check the queue rule ("an entry leaves the queue only when the
/// receipt says success") against a chain that really reverts, not a mocked one.
contract RevertingRegistry {
    event Anchored(address indexed author, bytes32 indexed hash);

    /// @notice Gas that must still be available on entry; the app sends with less.
    uint256 public constant FLOOR = 250_000;

    function anchor(bytes32 hash) external {
        require(gasleft() > FLOOR, "reverts when really sent");
        emit Anchored(msg.sender, hash);
    }

    function anchorMany(bytes32[] calldata hashes) external {
        require(gasleft() > FLOOR, "reverts when really sent");
        uint256 n = hashes.length;
        for (uint256 i = 0; i < n; ++i) {
            emit Anchored(msg.sender, hashes[i]);
        }
    }
}
