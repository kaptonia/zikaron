// SPDX-License-Identifier: CC0-1.0
pragma solidity 0.8.28;

/// A contract a basis may declare as a registry, whose logs share the
/// `Anchored(address,bytes32)` topic 0 and break the form of zikaron/1 s9.1
/// in one way each. Every function carries the hash in its calldata at
/// offset 4, so the only thing the scan can object to is the log's shape.
contract Rogue {
    bytes32 private constant T0 = keccak256("Anchored(address,bytes32)");

    /// Two topics; the hash travels in the data instead.
    function twoTopics(bytes32 h) external {
        bytes32 t0 = T0;
        assembly {
            mstore(0, h)
            log2(0, 32, t0, caller())
        }
    }

    /// Three well-formed topics and 32 data bytes.
    function threeTopicsWithData(bytes32 h) external {
        bytes32 t0 = T0;
        assembly {
            mstore(0, h)
            log3(0, 32, t0, caller(), h)
        }
    }

    /// Four topics and no data.
    function fourTopics(bytes32 h) external {
        bytes32 t0 = T0;
        assembly {
            log4(0, 0, t0, caller(), h, h)
        }
    }
}

/// Code for an EOA to delegate to (EIP-7702): every call reverts, so a
/// self-directed transaction of the delegated account fails with status 0.
contract Reverter {
    fallback() external payable {
        revert("reverter");
    }

    receive() external payable {
        revert("reverter");
    }
}
