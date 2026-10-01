// SPDX-License-Identifier: CC0-1.0
pragma solidity 0.8.28;

/// @title MockERC20: a minimal token contract for testing how the app reads token-denominated escrow.
/// @notice The app reads exactly two things off a token contract (`crates/app/src/chainx.rs`,
/// the closed selector pair): `balanceOf(address)` and `allowance(address,address)`. This
/// contract answers both, plus the two writes a test needs to set up state: `mint` to put a
/// balance on an escrow address and `approve` to set an allowance.
///
/// A local anvil chain has no ERC-20 deployed, so without this contract a test could only
/// reach the refusal ("the token balance does not read as a number") and never the
/// affirmative case (a bond whose amount really is read off a token contract at a pinned
/// block).
///
/// `mint` takes a full `uint256` on purpose: the app's word reader refuses a balance above
/// `u128::MAX` rather than truncating it (`chainx::word_value` requires the high sixteen
/// bytes to be zero), so both sides of that boundary have to be reachable from here.
///
/// It is a mock: no transfers, no supply accounting, no access control. The shipped app
/// never deploys it or depends on it; tests deploy it on a throwaway local chain.
contract MockERC20 {
    mapping(address => uint256) public balanceOf;
    mapping(address => mapping(address => uint256)) public allowance;

    /// @notice Put `amount` on `who`. Whatever was there before is replaced.
    function mint(address who, uint256 amount) external {
        balanceOf[who] = amount;
    }

    /// @notice Let `spender` draw `amount` from the caller.
    function approve(address spender, uint256 amount) external returns (bool) {
        allowance[msg.sender][spender] = amount;
        return true;
    }
}
