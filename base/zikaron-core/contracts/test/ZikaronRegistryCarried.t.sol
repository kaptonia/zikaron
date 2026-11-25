// SPDX-License-Identifier: CC0-1.0
pragma solidity 0.8.28;

import {Test} from "forge-std/Test.sol";
import {Vm} from "forge-std/Vm.sol";
import {ZikaronRegistry} from "../src/ZikaronRegistry.sol";

/// @title Bytes after the hash: the registry under zikaron.face/1 §5.
/// @notice A face entry travels with its anchor: the calldata is the
/// selector of `anchor(bytes32)`, the entry's id, and the entry's bytes.
/// The law leans on one property of the pinned registry build, witnessed
/// here: `anchor` called with bytes after its one word succeeds and emits
/// the log it emits without them, so the transaction anchors as zikaron/2
/// §9.1 reads it and the bytes after the hash anchor nothing of their own.
contract ZikaronRegistryCarriedTest is Test {
    /// keccak256("Anchored(address,bytes32)").
    bytes32 internal constant ANCHORED_TOPIC0 = 0xee1610244dfc4b116b1433dd4459fd7b1d3d2bf737a05fc3f6d8f173005d7147;

    /// From CODEHASH.md: the build the law's readers pin.
    bytes32 internal constant PINNED_CODEHASH = 0xfa97a1d9b22fab2b52f4e27c9a965b32734c40001b565ab365d05c887118f57d;

    bytes4 internal constant SEL_ANCHOR = bytes4(0xeecdf927); // anchor(bytes32)

    ZikaronRegistry internal reg;
    address internal alice = makeAddr("alice");

    function setUp() public {
        reg = new ZikaronRegistry();
        assertEq(address(reg).codehash, PINNED_CODEHASH, "the registry under test is the pinned build");
    }

    function _call(address from, bytes memory data) internal returns (bool ok, Vm.Log[] memory logs) {
        vm.recordLogs();
        vm.prank(from);
        (ok,) = address(reg).call(data);
        logs = vm.getRecordedLogs();
    }

    function _assertOneAnchor(Vm.Log[] memory logs, address from, bytes32 h) internal view {
        assertEq(logs.length, 1, "exactly one log");
        assertEq(logs[0].emitter, address(reg), "emitted by the registry");
        assertEq(logs[0].topics.length, 3, "three topics");
        assertEq(logs[0].topics[0], ANCHORED_TOPIC0, "topic 0");
        assertEq(logs[0].topics[1], bytes32(uint256(uint160(from))), "topic 1 the caller");
        assertEq(logs[0].topics[2], h, "topic 2 the hash");
        assertEq(logs[0].data.length, 0, "empty data");
    }

    /// The calldata of a face entry: selector, sha256 of the bytes, the bytes.
    function test_Carried_EntryBytesAfterTheHash_AnchorAsWithout() public {
        bytes memory entry = bytes(
            '{"author":"0x0000000000000000000000000000000000000001","body":{"act":"home","chain":1,"root":"0x0000000000000000000000000000000000000001"},"entryType":"zikaron.face/1"}'
        );
        bytes32 h = sha256(entry);

        (bool okPlain, Vm.Log[] memory plain) = _call(alice, abi.encodePacked(SEL_ANCHOR, h));
        assertTrue(okPlain, "anchor without bytes after the hash succeeds");
        _assertOneAnchor(plain, alice, h);

        (bool okCarried, Vm.Log[] memory carried) = _call(alice, abi.encodePacked(SEL_ANCHOR, h, entry));
        assertTrue(okCarried, "anchor with the entry after the hash succeeds");
        _assertOneAnchor(carried, alice, h);
    }

    /// Whatever follows the hash, and however long: one log, the same log.
    function testFuzz_Carried_AnyTail_SameLog(address caller, bytes32 h, bytes calldata tail) public {
        vm.assume(caller != address(0));
        vm.assume(caller != address(vm));
        (bool ok, Vm.Log[] memory logs) = _call(caller, abi.encodePacked(SEL_ANCHOR, h, tail));
        assertTrue(ok, "anchor succeeds with any bytes after its word");
        _assertOneAnchor(logs, caller, h);
    }

    /// A word of the tail is no second anchor: the registry reads one word.
    function test_Carried_AWordInTheTail_IsNotAnchored() public {
        bytes32 h = keccak256("the anchored hash");
        bytes32 other = keccak256("a word riding in the tail");
        (bool ok, Vm.Log[] memory logs) = _call(alice, abi.encodePacked(SEL_ANCHOR, h, other));
        assertTrue(ok);
        _assertOneAnchor(logs, alice, h);
    }

    /// The decoder's one bound is below: fewer than the word's 32 bytes reverts.
    function test_Carried_ShortOfTheWord_Reverts() public {
        bytes32 h = keccak256("short");
        bytes memory full = abi.encodePacked(SEL_ANCHOR, h);
        bytes memory short_ = new bytes(35);
        for (uint256 i = 0; i < 35; ++i) {
            short_[i] = full[i];
        }
        (bool ok, Vm.Log[] memory logs) = _call(alice, short_);
        assertFalse(ok, "35 bytes of calldata do not reach anchor's word");
        assertEq(logs.length, 0, "and nothing is logged");
    }
}
