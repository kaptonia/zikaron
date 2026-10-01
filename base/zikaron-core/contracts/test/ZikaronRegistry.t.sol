// SPDX-License-Identifier: CC0-1.0
pragma solidity 0.8.28;

import {Test} from "forge-std/Test.sol";
import {Vm} from "forge-std/Vm.sol";
import {ZikaronRegistry} from "../src/ZikaronRegistry.sol";

/// @dev A contract caller. Used to show that the registry charges its
/// immediate caller, and that a log so produced fails the zikaron/1 §9.1
/// registry-form test, whose topic 1 must be the *transaction's* sender.
contract Forwarder {
    error Boom();

    ZikaronRegistry internal immutable REG;

    constructor(ZikaronRegistry r) {
        REG = r;
    }

    function forward(bytes32 h) external {
        REG.anchor(h);
    }

    function forwardMany(bytes32[] calldata hs) external {
        REG.anchorMany(hs);
    }

    function forwardThenRevert(bytes32 h) external {
        REG.anchor(h);
        revert Boom();
    }
}

/// @title The horn read by its wire law.
/// @notice Every assertion here is a clause of zikaron/1 §9.1 registry form,
/// or one of the three properties §9.1 leans on: one event per hash and none
/// for an empty array, the caller's own address in topic 1, and a registry
/// that holds no state and has no owner. Gas is recorded for the record and
/// is asserted nowhere.
contract ZikaronRegistryTest is Test {
    // ---- pinned constants -------------------------------------------------

    /// keccak256("Anchored(address,bytes32)").
    bytes32 internal constant ANCHORED_TOPIC0 = 0xee1610244dfc4b116b1433dd4459fd7b1d3d2bf737a05fc3f6d8f173005d7147;

    /// From CODEHASH.md, transcribed by hand. A drift in solc version,
    /// evm_version, optimizer, via_ir, bytecode_hash or cbor_metadata moves
    /// this and fails the suite.
    bytes32 internal constant PINNED_CODEHASH = 0xfa97a1d9b22fab2b52f4e27c9a965b32734c40001b565ab365d05c887118f57d;
    uint256 internal constant PINNED_RUNTIME_LENGTH = 246;

    bytes4 internal constant SEL_ANCHOR = bytes4(0xeecdf927); // anchor(bytes32)
    bytes4 internal constant SEL_ANCHOR_MANY = bytes4(0x1e376b3f); // anchorMany(bytes32[])

    // EVM opcodes.
    uint8 internal constant OP_SLOAD = 0x54;
    uint8 internal constant OP_SSTORE = 0x55;
    uint8 internal constant OP_TLOAD = 0x5c;
    uint8 internal constant OP_TSTORE = 0x5d;
    uint8 internal constant OP_LOG0 = 0xa0;
    uint8 internal constant OP_LOG1 = 0xa1;
    uint8 internal constant OP_LOG2 = 0xa2;
    uint8 internal constant OP_LOG3 = 0xa3;
    uint8 internal constant OP_LOG4 = 0xa4;
    uint8 internal constant OP_CREATE = 0xf0;
    uint8 internal constant OP_CALL = 0xf1;
    uint8 internal constant OP_DELEGATECALL = 0xf4;
    uint8 internal constant OP_CREATE2 = 0xf5;
    uint8 internal constant OP_SELFDESTRUCT = 0xff;

    ZikaronRegistry internal reg;
    Forwarder internal fwd;

    address internal alice = makeAddr("alice");
    address internal bob = makeAddr("bob");

    function setUp() public {
        reg = new ZikaronRegistry();
        fwd = new Forwarder(reg);
    }

    // =======================================================================
    // 1. anchor(h): exactly one log, of the §9.1 shape
    // =======================================================================

    function test_Anchor_EmitsExactlyOneWellShapedLog() public {
        bytes32 h = keccak256("one hash");

        vm.recordLogs();
        vm.prank(alice);
        reg.anchor(h);
        Vm.Log[] memory logs = vm.getRecordedLogs();

        assertEq(logs.length, 1, "anchor must emit exactly one log");
        _assertWireShape(logs[0], address(reg), alice, h);
    }

    function test_Anchor_ZeroHash_IsStillAnchored() public {
        vm.recordLogs();
        vm.prank(alice);
        reg.anchor(bytes32(0));
        Vm.Log[] memory logs = vm.getRecordedLogs();

        assertEq(logs.length, 1, "the zero hash gets a log like any other");
        _assertWireShape(logs[0], address(reg), alice, bytes32(0));
    }

    function testFuzz_Anchor_TopicsTrackTheInputs(address caller, bytes32 h) public {
        vm.assume(caller != address(0));
        vm.assume(caller != address(vm)); // the cheatcode address cannot call out

        vm.recordLogs();
        vm.prank(caller);
        reg.anchor(h);
        Vm.Log[] memory logs = vm.getRecordedLogs();

        assertEq(logs.length, 1, "one log per anchor, always");
        _assertWireShape(logs[0], address(reg), caller, h);
    }

    // =======================================================================
    // 2. anchorMany: one log per element, in order; none for an empty array
    // =======================================================================

    function test_AnchorMany_ThreeHashesInOrder() public {
        bytes32[] memory hs = new bytes32[](3);
        hs[0] = keccak256("h1");
        hs[1] = keccak256("h2");
        hs[2] = keccak256("h3");

        vm.recordLogs();
        vm.prank(alice);
        reg.anchorMany(hs);
        Vm.Log[] memory logs = vm.getRecordedLogs();

        assertEq(logs.length, 3, "one log per element");
        for (uint256 i = 0; i < 3; ++i) {
            _assertWireShape(logs[i], address(reg), alice, hs[i]);
        }
    }

    function test_AnchorMany_Empty_EmitsNothing() public {
        bytes32[] memory none = new bytes32[](0);

        vm.recordLogs();
        vm.prank(alice);
        reg.anchorMany(none);
        Vm.Log[] memory logs = vm.getRecordedLogs();

        assertEq(logs.length, 0, "an empty array anchors nothing");
    }

    function test_AnchorMany_RepeatedHash_EmitsOnePerOccurrence() public {
        bytes32 h = keccak256("same");
        bytes32[] memory hs = new bytes32[](3);
        hs[0] = h;
        hs[1] = h;
        hs[2] = h;

        vm.recordLogs();
        vm.prank(alice);
        reg.anchorMany(hs);
        Vm.Log[] memory logs = vm.getRecordedLogs();

        assertEq(logs.length, 3, "the registry deduplicates nothing");
        for (uint256 i = 0; i < 3; ++i) {
            _assertWireShape(logs[i], address(reg), alice, h);
        }
    }

    function testFuzz_AnchorMany_OneLogPerElementInOrder(bytes32[] memory hs) public {
        vm.assume(hs.length <= 64);

        vm.recordLogs();
        vm.prank(alice);
        reg.anchorMany(hs);
        Vm.Log[] memory logs = vm.getRecordedLogs();

        assertEq(logs.length, hs.length, "log count equals element count");
        for (uint256 i = 0; i < hs.length; ++i) {
            _assertWireShape(logs[i], address(reg), alice, hs[i]);
        }
    }

    // =======================================================================
    // 3. Caller attribution
    // =======================================================================

    function test_TwoCallers_EachGetsItsOwnAddressInTopic1() public {
        bytes32 h = keccak256("shared hash");

        vm.recordLogs();
        vm.prank(alice);
        reg.anchor(h);
        Vm.Log[] memory a = vm.getRecordedLogs();

        vm.recordLogs();
        vm.prank(bob);
        reg.anchor(h);
        Vm.Log[] memory b = vm.getRecordedLogs();

        assertEq(a.length, 1, "alice: one log");
        assertEq(b.length, 1, "bob: one log");
        _assertWireShape(a[0], address(reg), alice, h);
        _assertWireShape(b[0], address(reg), bob, h);
        assertTrue(a[0].topics[1] != b[0].topics[1], "two callers, two authors");
        assertEq(a[0].topics[2], b[0].topics[2], "the same hash for both");
    }

    /// A contract caller is charged as itself. The EOA behind it never
    /// appears, which is exactly why §9.1 (topic 1 = the *transaction's*
    /// sender) does not read such a log as an anchor.
    function test_ContractCaller_IsChargedAsItself_NotTheEoaBehindIt() public {
        bytes32 h = keccak256("via forwarder");
        bytes memory txCalldata = abi.encodeWithSelector(Forwarder.forward.selector, h);

        vm.recordLogs();
        vm.prank(alice, alice); // msg.sender and tx.origin both alice
        (bool ok,) = address(fwd).call(txCalldata);
        Vm.Log[] memory logs = vm.getRecordedLogs();

        assertTrue(ok, "the forward call succeeds");
        assertEq(logs.length, 1, "one log");
        _assertWireShape(logs[0], address(reg), address(fwd), h);
        assertTrue(
            logs[0].topics[1] != bytes32(uint256(uint160(alice))), "the EOA behind the forwarder is not the author"
        );

        // The §9.1 consequence: with alice as the transaction's sender this
        // log is not an anchor, however well shaped it is.
        assertFalse(
            _isAnchorUnder91(1, address(reg), alice, txCalldata, logs[0]),
            "a forwarded log is not an anchor for the EOA"
        );
    }

    function test_ContractCaller_AnchorMany_IsChargedAsItself() public {
        bytes32[] memory hs = new bytes32[](2);
        hs[0] = keccak256("f1");
        hs[1] = keccak256("f2");

        vm.recordLogs();
        vm.prank(alice, alice);
        fwd.forwardMany(hs);
        Vm.Log[] memory logs = vm.getRecordedLogs();

        assertEq(logs.length, 2, "two logs");
        _assertWireShape(logs[0], address(reg), address(fwd), hs[0]);
        _assertWireShape(logs[1], address(reg), address(fwd), hs[1]);
    }

    // =======================================================================
    // 4. Calldata containment: the §9.1 alignment rule
    // =======================================================================

    function test_Calldata_Anchor_HashSitsAtOffsetFour() public pure {
        bytes32 h = keccak256("offset four");
        bytes memory cd = abi.encodeWithSelector(SEL_ANCHOR, h);

        assertEq(cd.length, 36, "selector plus one word");
        assertEq(_word(cd, 4), h, "the hash begins at byte 4");

        (bool found, uint256 off) = _carriedAtAlignedOffset(cd, h);
        assertTrue(found, "the hash is carried at an aligned offset");
        assertEq(off, 4, "and that offset is 4");
        assertEq(off % 32, 4, "4 plus a multiple of 32");
    }

    function test_Calldata_AnchorMany_ElementsAtFourPlusMultipleOf32() public pure {
        uint256 n = 5;
        bytes32[] memory hs = new bytes32[](n);
        for (uint256 i = 0; i < n; ++i) {
            hs[i] = keccak256(abi.encodePacked("elem", i));
        }
        bytes memory cd = abi.encodeWithSelector(SEL_ANCHOR_MANY, hs);

        // selector (4) + head offset word (32) + length word (32) + n words.
        assertEq(cd.length, 4 + 64 + 32 * n, "canonical encoding length");
        assertEq(uint256(_word(cd, 4)), 32, "head offset word is 0x20");
        assertEq(uint256(_word(cd, 36)), n, "length word");

        for (uint256 i = 0; i < n; ++i) {
            uint256 expected = 4 + 64 + 32 * i;
            assertEq(_word(cd, expected), hs[i], "element sits where the law expects");

            (bool found, uint256 off) = _carriedAtAlignedOffset(cd, hs[i]);
            assertTrue(found, "element is carried at an aligned offset");
            assertEq(off, expected, "first aligned occurrence is the element itself");
            assertEq((off - 4) % 32, 0, "4 plus a multiple of 32");
        }
    }

    /// The full §9.1 registry-form test, applied end to end to a real call.
    function test_Section91_Anchor_PassesTheWholeTest() public {
        bytes32 h = keccak256("end to end");
        bytes memory cd = abi.encodeWithSelector(SEL_ANCHOR, h);

        vm.recordLogs();
        vm.prank(alice, alice);
        (bool ok,) = address(reg).call(cd);
        Vm.Log[] memory logs = vm.getRecordedLogs();

        assertTrue(ok, "status 1");
        assertEq(logs.length, 1, "one log");
        assertTrue(_isAnchorUnder91(1, address(reg), alice, cd, logs[0]), "the log is an anchor under 9.1");
    }

    function test_Section91_AnchorMany_EveryLogPassesTheWholeTest() public {
        uint256 n = 4;
        bytes32[] memory hs = new bytes32[](n);
        for (uint256 i = 0; i < n; ++i) {
            hs[i] = keccak256(abi.encodePacked("many", i));
        }
        bytes memory cd = abi.encodeWithSelector(SEL_ANCHOR_MANY, hs);

        vm.recordLogs();
        vm.prank(bob, bob);
        (bool ok,) = address(reg).call(cd);
        Vm.Log[] memory logs = vm.getRecordedLogs();

        assertTrue(ok, "status 1");
        assertEq(logs.length, n, "one log per element");
        for (uint256 i = 0; i < n; ++i) {
            assertTrue(_isAnchorUnder91(1, address(reg), bob, cd, logs[i]), "every log is an anchor under 9.1");
        }
    }

    /// The registry list bounds which logs are read. An undeclared emitter
    /// is not an anchor however well shaped its log is.
    function test_Section91_UndeclaredEmitter_IsNotAnAnchor() public {
        ZikaronRegistry other = new ZikaronRegistry();
        bytes32 h = keccak256("undeclared");
        bytes memory cd = abi.encodeWithSelector(SEL_ANCHOR, h);

        vm.recordLogs();
        vm.prank(alice, alice);
        (bool ok,) = address(other).call(cd);
        Vm.Log[] memory logs = vm.getRecordedLogs();

        assertTrue(ok, "the call succeeds");
        assertEq(logs.length, 1, "and emits a well shaped log");
        assertEq(address(other).codehash, PINNED_CODEHASH, "byte-identical code");
        assertFalse(
            _isAnchorUnder91(1, address(reg), alice, cd, logs[0]), "an undeclared emitter anchors nothing"
        );
    }

    // --- the alignment rule itself, exercised against hand-built calldata ---

    function test_Alignment_HashAtOffsetFive_IsNotCarried() public pure {
        bytes32 h = keccak256("misaligned");
        bytes memory cd = abi.encodePacked(bytes5(0), h);

        assertEq(cd.length, 37, "five filler bytes plus one word");
        assertEq(_word(cd, 5), h, "the hash is present, at byte 5");

        (bool found,) = _carriedAtAlignedOffset(cd, h);
        assertFalse(found, "offset 5 is neither 32k nor 4 + 32k");
    }

    function test_Alignment_HashAtOffsetZero_IsCarried() public pure {
        bytes32 h = keccak256("aligned at zero");
        bytes memory cd = abi.encodePacked(h);

        (bool found, uint256 off) = _carriedAtAlignedOffset(cd, h);
        assertTrue(found, "a multiple of 32 is an accepted offset");
        assertEq(off, 0, "offset zero");
    }

    function test_Alignment_HashRunningPastTheEnd_IsNotCarried() public pure {
        bytes32 h = keccak256("truncated");
        bytes memory cd = abi.encodePacked(bytes4(0), bytes31(h)); // 35 bytes: one byte short

        assertEq(cd.length, 35, "one byte short of a whole word at offset 4");
        (bool found,) = _carriedAtAlignedOffset(cd, h);
        assertFalse(found, "the 32 bytes must lie wholly within the calldata");
    }

    /// Non-canonical ABI encoding: a hand-built `anchorMany` calldata whose
    /// head offset is not a multiple of 32. The registry accepts it and emits
    /// a well shaped log, and §9.1 declines to read that log because the hash
    /// lands at calldata offset 69. Recorded in FINDINGS.md.
    function test_Section91_NonCanonicalAnchorManyEncoding_IsNotAnAnchor() public {
        bytes32 h = keccak256("crafted");
        bytes memory cd = abi.encodePacked(
            SEL_ANCHOR_MANY,
            bytes32(uint256(33)), // head offset, not a multiple of 32
            bytes1(0), // one filler byte so the array data starts at byte 37
            bytes32(uint256(1)), // length
            h // the single element, at byte 69
        );

        vm.recordLogs();
        vm.prank(alice, alice);
        (bool ok,) = address(reg).call(cd);
        Vm.Log[] memory logs = vm.getRecordedLogs();

        assertTrue(ok, "solc does not require the head offset to be 32-aligned");
        assertEq(logs.length, 1, "a well shaped log is emitted");
        _assertWireShape(logs[0], address(reg), alice, h);

        assertEq(_word(cd, 69), h, "the element sits at byte 69");
        assertEq(uint256(69) % 32, 5, "which is neither 32k nor 4 + 32k");
        assertFalse(
            _isAnchorUnder91(1, address(reg), alice, cd, logs[0]),
            "the calldata rule rejects the non-canonical encoding"
        );
    }

    // =======================================================================
    // 5. No state, no owner
    // =======================================================================

    function test_Runtime_HasNoSstoreOutsidePushData() public view {
        bytes memory code = address(reg).code;
        assertGt(code.length, 0, "deployed");
        assertFalse(_hasOpcode(code, OP_SSTORE), "no SSTORE in the runtime bytecode");
    }

    function test_Runtime_HasNoStateOpcodeAtAll() public view {
        bytes memory code = address(reg).code;
        assertFalse(_hasOpcode(code, OP_SLOAD), "no SLOAD: nothing to read, so no owner check");
        assertFalse(_hasOpcode(code, OP_TSTORE), "no TSTORE");
        assertFalse(_hasOpcode(code, OP_TLOAD), "no TLOAD");
        assertFalse(_hasOpcode(code, OP_CREATE), "no CREATE");
        assertFalse(_hasOpcode(code, OP_CREATE2), "no CREATE2");
        assertFalse(_hasOpcode(code, OP_CALL), "no CALL: it reaches nothing");
        assertFalse(_hasOpcode(code, OP_DELEGATECALL), "no DELEGATECALL: no upgrade path");
        assertFalse(_hasOpcode(code, OP_SELFDESTRUCT), "no SELFDESTRUCT");
    }

    /// Three topics is structural, not a convention: LOG3 is the only log
    /// opcode the runtime contains, so no call path can produce a log of any
    /// other arity.
    function test_Runtime_ContainsOnlyLog3() public view {
        bytes memory code = address(reg).code;
        assertEq(_countOpcode(code, OP_LOG3), 2, "one LOG3 per entry point");
        assertFalse(_hasOpcode(code, OP_LOG0), "no LOG0");
        assertFalse(_hasOpcode(code, OP_LOG1), "no LOG1");
        assertFalse(_hasOpcode(code, OP_LOG2), "no LOG2");
        assertFalse(_hasOpcode(code, OP_LOG4), "no LOG4");
    }

    function test_OpcodeWalk_SkipsPushImmediates() public pure {
        // PUSH1 0x55: the 0x55 byte is data, not an SSTORE.
        assertFalse(_hasOpcode(hex"6055", OP_SSTORE), "PUSH1 immediate is skipped");
        // PUSH32 whose immediate ends in 0x55, then STOP.
        assertFalse(
            _hasOpcode(hex"7f0000000000000000000000000000000000000000000000000000000000000055" hex"00", OP_SSTORE),
            "PUSH32 immediate is skipped"
        );
        // A bare 0x55 is found.
        assertTrue(_hasOpcode(hex"55", OP_SSTORE), "a real SSTORE is found");
        // PUSH0 (0x5f) carries no immediate, so the next byte is an opcode.
        assertTrue(_hasOpcode(hex"5f55", OP_SSTORE), "PUSH0 has no immediate");
        // A truncated PUSH immediate must not run off the end.
        assertFalse(_hasOpcode(hex"7f00", OP_SSTORE), "truncated PUSH terminates the walk");
    }

    function test_NotPayable_AnchorWithValueReverts() public {
        bytes32 h = keccak256("paid");
        vm.deal(alice, 1 ether);

        vm.recordLogs();
        vm.prank(alice);
        (bool ok, bytes memory ret) = address(reg).call{value: 1 wei}(abi.encodeWithSelector(SEL_ANCHOR, h));
        Vm.Log[] memory logs = vm.getRecordedLogs();

        assertFalse(ok, "no payable function exists");
        assertEq(ret.length, 0, "a bare revert, no reason");
        assertEq(logs.length, 0, "the revert precedes the emit, so nothing is logged");
        assertEq(address(reg).balance, 0, "the registry holds nothing");
    }

    function test_NotPayable_AnchorManyWithValueReverts() public {
        bytes32[] memory hs = new bytes32[](1);
        hs[0] = keccak256("paid many");
        vm.deal(alice, 1 ether);

        vm.recordLogs();
        vm.prank(alice);
        (bool ok,) = address(reg).call{value: 1 wei}(abi.encodeWithSelector(SEL_ANCHOR_MANY, hs));
        Vm.Log[] memory logs = vm.getRecordedLogs();

        assertFalse(ok, "no payable function exists");
        assertEq(logs.length, 0, "nothing is logged");
        assertEq(address(reg).balance, 0, "the registry holds nothing");
    }

    function test_PlainTransfer_Reverts() public {
        vm.deal(alice, 1 ether);
        vm.prank(alice);
        (bool ok,) = address(reg).call{value: 1 wei}("");
        assertFalse(ok, "no receive function");
        assertEq(address(reg).balance, 0, "the registry holds nothing");
    }

    function test_UnknownSelector_Reverts() public {
        bytes4[4] memory unknown =
            [bytes4(0xdeadbeef), bytes4(0x00000000), bytes4(0xffffffff), bytes4(keccak256("owner()"))];

        for (uint256 i = 0; i < unknown.length; ++i) {
            vm.recordLogs();
            (bool ok, bytes memory ret) = address(reg).call(abi.encodePacked(unknown[i]));
            Vm.Log[] memory logs = vm.getRecordedLogs();

            assertFalse(ok, "no function beyond the two selectors");
            assertEq(ret.length, 0, "a bare revert");
            assertEq(logs.length, 0, "and no log");
        }
    }

    function test_EmptyCalldata_Reverts() public {
        (bool ok,) = address(reg).call("");
        assertFalse(ok, "no fallback function");
    }

    function test_ShortCalldata_Reverts() public {
        (bool ok,) = address(reg).call(hex"eecdf9"); // three bytes of the anchor selector
        assertFalse(ok, "a partial selector is not a call");
    }

    function test_Anchor_WithoutItsArgument_Reverts() public {
        (bool ok,) = address(reg).call(abi.encodePacked(SEL_ANCHOR));
        assertFalse(ok, "anchor(bytes32) needs its word");
    }

    function testFuzz_UnknownSelector_Reverts(bytes4 sel) public {
        vm.assume(sel != SEL_ANCHOR);
        vm.assume(sel != SEL_ANCHOR_MANY);

        vm.recordLogs();
        (bool ok,) = address(reg).call(abi.encodePacked(sel, bytes32(0)));
        Vm.Log[] memory logs = vm.getRecordedLogs();

        assertFalse(ok, "only two selectors exist");
        assertEq(logs.length, 0, "and nothing else logs");
    }

    // =======================================================================
    // 6. Gas, for the record only
    // =======================================================================

    function test_Gas_Record() public {
        // Warm the registry account first, so no measurement below carries the
        // 2600-gas cold-access charge that only the first touch would pay.
        reg.anchor(keccak256("warm"));

        bytes32 h = keccak256("gas");

        uint256 g0 = gasleft();
        reg.anchor(h);
        uint256 gAnchor = g0 - gasleft();
        emit log_named_uint("gas anchor(bytes32)", gAnchor);

        uint256[3] memory sizes = [uint256(1), 10, 100];
        for (uint256 s = 0; s < sizes.length; ++s) {
            uint256 n = sizes[s];
            bytes32[] memory hs = new bytes32[](n);
            for (uint256 i = 0; i < n; ++i) {
                hs[i] = keccak256(abi.encodePacked("gas", s, i));
            }

            uint256 g1 = gasleft();
            reg.anchorMany(hs);
            uint256 used = g1 - gasleft();

            emit log_named_uint("elements", n);
            emit log_named_uint("  gas anchorMany(bytes32[])", used);
            emit log_named_uint("  gas per element", used / n);
        }
    }

    // =======================================================================
    // 7. codeHash: a settings drift is a different contract
    // =======================================================================

    function test_CodeHash_MatchesCodehashMd() public view {
        assertEq(address(reg).codehash, PINNED_CODEHASH, "codeHash from CODEHASH.md");
    }

    function test_RuntimeLength_MatchesCodehashMd() public view {
        assertEq(address(reg).code.length, PINNED_RUNTIME_LENGTH, "246 bytes of runtime");
    }

    function test_CodeHash_IsKeccakOfTheRuntimeBytecode() public view {
        assertEq(keccak256(address(reg).code), PINNED_CODEHASH, "codeHash is keccak of the runtime");
    }

    function test_EveryDeploymentHasTheSameCodeHash() public {
        ZikaronRegistry a = new ZikaronRegistry();
        ZikaronRegistry b = new ZikaronRegistry();
        assertTrue(address(a) != address(b), "two addresses");
        assertEq(address(a).codehash, PINNED_CODEHASH, "same code");
        assertEq(address(b).codehash, PINNED_CODEHASH, "same code");
    }

    function test_Selectors_MatchTheInterfaceCodehashMdNames() public pure {
        assertEq(bytes32(ZikaronRegistry.anchor.selector), bytes32(SEL_ANCHOR), "anchor(bytes32)");
        assertEq(bytes32(ZikaronRegistry.anchorMany.selector), bytes32(SEL_ANCHOR_MANY), "anchorMany(bytes32[])");
        assertEq(keccak256("Anchored(address,bytes32)"), ANCHORED_TOPIC0, "topic 0");
    }

    // =======================================================================
    // 8. A reverted transaction anchors nothing
    // =======================================================================

    /// The forwarder emits and then reverts. Under §9.1 the receipt carries
    /// status 0, so no log in it is an anchor. `vm.recordLogs()` is not
    /// receipt-faithful here (see FINDINGS.md): it keeps logs emitted inside
    /// a frame that later reverted, which no receipt ever carries. The
    /// assertion therefore runs the §9.1 status gate over what was recorded.
    function test_RevertedTransaction_AnchorsNothing() public {
        bytes32 h = keccak256("doomed");
        bytes memory txCalldata = abi.encodeWithSelector(Forwarder.forwardThenRevert.selector, h);

        vm.recordLogs();
        vm.prank(alice, alice);
        (bool ok,) = address(fwd).call(txCalldata);
        Vm.Log[] memory logs = vm.getRecordedLogs();

        assertFalse(ok, "the transaction reverts, so its receipt status is 0");

        uint8 status = ok ? 1 : 0;
        uint256 anchors = 0;
        for (uint256 i = 0; i < logs.length; ++i) {
            if (_isAnchorUnder91(status, address(reg), alice, txCalldata, logs[i])) {
                ++anchors;
            }
        }
        assertEq(anchors, 0, "a reverted transaction anchors nothing");

        // The state left behind is empty in every observable sense.
        assertEq(address(reg).codehash, PINNED_CODEHASH, "the registry is unchanged");
    }

    /// A revert that precedes the emit records no log at all, which is the
    /// strict form of the claim, assertable without a receipt.
    function test_RevertBeforeEmit_RecordsNoLogAtAll() public {
        bytes32 h = keccak256("never emitted");
        vm.deal(alice, 1 ether);

        vm.recordLogs();
        vm.prank(alice, alice);
        (bool ok,) = address(reg).call{value: 1 wei}(abi.encodeWithSelector(SEL_ANCHOR, h));
        Vm.Log[] memory logs = vm.getRecordedLogs();

        assertFalse(ok, "the value guard reverts before the emit");
        assertEq(logs.length, 0, "no logs recorded");
    }

    function test_SuccessfulSibling_StillAnchors() public {
        // A positive control for the two tests above: the same shapes, not reverted.
        bytes32 h = keccak256("survives");
        bytes memory cd = abi.encodeWithSelector(SEL_ANCHOR, h);

        vm.recordLogs();
        vm.prank(alice, alice);
        (bool ok,) = address(reg).call(cd);
        Vm.Log[] memory logs = vm.getRecordedLogs();

        assertTrue(ok, "status 1");
        assertEq(logs.length, 1, "one log");
        assertTrue(_isAnchorUnder91(1, address(reg), alice, cd, logs[0]), "and it is an anchor");
    }

    // =======================================================================
    // helpers
    // =======================================================================

    /// The §9.1 registry-form test, whole.
    function _isAnchorUnder91(
        uint8 receiptStatus,
        address declaredRegistry,
        address txSender,
        bytes memory txCalldata,
        Vm.Log memory lg
    ) internal pure returns (bool) {
        if (receiptStatus != 1) return false;
        if (lg.emitter != declaredRegistry) return false;
        if (lg.topics.length != 3) return false;
        if (lg.data.length != 0) return false;
        if (lg.topics[0] != ANCHORED_TOPIC0) return false;
        if (uint256(lg.topics[1]) >> 160 != 0) return false; // 12 zero bytes
        if (address(uint160(uint256(lg.topics[1]))) != txSender) return false;
        (bool carried,) = _carriedAtAlignedOffset(txCalldata, lg.topics[2]);
        return carried;
    }

    function _assertWireShape(Vm.Log memory lg, address emitter, address author, bytes32 h) internal pure {
        assertEq(lg.emitter, emitter, "emitter");
        assertEq(lg.topics.length, 3, "exactly three topics");
        assertEq(lg.data.length, 0, "empty data");
        assertEq(lg.topics[0], ANCHORED_TOPIC0, "topic 0");
        assertEq(uint256(lg.topics[1]) >> 160, 0, "topic 1: twelve leading zero bytes");
        assertEq(lg.topics[1], bytes32(uint256(uint160(author))), "topic 1: the author");
        assertEq(lg.topics[2], h, "topic 2: the anchored hash");
    }

    /// Do 32 consecutive bytes equal to `h` lie wholly within `cd`, beginning
    /// at an offset that is a multiple of 32 or 4 plus a multiple of 32?
    function _carriedAtAlignedOffset(bytes memory cd, bytes32 h) internal pure returns (bool, uint256) {
        if (cd.length < 32) return (false, 0);
        for (uint256 o = 0; o + 32 <= cd.length; ++o) {
            uint256 m = o % 32;
            if (m != 0 && m != 4) continue;
            if (_word(cd, o) == h) return (true, o);
        }
        return (false, 0);
    }

    function _word(bytes memory b, uint256 off) internal pure returns (bytes32 w) {
        require(off + 32 <= b.length, "word out of range");
        assembly {
            w := mload(add(add(b, 0x20), off))
        }
    }

    /// Walk `code` as opcodes, stepping over PUSH immediates so that a byte
    /// of push data is never mistaken for an instruction.
    function _hasOpcode(bytes memory code, uint8 target) internal pure returns (bool) {
        return _countOpcode(code, target) > 0;
    }

    function _countOpcode(bytes memory code, uint8 target) internal pure returns (uint256 n) {
        uint256 i = 0;
        while (i < code.length) {
            uint8 op = uint8(code[i]);
            if (op == target) ++n;
            unchecked {
                ++i;
            }
            if (op >= 0x60 && op <= 0x7f) {
                unchecked {
                    i += uint256(op) - 0x5f;
                }
            }
        }
    }
}
