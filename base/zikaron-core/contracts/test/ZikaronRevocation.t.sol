// SPDX-License-Identifier: CC0-1.0
pragma solidity 0.8.28;

import {Test} from "forge-std/Test.sol";
import {Vm} from "forge-std/Vm.sol";
import {ZikaronRevocation} from "../src/ZikaronRevocation.sol";

/// @dev A contract caller. Used to show that the contract charges its
/// immediate caller, and that a log so produced fails both laws' form tests,
/// whose topic 1 must be the *transaction's* sender.
contract RevokeForwarder {
    error Boom();

    ZikaronRevocation internal immutable REV;

    constructor(ZikaronRevocation r) {
        REV = r;
    }

    function forward(bytes32 e, bytes32 g) external {
        REV.revoke(e, g);
    }

    function forwardThenRevert(bytes32 e, bytes32 g) external {
        REV.revoke(e, g);
        revert Boom();
    }
}

/// @title The notice of withdrawal read by its two laws.
/// @notice Every assertion here is a clause of zikaron/2 §9.1 (the anchor
/// form, for the first log) or of zikaron.kit/2 §6.3 (the notice form, for the
/// second), or one of the properties both lean on: exactly two logs per call
/// and none otherwise, the caller's own address in topic 1 of each, the two
/// words at calldata offsets 4 and 36, and a contract that holds no state and
/// has no owner. Gas is recorded for the record and is asserted nowhere.
contract ZikaronRevocationTest is Test {
    // ---- pinned constants -------------------------------------------------

    /// keccak256("Anchored(address,bytes32)").
    bytes32 internal constant ANCHORED_TOPIC0 = 0xee1610244dfc4b116b1433dd4459fd7b1d3d2bf737a05fc3f6d8f173005d7147;
    /// keccak256("Revoked(address,bytes32)").
    bytes32 internal constant REVOKED_TOPIC0 = 0x6e70be4be1a4aebd688b5523bd8b6278acac3963d71ebf2bd5ea50757047664b;

    /// From CODEHASH.md, transcribed by hand. A drift in solc version,
    /// evm_version, optimizer, via_ir, bytecode_hash or cbor_metadata moves
    /// this and fails the suite.
    bytes32 internal constant PINNED_CODEHASH = 0x07a2fef228662aa2adc0a273a304ac4372898138ee1a8a8531760207988ddec2;
    uint256 internal constant PINNED_RUNTIME_LENGTH = 130;

    bytes4 internal constant SEL_REVOKE = bytes4(0xc2664610); // revoke(bytes32,bytes32)

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

    ZikaronRevocation internal rev;
    RevokeForwarder internal fwd;

    address internal alice = makeAddr("alice");
    address internal bob = makeAddr("bob");

    function setUp() public {
        rev = new ZikaronRevocation();
        fwd = new RevokeForwarder(rev);
    }

    // =======================================================================
    // 1. revoke(e, g): exactly two logs, the anchor then the notice
    // =======================================================================

    function test_Revoke_EmitsExactlyTwoWellShapedLogsInOrder() public {
        bytes32 e = keccak256("revocation entry");
        bytes32 g = keccak256("grant withdrawn");

        vm.recordLogs();
        vm.prank(alice);
        rev.revoke(e, g);
        Vm.Log[] memory logs = vm.getRecordedLogs();

        assertEq(logs.length, 2, "revoke must emit exactly two logs");
        _assertAnchorShape(logs[0], address(rev), alice, e);
        _assertNoticeShape(logs[1], address(rev), alice, g);
    }

    function test_Revoke_ZeroWords_AreStillLogged() public {
        vm.recordLogs();
        vm.prank(alice);
        rev.revoke(bytes32(0), bytes32(0));
        Vm.Log[] memory logs = vm.getRecordedLogs();

        assertEq(logs.length, 2, "the zero words get their logs like any other");
        _assertAnchorShape(logs[0], address(rev), alice, bytes32(0));
        _assertNoticeShape(logs[1], address(rev), alice, bytes32(0));
    }

    function test_Revoke_EqualWords_AreStillTwoLogs() public {
        bytes32 w = keccak256("one word twice");

        vm.recordLogs();
        vm.prank(alice);
        rev.revoke(w, w);
        Vm.Log[] memory logs = vm.getRecordedLogs();

        assertEq(logs.length, 2, "the contract compares nothing");
        _assertAnchorShape(logs[0], address(rev), alice, w);
        _assertNoticeShape(logs[1], address(rev), alice, w);
    }

    function testFuzz_Revoke_TopicsTrackTheInputs(address caller, bytes32 e, bytes32 g) public {
        vm.assume(caller != address(0));
        vm.assume(caller != address(vm)); // the cheatcode address cannot call out

        vm.recordLogs();
        vm.prank(caller);
        rev.revoke(e, g);
        Vm.Log[] memory logs = vm.getRecordedLogs();

        assertEq(logs.length, 2, "two logs per call, always");
        _assertAnchorShape(logs[0], address(rev), caller, e);
        _assertNoticeShape(logs[1], address(rev), caller, g);
    }

    function test_Revoke_TwiceInOneTransaction_FourLogsInOrder() public {
        bytes32 e1 = keccak256("e1");
        bytes32 g1 = keccak256("g1");
        bytes32 e2 = keccak256("e2");
        bytes32 g2 = keccak256("g2");

        vm.recordLogs();
        vm.startPrank(alice);
        rev.revoke(e1, g1);
        rev.revoke(e2, g2);
        vm.stopPrank();
        Vm.Log[] memory logs = vm.getRecordedLogs();

        assertEq(logs.length, 4, "two calls, four logs");
        _assertAnchorShape(logs[0], address(rev), alice, e1);
        _assertNoticeShape(logs[1], address(rev), alice, g1);
        _assertAnchorShape(logs[2], address(rev), alice, e2);
        _assertNoticeShape(logs[3], address(rev), alice, g2);
    }

    // =======================================================================
    // 2. Caller attribution
    // =======================================================================

    function test_TwoCallers_EachGetsItsOwnAddressInTopic1() public {
        bytes32 e = keccak256("shared entry");
        bytes32 g = keccak256("shared grant");

        vm.recordLogs();
        vm.prank(alice);
        rev.revoke(e, g);
        Vm.Log[] memory a = vm.getRecordedLogs();

        vm.recordLogs();
        vm.prank(bob);
        rev.revoke(e, g);
        Vm.Log[] memory b = vm.getRecordedLogs();

        assertEq(a.length, 2, "alice: two logs");
        assertEq(b.length, 2, "bob: two logs");
        _assertAnchorShape(a[0], address(rev), alice, e);
        _assertNoticeShape(a[1], address(rev), alice, g);
        _assertAnchorShape(b[0], address(rev), bob, e);
        _assertNoticeShape(b[1], address(rev), bob, g);
        assertTrue(a[1].topics[1] != b[1].topics[1], "two callers, two authors");
        assertEq(a[1].topics[2], b[1].topics[2], "the same grant for both");
    }

    /// A contract caller is charged as itself. The EOA behind it never
    /// appears, which is exactly why neither law reads such a log: zikaron/2
    /// §9.1 and zikaron.kit/2 §6.3 each require topic 1 to be the
    /// *transaction's* sender.
    function test_ContractCaller_IsChargedAsItself_NotTheEoaBehindIt() public {
        bytes32 e = keccak256("via forwarder e");
        bytes32 g = keccak256("via forwarder g");
        bytes memory txCalldata = abi.encodeWithSelector(RevokeForwarder.forward.selector, e, g);

        vm.recordLogs();
        vm.prank(alice, alice); // msg.sender and tx.origin both alice
        (bool ok,) = address(fwd).call(txCalldata);
        Vm.Log[] memory logs = vm.getRecordedLogs();

        assertTrue(ok, "the forward call succeeds");
        assertEq(logs.length, 2, "two logs");
        _assertAnchorShape(logs[0], address(rev), address(fwd), e);
        _assertNoticeShape(logs[1], address(rev), address(fwd), g);
        assertTrue(
            logs[1].topics[1] != bytes32(uint256(uint160(alice))), "the EOA behind the forwarder is not the author"
        );

        // The consequence under both laws: with alice as the transaction's
        // sender neither log is read, however well shaped.
        assertFalse(_isAnchorUnder91(1, address(rev), alice, txCalldata, logs[0]), "no anchor for the EOA");
        assertFalse(_isNoticeUnder63(1, address(rev), alice, txCalldata, logs[1]), "no notice for the EOA");
    }

    // =======================================================================
    // 3. Calldata containment: offsets 4 and 36
    // =======================================================================

    function test_Calldata_EntryAtOffsetFour_GrantAtOffsetThirtySix() public pure {
        bytes32 e = keccak256("offset four");
        bytes32 g = keccak256("offset thirty-six");
        bytes memory cd = abi.encodeWithSelector(SEL_REVOKE, e, g);

        assertEq(cd.length, 68, "selector plus two words");
        assertEq(_word(cd, 4), e, "the entry begins at byte 4");
        assertEq(_word(cd, 36), g, "the grant begins at byte 36");

        (bool fe, uint256 oe) = _carriedAtAlignedOffset(cd, e);
        assertTrue(fe, "the entry is carried at an aligned offset");
        assertEq(oe, 4, "4");
        (bool fg, uint256 og) = _carriedAtAlignedOffset(cd, g);
        assertTrue(fg, "the grant is carried at an aligned offset");
        assertEq(og, 36, "36, which is 4 plus 32");
        assertEq(og % 32, 4, "4 plus a multiple of 32");
    }

    /// The whole §9.1 test over the first log and the whole §6.3 test over
    /// the second, applied end to end to a real call.
    function test_BothForms_PassTheWholeTests() public {
        bytes32 e = keccak256("end to end e");
        bytes32 g = keccak256("end to end g");
        bytes memory cd = abi.encodeWithSelector(SEL_REVOKE, e, g);

        vm.recordLogs();
        vm.prank(alice, alice);
        (bool ok,) = address(rev).call(cd);
        Vm.Log[] memory logs = vm.getRecordedLogs();

        assertTrue(ok, "status 1");
        assertEq(logs.length, 2, "two logs");
        assertTrue(_isAnchorUnder91(1, address(rev), alice, cd, logs[0]), "the first log is an anchor under 9.1");
        assertTrue(_isNoticeUnder63(1, address(rev), alice, cd, logs[1]), "the second log is a notice under 6.3");
        // Each form reads its own topic 0: the other log is not of its form.
        assertFalse(_isAnchorUnder91(1, address(rev), alice, cd, logs[1]), "the notice is not an anchor");
        assertFalse(_isNoticeUnder63(1, address(rev), alice, cd, logs[0]), "the anchor is not a notice");
    }

    /// Each word is also carried where the other law looks: the entry sits
    /// at an offset §6.3 admits and the grant at one §9.1 admits. Neither law
    /// reads it there, since each binds a topic 2 of its own form to the
    /// calldata; the containment is one test and the topic 0 is what keeps
    /// the two logs apart.
    function test_Calldata_EitherWordIsCarriedForEitherTest() public pure {
        bytes32 e = keccak256("e");
        bytes32 g = keccak256("g");
        bytes memory cd = abi.encodeWithSelector(SEL_REVOKE, e, g);
        (bool fe,) = _carriedAtAlignedOffset(cd, e);
        (bool fg,) = _carriedAtAlignedOffset(cd, g);
        assertTrue(fe && fg, "both words are carried at aligned offsets");
    }

    /// The reference registry and this contract are two emitters: a reader
    /// that lists only the registry reads no anchor here, and a grant whose
    /// member names another contract takes no notice from this one.
    function test_UndeclaredEmitter_IsNeitherAnchorNorNotice() public {
        ZikaronRevocation other = new ZikaronRevocation();
        bytes32 e = keccak256("undeclared e");
        bytes32 g = keccak256("undeclared g");
        bytes memory cd = abi.encodeWithSelector(SEL_REVOKE, e, g);

        vm.recordLogs();
        vm.prank(alice, alice);
        (bool ok,) = address(other).call(cd);
        Vm.Log[] memory logs = vm.getRecordedLogs();

        assertTrue(ok, "the call succeeds");
        assertEq(logs.length, 2, "and emits two well shaped logs");
        assertEq(address(other).codehash, PINNED_CODEHASH, "byte-identical code");
        assertFalse(_isAnchorUnder91(1, address(rev), alice, cd, logs[0]), "an undeclared emitter anchors nothing");
        assertFalse(_isNoticeUnder63(1, address(rev), alice, cd, logs[1]), "an undeclared emitter notices nothing");
    }

    // =======================================================================
    // 4. No state, no owner, no other log
    // =======================================================================

    function test_Runtime_HasNoStateOpcodeAtAll() public view {
        bytes memory code = address(rev).code;
        assertGt(code.length, 0, "deployed");
        assertFalse(_hasOpcode(code, OP_SSTORE), "no SSTORE in the runtime bytecode");
        assertFalse(_hasOpcode(code, OP_SLOAD), "no SLOAD: nothing to read, so no owner check");
        assertFalse(_hasOpcode(code, OP_TSTORE), "no TSTORE");
        assertFalse(_hasOpcode(code, OP_TLOAD), "no TLOAD");
        assertFalse(_hasOpcode(code, OP_CREATE), "no CREATE");
        assertFalse(_hasOpcode(code, OP_CREATE2), "no CREATE2");
        assertFalse(_hasOpcode(code, OP_CALL), "no CALL: it reaches nothing");
        assertFalse(_hasOpcode(code, OP_DELEGATECALL), "no DELEGATECALL: no upgrade path");
        assertFalse(_hasOpcode(code, OP_SELFDESTRUCT), "no SELFDESTRUCT");
    }

    /// Three topics is structural: LOG3 is the only log opcode the runtime
    /// contains, twice, so no call path can produce a log of any other arity.
    function test_Runtime_ContainsOnlyTwoLog3() public view {
        bytes memory code = address(rev).code;
        assertEq(_countOpcode(code, OP_LOG3), 2, "two LOG3: the anchor and the notice");
        assertFalse(_hasOpcode(code, OP_LOG0), "no LOG0");
        assertFalse(_hasOpcode(code, OP_LOG1), "no LOG1");
        assertFalse(_hasOpcode(code, OP_LOG2), "no LOG2");
        assertFalse(_hasOpcode(code, OP_LOG4), "no LOG4");
    }

    function test_OpcodeWalk_SkipsPushImmediates() public pure {
        assertFalse(_hasOpcode(hex"6055", OP_SSTORE), "PUSH1 immediate is skipped");
        assertFalse(
            _hasOpcode(hex"7f0000000000000000000000000000000000000000000000000000000000000055" hex"00", OP_SSTORE),
            "PUSH32 immediate is skipped"
        );
        assertTrue(_hasOpcode(hex"55", OP_SSTORE), "a real SSTORE is found");
        assertTrue(_hasOpcode(hex"5f55", OP_SSTORE), "PUSH0 has no immediate");
        assertFalse(_hasOpcode(hex"7f00", OP_SSTORE), "truncated PUSH terminates the walk");
    }

    function test_NotPayable_RevokeWithValueReverts() public {
        vm.deal(alice, 1 ether);

        vm.recordLogs();
        vm.prank(alice);
        (bool ok, bytes memory ret) =
            address(rev).call{value: 1 wei}(abi.encodeWithSelector(SEL_REVOKE, keccak256("e"), keccak256("g")));
        Vm.Log[] memory logs = vm.getRecordedLogs();

        assertFalse(ok, "no payable function exists");
        assertEq(ret.length, 0, "a bare revert, no reason");
        assertEq(logs.length, 0, "the revert precedes the emits, so nothing is logged");
        assertEq(address(rev).balance, 0, "the contract holds nothing");
    }

    function test_PlainTransfer_Reverts() public {
        vm.deal(alice, 1 ether);
        vm.prank(alice);
        (bool ok,) = address(rev).call{value: 1 wei}("");
        assertFalse(ok, "no receive function");
        assertEq(address(rev).balance, 0, "the contract holds nothing");
    }

    function test_UnknownSelector_Reverts() public {
        bytes4[5] memory unknown = [
            bytes4(0xdeadbeef),
            bytes4(0x00000000),
            bytes4(0xffffffff),
            bytes4(keccak256("owner()")),
            bytes4(keccak256("anchor(bytes32)"))
        ];

        for (uint256 i = 0; i < unknown.length; ++i) {
            vm.recordLogs();
            (bool ok, bytes memory ret) = address(rev).call(abi.encodePacked(unknown[i], bytes32(0), bytes32(0)));
            Vm.Log[] memory logs = vm.getRecordedLogs();

            assertFalse(ok, "no function beyond revoke");
            assertEq(ret.length, 0, "a bare revert");
            assertEq(logs.length, 0, "and no log");
        }
    }

    function test_EmptyCalldata_Reverts() public {
        (bool ok,) = address(rev).call("");
        assertFalse(ok, "no fallback function");
    }

    function test_ShortCalldata_Reverts() public {
        (bool ok,) = address(rev).call(hex"c26646"); // three bytes of the selector
        assertFalse(ok, "a partial selector is not a call");
    }

    function test_Revoke_WithOneWord_Reverts() public {
        vm.recordLogs();
        (bool ok,) = address(rev).call(abi.encodePacked(SEL_REVOKE, keccak256("only one")));
        Vm.Log[] memory logs = vm.getRecordedLogs();
        assertFalse(ok, "revoke(bytes32,bytes32) needs both words");
        assertEq(logs.length, 0, "and logs nothing");
    }

    function test_Revoke_WithNoWord_Reverts() public {
        (bool ok,) = address(rev).call(abi.encodePacked(SEL_REVOKE));
        assertFalse(ok, "revoke(bytes32,bytes32) needs its words");
    }

    /// Extra calldata beyond the two words is ignored by the ABI decoder; the
    /// logs still carry the two words at offsets 4 and 36, and nothing else
    /// of the calldata reaches a topic.
    function test_Revoke_WithTrailingBytes_LogsTheTwoWords() public {
        bytes32 e = keccak256("e trailing");
        bytes32 g = keccak256("g trailing");
        bytes memory cd = abi.encodePacked(SEL_REVOKE, e, g, bytes32(uint256(0xBAD)));

        vm.recordLogs();
        vm.prank(alice, alice);
        (bool ok,) = address(rev).call(cd);
        Vm.Log[] memory logs = vm.getRecordedLogs();

        assertTrue(ok, "trailing calldata does not revert");
        assertEq(logs.length, 2, "two logs");
        _assertAnchorShape(logs[0], address(rev), alice, e);
        _assertNoticeShape(logs[1], address(rev), alice, g);
    }

    function testFuzz_UnknownSelector_Reverts(bytes4 sel) public {
        vm.assume(sel != SEL_REVOKE);

        vm.recordLogs();
        (bool ok,) = address(rev).call(abi.encodePacked(sel, bytes32(0), bytes32(0)));
        Vm.Log[] memory logs = vm.getRecordedLogs();

        assertFalse(ok, "one selector exists");
        assertEq(logs.length, 0, "and nothing else logs");
    }

    // =======================================================================
    // 5. Gas, for the record only
    // =======================================================================

    function test_Gas_Record() public {
        rev.revoke(keccak256("warm e"), keccak256("warm g"));
        uint256 g0 = gasleft();
        rev.revoke(keccak256("gas e"), keccak256("gas g"));
        uint256 used = g0 - gasleft();
        emit log_named_uint("gas revoke(bytes32,bytes32)", used);
    }

    // =======================================================================
    // 6. codeHash: a settings drift is a different contract
    // =======================================================================

    function test_CodeHash_MatchesCodehashMd() public view {
        assertEq(address(rev).codehash, PINNED_CODEHASH, "codeHash from CODEHASH.md");
    }

    function test_RuntimeLength_MatchesCodehashMd() public view {
        assertEq(address(rev).code.length, PINNED_RUNTIME_LENGTH, "130 bytes of runtime");
    }

    function test_CodeHash_IsKeccakOfTheRuntimeBytecode() public view {
        assertEq(keccak256(address(rev).code), PINNED_CODEHASH, "codeHash is keccak of the runtime");
    }

    function test_EveryDeploymentHasTheSameCodeHash() public {
        ZikaronRevocation a = new ZikaronRevocation();
        ZikaronRevocation b = new ZikaronRevocation();
        assertTrue(address(a) != address(b), "two addresses");
        assertEq(address(a).codehash, PINNED_CODEHASH, "same code");
        assertEq(address(b).codehash, PINNED_CODEHASH, "same code");
    }

    function test_Selectors_MatchTheInterfaceCodehashMdNames() public pure {
        assertEq(bytes32(ZikaronRevocation.revoke.selector), bytes32(SEL_REVOKE), "revoke(bytes32,bytes32)");
        assertEq(keccak256("Anchored(address,bytes32)"), ANCHORED_TOPIC0, "anchor topic 0");
        assertEq(keccak256("Revoked(address,bytes32)"), REVOKED_TOPIC0, "notice topic 0");
    }

    // =======================================================================
    // 7. A reverted transaction anchors and notices nothing
    // =======================================================================

    /// The forwarder emits and then reverts. Under both laws the receipt
    /// carries status 0, so no log in it is an anchor or a notice.
    /// `vm.recordLogs()` is not receipt-faithful here (see FINDINGS.md): it
    /// keeps logs emitted inside a frame that later reverted, which no
    /// receipt ever carries. The assertion therefore runs each law's status
    /// gate over what was recorded.
    function test_RevertedTransaction_AnchorsAndNoticesNothing() public {
        bytes32 e = keccak256("doomed e");
        bytes32 g = keccak256("doomed g");
        bytes memory txCalldata = abi.encodeWithSelector(RevokeForwarder.forwardThenRevert.selector, e, g);

        vm.recordLogs();
        vm.prank(alice, alice);
        (bool ok,) = address(fwd).call(txCalldata);
        Vm.Log[] memory logs = vm.getRecordedLogs();

        assertFalse(ok, "the transaction reverts, so its receipt status is 0");

        uint8 status = ok ? 1 : 0;
        uint256 anchors = 0;
        uint256 notices = 0;
        for (uint256 i = 0; i < logs.length; ++i) {
            if (_isAnchorUnder91(status, address(rev), alice, txCalldata, logs[i])) ++anchors;
            if (_isNoticeUnder63(status, address(rev), alice, txCalldata, logs[i])) ++notices;
        }
        assertEq(anchors, 0, "a reverted transaction anchors nothing");
        assertEq(notices, 0, "a reverted transaction notices nothing");
        assertEq(address(rev).codehash, PINNED_CODEHASH, "the contract is unchanged");
    }

    function test_SuccessfulSibling_StillAnchorsAndNotices() public {
        bytes32 e = keccak256("survives e");
        bytes32 g = keccak256("survives g");
        bytes memory cd = abi.encodeWithSelector(SEL_REVOKE, e, g);

        vm.recordLogs();
        vm.prank(alice, alice);
        (bool ok,) = address(rev).call(cd);
        Vm.Log[] memory logs = vm.getRecordedLogs();

        assertTrue(ok, "status 1");
        assertEq(logs.length, 2, "two logs");
        assertTrue(_isAnchorUnder91(1, address(rev), alice, cd, logs[0]), "the anchor is an anchor");
        assertTrue(_isNoticeUnder63(1, address(rev), alice, cd, logs[1]), "the notice is a notice");
    }

    // =======================================================================
    // helpers
    // =======================================================================

    /// The zikaron/2 §9.1 anchor-form test, whole.
    function _isAnchorUnder91(
        uint8 receiptStatus,
        address declaredRegistry,
        address txSender,
        bytes memory txCalldata,
        Vm.Log memory lg
    ) internal pure returns (bool) {
        return _formTest(receiptStatus, declaredRegistry, txSender, txCalldata, lg, ANCHORED_TOPIC0);
    }

    /// The zikaron.kit/2 §6.3 notice-form test, whole.
    function _isNoticeUnder63(
        uint8 receiptStatus,
        address declaredContract,
        address txSender,
        bytes memory txCalldata,
        Vm.Log memory lg
    ) internal pure returns (bool) {
        return _formTest(receiptStatus, declaredContract, txSender, txCalldata, lg, REVOKED_TOPIC0);
    }

    function _formTest(
        uint8 receiptStatus,
        address declared,
        address txSender,
        bytes memory txCalldata,
        Vm.Log memory lg,
        bytes32 topic0
    ) internal pure returns (bool) {
        if (receiptStatus != 1) return false;
        if (lg.emitter != declared) return false;
        if (lg.topics.length != 3) return false;
        if (lg.data.length != 0) return false;
        if (lg.topics[0] != topic0) return false;
        if (uint256(lg.topics[1]) >> 160 != 0) return false; // 12 zero bytes
        if (address(uint160(uint256(lg.topics[1]))) != txSender) return false;
        (bool carried,) = _carriedAtAlignedOffset(txCalldata, lg.topics[2]);
        return carried;
    }

    function _assertAnchorShape(Vm.Log memory lg, address emitter, address author, bytes32 h) internal pure {
        _assertShape(lg, emitter, author, h, ANCHORED_TOPIC0, "anchor");
    }

    function _assertNoticeShape(Vm.Log memory lg, address emitter, address author, bytes32 g) internal pure {
        _assertShape(lg, emitter, author, g, REVOKED_TOPIC0, "notice");
    }

    function _assertShape(Vm.Log memory lg, address emitter, address author, bytes32 w, bytes32 topic0, string memory what)
        internal
        pure
    {
        assertEq(lg.emitter, emitter, string.concat(what, ": emitter"));
        assertEq(lg.topics.length, 3, string.concat(what, ": exactly three topics"));
        assertEq(lg.data.length, 0, string.concat(what, ": empty data"));
        assertEq(lg.topics[0], topic0, string.concat(what, ": topic 0"));
        assertEq(uint256(lg.topics[1]) >> 160, 0, string.concat(what, ": topic 1 has twelve leading zero bytes"));
        assertEq(lg.topics[1], bytes32(uint256(uint160(author))), string.concat(what, ": topic 1 is the author"));
        assertEq(lg.topics[2], w, string.concat(what, ": topic 2 is the word"));
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
