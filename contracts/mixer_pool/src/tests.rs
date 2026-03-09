use crate::{MixerPool, MixerPoolClient};
use soroban_sdk::{
    testutils::Address as _,
    token::{Client as TokenClient, StellarAssetClient},
    Address, BytesN, Env,
};

/// Standard test denomination: 100 XLM in stroops
const DENOMINATION: i128 = 1_000_000_000;
/// Standard max fee: 1 XLM in stroops
const MAX_FEE: i128 = 10_000_000;
/// Tree depth for tests (small for speed)
const TEST_TREE_DEPTH: u32 = 4;
/// Root history size for tests
const TEST_ROOT_HISTORY: u32 = 10;

struct TestContext {
    env: Env,
    contract_addr: Address,
    token_addr: Address,
    admin: Address,
    depositor: Address,
}

impl TestContext {
    fn client(&self) -> MixerPoolClient<'_> {
        MixerPoolClient::new(&self.env, &self.contract_addr)
    }

    fn token_client(&self) -> TokenClient<'_> {
        TokenClient::new(&self.env, &self.token_addr)
    }

    fn sac_client(&self) -> StellarAssetClient<'_> {
        StellarAssetClient::new(&self.env, &self.token_addr)
    }

    fn commitment(&self, seed: u8) -> BytesN<32> {
        let mut bytes = [0u8; 32];
        bytes[0] = seed;
        for i in 1..32 {
            bytes[i] = seed.wrapping_mul(i as u8).wrapping_add(0x42);
        }
        BytesN::from_array(&self.env, &bytes)
    }
}

fn setup() -> TestContext {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let contract_addr = env.register(MixerPool, ());

    let token_admin = Address::generate(&env);
    let token_contract = env.register_stellar_asset_contract_v2(token_admin.clone());
    let token_addr = token_contract.address();

    let client = MixerPoolClient::new(&env, &contract_addr);
    client.initialize(
        &admin,
        &token_addr,
        &DENOMINATION,
        &MAX_FEE,
        &TEST_TREE_DEPTH,
        &TEST_ROOT_HISTORY,
    );

    let depositor = Address::generate(&env);
    let sac = StellarAssetClient::new(&env, &token_addr);
    sac.mint(&depositor, &(DENOMINATION * 10));

    TestContext {
        env,
        contract_addr,
        token_addr,
        admin,
        depositor,
    }
}

// ── Initialization Tests ──

#[test]
fn test_initialize_success() {
    let ctx = setup();
    let client = ctx.client();

    assert_eq!(client.get_denomination(), DENOMINATION);
    assert_eq!(client.get_deposit_count(), 0);

    let roots = client.get_root_history();
    assert_eq!(roots.len(), 1); // Initial empty root
}

#[test]
#[should_panic(expected = "Error(Contract, #1)")]
fn test_initialize_twice_panics() {
    let ctx = setup();
    let client = ctx.client();

    client.initialize(
        &ctx.admin,
        &ctx.token_addr,
        &DENOMINATION,
        &MAX_FEE,
        &TEST_TREE_DEPTH,
        &TEST_ROOT_HISTORY,
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #9)")]
fn test_initialize_zero_denomination() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let contract_addr = env.register(MixerPool, ());
    let client = MixerPoolClient::new(&env, &contract_addr);
    let token_admin = Address::generate(&env);
    let token = env.register_stellar_asset_contract_v2(token_admin).address();

    client.initialize(&admin, &token, &0, &MAX_FEE, &TEST_TREE_DEPTH, &TEST_ROOT_HISTORY);
}

#[test]
#[should_panic(expected = "Error(Contract, #10)")]
fn test_initialize_zero_depth() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let contract_addr = env.register(MixerPool, ());
    let client = MixerPoolClient::new(&env, &contract_addr);
    let token_admin = Address::generate(&env);
    let token = env.register_stellar_asset_contract_v2(token_admin).address();

    client.initialize(&admin, &token, &DENOMINATION, &MAX_FEE, &0, &TEST_ROOT_HISTORY);
}

#[test]
#[should_panic(expected = "Error(Contract, #11)")]
fn test_initialize_zero_history_size() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let contract_addr = env.register(MixerPool, ());
    let client = MixerPoolClient::new(&env, &contract_addr);
    let token_admin = Address::generate(&env);
    let token = env.register_stellar_asset_contract_v2(token_admin).address();

    client.initialize(&admin, &token, &DENOMINATION, &MAX_FEE, &TEST_TREE_DEPTH, &0);
}

// ── Deposit Tests ──

