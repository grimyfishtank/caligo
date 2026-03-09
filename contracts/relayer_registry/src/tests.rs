use soroban_sdk::{testutils::Address as _, Address, Env, String};

use crate::{RelayerRegistry, RelayerRegistryClient};

fn setup() -> (Env, RelayerRegistryClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(RelayerRegistry, ());
    let client = RelayerRegistryClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    (env, client, admin)
}

#[test]
fn test_initialize_success() {
    let (_, client, admin) = setup();
    client.initialize(&admin, &100);
    assert_eq!(client.get_max_fee(), 100);
    assert_eq!(client.get_relayer_count(), 0);
}

#[test]
#[should_panic(expected = "Error(Contract, #1)")]
fn test_initialize_twice() {
    let (_, client, admin) = setup();
    client.initialize(&admin, &100);
    client.initialize(&admin, &100);
}

#[test]
fn test_register_relayer() {
    let (env, client, admin) = setup();
    client.initialize(&admin, &100);

    let relayer = Address::generate(&env);
    let endpoint = String::from_str(&env, "https://relay.example.com");
    client.register(&relayer, &endpoint, &50);

    assert_eq!(client.get_relayer_count(), 1);
    let info = client.get_relayer(&relayer).unwrap();
    assert_eq!(info.fee_bps, 50);
    assert_eq!(info.active, true);
}

#[test]
#[should_panic(expected = "Error(Contract, #2)")]
fn test_register_fee_too_high() {
    let (env, client, admin) = setup();
    client.initialize(&admin, &100);

    let relayer = Address::generate(&env);
    let endpoint = String::from_str(&env, "https://relay.example.com");
    client.register(&relayer, &endpoint, &200); // exceeds max 100
}

#[test]
#[should_panic(expected = "Error(Contract, #3)")]
fn test_register_empty_endpoint() {
    let (env, client, admin) = setup();
    client.initialize(&admin, &100);

    let relayer = Address::generate(&env);
    let endpoint = String::from_str(&env, "");
    client.register(&relayer, &endpoint, &50);
}

#[test]
fn test_update_existing_relayer() {
    let (env, client, admin) = setup();
    client.initialize(&admin, &100);

    let relayer = Address::generate(&env);
    let ep1 = String::from_str(&env, "https://v1.relay.example.com");
    let ep2 = String::from_str(&env, "https://v2.relay.example.com");

    client.register(&relayer, &ep1, &50);
    client.register(&relayer, &ep2, &75);

    // Count should not increase on update
    assert_eq!(client.get_relayer_count(), 1);
    let info = client.get_relayer(&relayer).unwrap();
    assert_eq!(info.fee_bps, 75);
}

#[test]
fn test_deactivate_by_relayer() {
    let (env, client, admin) = setup();
    client.initialize(&admin, &100);

    let relayer = Address::generate(&env);
    let endpoint = String::from_str(&env, "https://relay.example.com");
    client.register(&relayer, &endpoint, &50);

    client.deactivate(&relayer, &relayer);
    let info = client.get_relayer(&relayer).unwrap();
    assert_eq!(info.active, false);
}

#[test]
fn test_deactivate_by_admin() {
    let (env, client, admin) = setup();
    client.initialize(&admin, &100);

    let relayer = Address::generate(&env);
    let endpoint = String::from_str(&env, "https://relay.example.com");
    client.register(&relayer, &endpoint, &50);

    client.deactivate(&admin, &relayer);
    let info = client.get_relayer(&relayer).unwrap();
    assert_eq!(info.active, false);
}

#[test]
#[should_panic(expected = "Error(Contract, #5)")]
fn test_deactivate_unauthorized() {
    let (env, client, admin) = setup();
    client.initialize(&admin, &100);

    let relayer = Address::generate(&env);
    let other = Address::generate(&env);
    let endpoint = String::from_str(&env, "https://relay.example.com");
    client.register(&relayer, &endpoint, &50);

    client.deactivate(&other, &relayer); // not admin or relayer
}

#[test]
fn test_reactivate() {
    let (env, client, admin) = setup();
    client.initialize(&admin, &100);

    let relayer = Address::generate(&env);
    let endpoint = String::from_str(&env, "https://relay.example.com");
    client.register(&relayer, &endpoint, &50);

    client.deactivate(&relayer, &relayer);
    assert_eq!(client.get_relayer(&relayer).unwrap().active, false);

    client.reactivate(&relayer);
    assert_eq!(client.get_relayer(&relayer).unwrap().active, true);
}

#[test]
fn test_get_active_relayers() {
    let (env, client, admin) = setup();
    client.initialize(&admin, &100);

    let r1 = Address::generate(&env);
    let r2 = Address::generate(&env);
    let r3 = Address::generate(&env);

    client.register(&r1, &String::from_str(&env, "https://r1.com"), &10);
    client.register(&r2, &String::from_str(&env, "https://r2.com"), &20);
    client.register(&r3, &String::from_str(&env, "https://r3.com"), &30);

    // All 3 active
    let active = client.get_active_relayers();
    assert_eq!(active.len(), 3);

    // Deactivate r2
    client.deactivate(&r2, &r2);
    let active = client.get_active_relayers();
    assert_eq!(active.len(), 2);
}

#[test]
fn test_set_max_fee() {
    let (_, client, admin) = setup();
    client.initialize(&admin, &100);
    assert_eq!(client.get_max_fee(), 100);

    client.set_max_fee(&admin, &200);
    assert_eq!(client.get_max_fee(), 200);
}

#[test]
#[should_panic(expected = "Error(Contract, #5)")]
fn test_set_max_fee_unauthorized() {
    let (env, client, admin) = setup();
    client.initialize(&admin, &100);

    let other = Address::generate(&env);
    client.set_max_fee(&other, &200);
}

#[test]
#[should_panic(expected = "Error(Contract, #4)")]
fn test_deactivate_nonexistent() {
    let (env, client, admin) = setup();
    client.initialize(&admin, &100);

    let fake = Address::generate(&env);
    client.deactivate(&admin, &fake);
}

#[test]
fn test_get_nonexistent_relayer() {
    let (env, client, admin) = setup();
    client.initialize(&admin, &100);

    let fake = Address::generate(&env);
    assert!(client.get_relayer(&fake).is_none());
}
