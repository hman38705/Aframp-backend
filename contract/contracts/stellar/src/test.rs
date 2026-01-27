#![cfg(test)]

use soroban_sdk::{
    Env,
    Address,
    testutils::{Address as _, Ledger},
};

use afri_stablecoin::AfrIContract;
use afri_stablecoin::AfrIContractClient;

#[test]
fn mint_transfer_burn_flow_works() {
    let env = Env::default();

    // Enable test auth + ledger
    env.mock_all_auths();
    env.ledger().with_mut(|li| {
        li.timestamp = 1;
    });

    // Test addresses
    let admin = Address::generate(&env);
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);

    // Deploy contract
    let contract_id = env.register_contract(None, AfrIContract);
    let client = AfrIContractClient::new(&env, &contract_id);

    // Initialize
    client.init(&admin);

    // Mint
    client.mint(&user1, &100);
    assert_eq!(client.balance(&user1), 100);

    // Transfer
    client.transfer(&user1, &user2, &40);
    assert_eq!(client.balance(&user1), 60);
    assert_eq!(client.balance(&user2), 40);

    // Burn
    client.burn(&user2, &20);
    assert_eq!(client.balance(&user2), 20);
}

#[test]
#[should_panic]
fn transfer_fails_on_insufficient_balance() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);

    let contract_id = env.register_contract(None, AfrIContract);
    let client = AfrIContractClient::new(&env, &contract_id);

    client.init(&admin);

    // No mint → should fail
    client.transfer(&user1, &user2, &10);
}

#[test]
#[should_panic]
fn mint_fails_for_non_admin() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let attacker = Address::generate(&env);

    let contract_id = env.register_contract(None, AfrIContract);
    let client = AfrIContractClient::new(&env, &contract_id);

    client.init(&admin);

    // Attacker tries to mint
    client.mint(&attacker, &100);
}