#[test]
fn test_deposit_success() {
    let ctx = setup();
    let client = ctx.client();

    client.deposit(&ctx.depositor, &ctx.commitment(1));
    assert_eq!(client.get_deposit_count(), 1);

    let balance = ctx.token_client().balance(&ctx.depositor);
    assert_eq!(balance, DENOMINATION * 10 - DENOMINATION);
}

#[test]
fn test_deposit_updates_root() {
    let ctx = setup();
    let client = ctx.client();

    let root_before = client.get_root();
    client.deposit(&ctx.depositor, &ctx.commitment(1));
    let root_after = client.get_root();

    assert_ne!(root_before, root_after);
}

#[test]
fn test_deposit_adds_to_root_history() {
    let ctx = setup();
    let client = ctx.client();

    client.deposit(&ctx.depositor, &ctx.commitment(1));

    let roots = client.get_root_history();
    assert_eq!(roots.len(), 2); // initial + 1 deposit
}

#[test]
#[should_panic(expected = "Error(Contract, #3)")]
fn test_deposit_duplicate_commitment() {
    let ctx = setup();
    let client = ctx.client();

    let c = ctx.commitment(1);
    client.deposit(&ctx.depositor, &c);
    client.deposit(&ctx.depositor, &c); // duplicate
}

#[test]
fn test_multiple_deposits() {
    let ctx = setup();
    let client = ctx.client();

    for i in 1..=5u8 {
        client.deposit(&ctx.depositor, &ctx.commitment(i));
    }

    assert_eq!(client.get_deposit_count(), 5);
    let roots = client.get_root_history();
    assert_eq!(roots.len(), 6); // initial + 5
}

#[test]
fn test_tree_capacity() {
    let ctx = setup();
    let client = ctx.client();

    // Fund enough for 16 deposits (depth 4 = 16 leaves)
    ctx.sac_client().mint(&ctx.depositor, &(DENOMINATION * 20));

    for i in 0..16u8 {
        client.deposit(&ctx.depositor, &ctx.commitment(i + 100));
    }

    assert_eq!(client.get_deposit_count(), 16);
}

#[test]
#[should_panic(expected = "Error(Contract, #4)")]
fn test_tree_full_rejects_deposit() {
    let ctx = setup();
    let client = ctx.client();

    ctx.sac_client().mint(&ctx.depositor, &(DENOMINATION * 20));

    // Fill tree (16 leaves for depth 4)
    for i in 0..16u8 {
        client.deposit(&ctx.depositor, &ctx.commitment(i + 50));
    }

    // 17th deposit should fail
    client.deposit(&ctx.depositor, &ctx.commitment(200));
}

// ── Root History Tests ──

#[test]
fn test_root_history_circular_buffer() {
    let ctx = setup();
    let client = ctx.client();

    // Fund for 12 deposits (history size is 10, so it wraps around)
    ctx.sac_client().mint(&ctx.depositor, &(DENOMINATION * 15));

    for i in 0..12u8 {
        client.deposit(&ctx.depositor, &ctx.commitment(i + 30));
    }

    let roots = client.get_root_history();
    assert_eq!(roots.len(), TEST_ROOT_HISTORY as u32);
}

// ── Withdraw Validation Tests (Phase 1 — proof always rejected) ──

#[test]
#[should_panic(expected = "Error(Contract, #5)")]
fn test_withdraw_invalid_root() {
    let ctx = setup();
    let client = ctx.client();

    client.deposit(&ctx.depositor, &ctx.commitment(1));

    let fake_root = BytesN::from_array(&ctx.env, &[0xFFu8; 32]);
    let nullifier = ctx.commitment(99);
    let recipient = Address::generate(&ctx.env);
    let relayer = Address::generate(&ctx.env);
    let proof = BytesN::from_array(&ctx.env, &[0u8; 256]);

    client.withdraw(&proof, &fake_root, &nullifier, &recipient, &relayer, &0);
}

#[test]
#[should_panic(expected = "Error(Contract, #8)")]
fn test_withdraw_fee_too_high() {
    let ctx = setup();
    let client = ctx.client();

    client.deposit(&ctx.depositor, &ctx.commitment(1));

    let root = client.get_root();
    let nullifier = ctx.commitment(99);
    let recipient = Address::generate(&ctx.env);
    let relayer = Address::generate(&ctx.env);
    let proof = BytesN::from_array(&ctx.env, &[0u8; 256]);

    client.withdraw(&proof, &root, &nullifier, &recipient, &relayer, &(MAX_FEE + 1));
}

