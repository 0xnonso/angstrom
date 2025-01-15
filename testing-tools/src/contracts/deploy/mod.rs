use alloy::primitives::{address, keccak256, Address, Bytes, B256, U160, U256};
use create3::calc_addr_with_bytes;

// use super::environment::{ANGSTROM_ADDRESS, ANGSTROM_ADDRESS_SALT};

pub mod angstrom;
pub mod mockreward;
pub mod tokens;
pub mod uniswap_flags;

const DEFAULT_CREATE2_FACTORY: Address = address!("4e59b44847b379578588920cA78FbF26c0B4956C");

/// Attempt to find a target address that includes the appropriate flags
/// Returns the address found and the salt needed to pad the initcode to
/// deploy to that address
pub fn mine_address(
    deployer: Address,
    flags: U160,
    mask: U160,
    initcode: &Bytes
) -> (Address, U256) {
    mine_address_with_factory(deployer, DEFAULT_CREATE2_FACTORY, flags, mask, initcode)
}

pub fn mine_address_with_factory(
    deployer: Address,
    factory: Address,
    flags: U160,
    mask: U160,
    initcode: &Bytes
) -> (Address, U256) {
    let init_code_hash = keccak256(initcode);
    let mut salt = U256::ZERO;
    let mut counter: u128 = 0;
    loop {
        let target_address: Address = factory.create2(B256::from(salt), init_code_hash);
        let u_address: U160 = target_address.into();
        if (u_address & mask) == flags {
            break;
        }
        salt += U256::from(1_u8);
        counter += 1;
        if counter > 100_000 {
            panic!("We tried this too many times!")
        }
    }
    // let final_address = factory.create2(B256::from(salt), init_code_hash);
    //let salt = U256::from(crate::contracts::environment::ANGSTROM_ADDRESS_SALT);
    let final_address =
        calc_addr_with_bytes(&**DEFAULT_CREATE2_FACTORY, &salt.to_le_bytes()).into();
    // (address.into(), salt)
    (final_address, salt)
    // (
    //     crate::contracts::environment::ANGSTROM_ADDRESS,
    //     U256::from(crate::contracts::environment::ANGSTROM_ADDRESS_SALT)
    // )
}

#[cfg(test)]
mod tests {
    use super::uniswap_flags::UniswapFlags;

    #[test]
    fn test_deploy_addresses() {
        let flags = UniswapFlags::BeforeSwap
            | UniswapFlags::BeforeInitialize
            | UniswapFlags::BeforeAddLiquidity
            | UniswapFlags::BeforeRemoveLiquidity;
    }
}
