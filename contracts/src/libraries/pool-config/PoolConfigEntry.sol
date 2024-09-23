// SPDX-License-Identifier: MIT
pragma solidity ^0.8.13;

import {PartialKey} from "./PartialKey.sol";

/// @dev Packed `partialKey:u216 ++ tickSpacing:u16 ++ feeInE6:u24`
type PoolConfigEntry is uint256;

uint256 constant ENTRY_SIZE = 32;
uint256 constant KEY_MASK = 0xffffffffffffffffffffffffffffffffffffffffffffffffffffff0000000000;
uint256 constant TICK_SPACING_MASK = 0xffff;
uint256 constant TICK_SPACING_OFFSET = 24;
uint256 constant FEE_MASK = 0xffffff;
uint256 constant FEE_OFFSET = 0;

using PoolConfigEntryLib for PoolConfigEntry global;

/// @author philogy <https://github.com/philogy>
library PoolConfigEntryLib {
    function isEmpty(PoolConfigEntry self) internal pure returns (bool) {
        return PoolConfigEntry.unwrap(self) == 0;
    }

    function tickSpacing(PoolConfigEntry self) internal pure returns (int24 spacing) {
        assembly {
            spacing := and(TICK_SPACING_MASK, shr(TICK_SPACING_OFFSET, self))
        }
    }

    function feeInE6(PoolConfigEntry self) internal pure returns (uint24) {
        return uint24(PoolConfigEntry.unwrap(self) >> FEE_OFFSET);
    }
}