#[test]
#[should_panic(expected = "Error(Contract, #8)")]
fn test_withdraw_negative_fee() {
    let ctx = setup();
    let client = ctx.client();

    client.deposit(&ctx.depositor, &ctx.commitment(1));

    let root = client.get_root();
    let nullifier = ctx.commitment(99);
    let recipient = Address::generate(&ctx.env);
    let relayer = Address::generate(&ctx.env);
    let proof = BytesN::from_array(&ctx.env, &[0u8; 256]);

    client.withdraw(&proof, &root, &nullifier, &recipient, &relayer, &(-1));
}

// ── Nullifier Tests ──

#[test]
fn test_nullifier_not_spent_initially() {
    let ctx = setup();
    let client = ctx.client();

    assert!(!client.is_nullifier_spent(&ctx.commitment(1)));
}

// ── Query Tests ──

#[test]
fn test_get_denomination() {
    let ctx = setup();
    assert_eq!(ctx.client().get_denomination(), DENOMINATION);
}

#[test]
fn test_get_root_returns_value() {
    let ctx = setup();
    let _root = ctx.client().get_root();
}

#[test]
fn test_deposit_count_increments() {
    let ctx = setup();
    let client = ctx.client();

    assert_eq!(client.get_deposit_count(), 0);
    client.deposit(&ctx.depositor, &ctx.commitment(1));
    assert_eq!(client.get_deposit_count(), 1);
    client.deposit(&ctx.depositor, &ctx.commitment(2));
    assert_eq!(client.get_deposit_count(), 2);
}

// ── Merkle Tree Property Tests ──

#[test]
fn test_each_deposit_produces_unique_root() {
    let ctx = setup();
    let client = ctx.client();

    let root0 = client.get_root();
    client.deposit(&ctx.depositor, &ctx.commitment(1));
    let root1 = client.get_root();
    client.deposit(&ctx.depositor, &ctx.commitment(2));
    let root2 = client.get_root();

    assert_ne!(root0, root1);
    assert_ne!(root1, root2);
    assert_ne!(root0, root2);
}

#[test]
fn test_same_commitment_order_produces_same_root() {
    // Two pools with the same deposits in the same order should have the same root
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let token = env.register_stellar_asset_contract_v2(token_admin.clone()).address();
    let depositor = Address::generate(&env);
    let sac = StellarAssetClient::new(&env, &token);
    sac.mint(&depositor, &(DENOMINATION * 20));

    // Pool A
    let pool_a = env.register(MixerPool, ());
    let client_a = MixerPoolClient::new(&env, &pool_a);
    client_a.initialize(&admin, &token, &DENOMINATION, &MAX_FEE, &TEST_TREE_DEPTH, &TEST_ROOT_HISTORY);

    // Pool B — separate contract, same parameters
    let pool_b = env.register(MixerPool, ());
    let client_b = MixerPoolClient::new(&env, &pool_b);
    client_b.initialize(&admin, &token, &DENOMINATION, &MAX_FEE, &TEST_TREE_DEPTH, &TEST_ROOT_HISTORY);

    let c1 = BytesN::from_array(&env, &[1u8; 32]);
    let c2 = BytesN::from_array(&env, &[2u8; 32]);

    client_a.deposit(&depositor, &c1);
    client_a.deposit(&depositor, &c2);

    client_b.deposit(&depositor, &c1);
    client_b.deposit(&depositor, &c2);

    assert_eq!(client_a.get_root(), client_b.get_root());
}

// ── Address Encoding Tests ──

#[test]
fn test_address_to_field_bytes_nonzero() {
    let ctx = setup();
    let addr = Address::generate(&ctx.env);
    let result = crate::address_to_field_bytes(&ctx.env, &addr);
    assert_ne!(result, [0u8; 32], "address_to_field_bytes must not return zeros");
}

#[test]
fn test_address_to_field_bytes_deterministic() {
    let ctx = setup();
    let addr = Address::generate(&ctx.env);
    let r1 = crate::address_to_field_bytes(&ctx.env, &addr);
    let r2 = crate::address_to_field_bytes(&ctx.env, &addr);
    assert_eq!(r1, r2);
}

#[test]
fn test_address_to_field_bytes_different_addrs() {
    let ctx = setup();
    let addr1 = Address::generate(&ctx.env);
    let addr2 = Address::generate(&ctx.env);
    let r1 = crate::address_to_field_bytes(&ctx.env, &addr1);
    let r2 = crate::address_to_field_bytes(&ctx.env, &addr2);
    assert_ne!(r1, r2, "different addresses must produce different field elements");
}
