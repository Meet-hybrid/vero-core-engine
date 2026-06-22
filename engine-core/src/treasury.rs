//! Treasury state snapshots for audit history.
//!
//! Records treasury state at critical points (deposits, withdrawals, governance actions)
//! to enable audit trails and historical analysis. Snapshots are immutable once recorded.
//!
//! Storage Layout:
//!   SNAP_COUNTER    → Current snapshot ID (incremental)
//!   SNAP:<id>       → TreasurySnapshot indexed by ID
//!   SNAP:LATEST     → Most recent snapshot ID

use soroban_sdk::{contracterror, panic_with_error, symbol_short, BytesN, Bytes, Env, Map, String, Symbol, Vec};
use crate::event_utils::publish_event;
use soroban_sdk::IntoVal;

use crate::types::TreasurySnapshot;

const KEY_SNAP_COUNTER: Symbol = symbol_short!("SNAPC");
const KEY_SNAP_LATEST:  Symbol = symbol_short!("SNAPL");

/// Maximum allowed total balance for a snapshot (prevents overflow/DoS).
const MAX_SNAPSHOT_BALANCE: i128 = 1_000_000_000_000_000_000_000_000_000_000_000;
/// Maximum allowed account count in a snapshot.
const MAX_ACCOUNT_COUNT: u32 = 1_000_000_000;
/// Maximum number of recent snapshots a caller may request in one call.
const MAX_RECENT_SNAPSHOTS: u32 = 1000;
/// Minimum snapshot ID for audit trail iteration.
const MIN_SNAPSHOT_ID: u64 = 1;

#[contracterror]
#[derive(Copy, Clone)]
pub enum TreasuryError {
    SnapshotNotFound = 1,
    InvalidBalance   = 2,
    InvalidAccountCount = 3,
    InvalidCount = 4,
}

/// Initialize treasury snapshot system. Called once at contract deployment.
pub fn init(env: &Env) {
    env.storage().instance().set(&KEY_SNAP_COUNTER, &0u64);
    env.storage().instance().set(&KEY_SNAP_LATEST, &0u64);
}

/// Record a treasury snapshot. Called after state-changing operations.
///
/// Returns the snapshot ID for reference.
pub fn record_snapshot(
    env: &Env,
    total_balance: i128,
    account_count: u32,
    triggered_by: String,
    context: Map<Symbol, soroban_sdk::Val>,
) -> u64 {
    if total_balance < 0 {
        panic_with_error!(env, TreasuryError::InvalidBalance);
    }
    if total_balance > MAX_SNAPSHOT_BALANCE {
        panic_with_error!(env, TreasuryError::InvalidBalance);
    }
    if account_count == 0 || account_count > MAX_ACCOUNT_COUNT {
        panic_with_error!(env, TreasuryError::InvalidAccountCount);
    }

    let counter: u64 = env.storage().instance().get(&KEY_SNAP_COUNTER).unwrap_or(0);
    let snapshot_id = counter
        .checked_add(1)
        .unwrap_or_else(|| panic_with_error!(env, TreasuryError::SnapshotNotFound));

    let state_hash = compute_hash(env, total_balance, account_count, env.ledger().sequence());

    // Convert ledger timestamp to its decimal string without format!.
    let ts = env.ledger().timestamp();
    let ts_str = u64_to_string(env, ts);

    let snapshot = TreasurySnapshot {
        id: snapshot_id,
        total_balance,
        account_count,
        ledger: env.ledger().sequence(),
        timestamp: ts_str,
        state_hash,
        triggered_by,
        context,
    };

    let snapshot_key = make_snap_key(env, snapshot_id);
    env.storage().instance().set(&snapshot_key, &snapshot);
    env.storage().instance().set(&KEY_SNAP_COUNTER, &snapshot_id);
    env.storage().instance().set(&KEY_SNAP_LATEST, &snapshot_id);

    env.events().publish(
        (symbol_short!("TRE"), symbol_short!("snapshot")),
        snapshot_id,
    );
    // Emit structured Event for treasury snapshot
    let mut payload = Map::new(env);
    payload.set(symbol_short!("id"), snapshot_id.into_val(env));
    payload.set(symbol_short!("bal"), total_balance.into_val(env));
    payload.set(symbol_short!("acc"), account_count.into_val(env));
    payload.set(symbol_short!("ldg"), env.ledger().sequence().into_val(env));
    publish_event(env, BytesN::from_array(env, &[0u8; 32]), BytesN::from_array(env, &[0u8; 32]), payload);

    snapshot_id
}

/// Retrieve a snapshot by ID.
pub fn get_snapshot(env: &Env, snapshot_id: u64) -> Option<TreasurySnapshot> {
    let key = make_snap_key(env, snapshot_id);
    env.storage().instance().get(&key)
}

/// Get the most recent snapshot.
pub fn get_latest_snapshot(env: &Env) -> Option<TreasurySnapshot> {
    let latest_id: u64 = env.storage().instance().get(&KEY_SNAP_LATEST).unwrap_or(0);
    if latest_id == 0 { return None; }
    get_snapshot(env, latest_id)
}

/// Get snapshot count.
pub fn snapshot_count(env: &Env) -> u64 {
    env.storage().instance().get(&KEY_SNAP_COUNTER).unwrap_or(0)
}

/// Get IDs of the most recent `count` snapshots (newest first).
/// `count` is clamped to `MAX_RECENT_SNAPSHOTS` to prevent DoS.
pub fn get_recent_snapshots(env: &Env, count: u32) -> Vec<u64> {
    let total = snapshot_count(env);
    if total == 0 {
        return Vec::new(env);
    }
    let clamped = (count as u64).min(MAX_RECENT_SNAPSHOTS as u64);
    let start = if total > clamped {
        total - clamped + 1
    } else {
        MIN_SNAPSHOT_ID
    };
    let mut result = Vec::new(env);
    for id in (start..=total).rev() {
        result.push_back(id);
    }
    result
}

/// Verify snapshot integrity by recomputing the hash.
pub fn verify_snapshot(env: &Env, snapshot: &TreasurySnapshot) -> bool {
    let recomputed = compute_hash(env, snapshot.total_balance, snapshot.account_count, snapshot.ledger);
    snapshot.state_hash == recomputed
}

/// Retrieve all snapshots from `from_id` onward (audit trail).
/// `from_id` is clamped to `MIN_SNAPSHOT_ID` to prevent DoS iteration.
pub fn audit_trail(env: &Env, from_id: u64) -> Vec<TreasurySnapshot> {
    let total = snapshot_count(env);
    if total == 0 {
        return Vec::new(env);
    }
    let start = from_id.max(MIN_SNAPSHOT_ID);
    let mut result = Vec::new(env);
    for id in start..=total {
        if let Some(snap) = get_snapshot(env, id) {
            result.push_back(snap);
        }
    }
    result
}

// ── internal ──────────────────────────────────────────────────────────────────

/// Convert a u64 to its decimal string representation (no_std safe).
fn u64_to_string(env: &Env, val: u64) -> String {
    if val == 0 {
        return String::from_str(env, "0");
    }
    // 20 digits max for u64
    let mut buf = [0u8; 20];
    let mut n = val;
    let mut i = 20;
    while n > 0 {
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
    }
    String::from_str(env, core::str::from_utf8(&buf[i..]).unwrap_or("0"))
}

fn compute_hash(env: &Env, balance: i128, account_count: u32, ledger: u32) -> BytesN<32> {
    // Pack fields into a fixed-size byte buffer for deterministic hashing.
    // Layout: balance(16) | account_count(4) | ledger(4) = 24 bytes
    let mut raw = [0u8; 24];
    raw[..16].copy_from_slice(&balance.to_be_bytes());
    raw[16..20].copy_from_slice(&account_count.to_be_bytes());
    raw[20..24].copy_from_slice(&ledger.to_be_bytes());
    env.crypto().sha256(&Bytes::from_slice(env, &raw)).into()
}

fn make_snap_key(env: &Env, id: u64) -> Symbol {
    Symbol::new(env, &alloc::format!("S{}", id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{Env, Map, String, Symbol};

    #[soroban_sdk::contract]
    pub struct TestContract;

    #[soroban_sdk::contractimpl]
    impl TestContract {}

    #[test]
    fn snapshot_creation_and_retrieval() {
        let env = Env::default();
        let contract_id = env.register_contract(None, TestContract);
        env.as_contract(&contract_id, || {
            init(&env);
            let ctx: Map<Symbol, soroban_sdk::Val> = Map::new(&env);
            let id = record_snapshot(&env, 1000, 5, String::from_str(&env, "deposit"), ctx);
            assert_eq!(id, 1);
            let snap = get_snapshot(&env, 1).unwrap();
            assert_eq!(snap.total_balance, 1000);
            assert_eq!(snap.account_count, 5);
            assert_eq!(snapshot_count(&env), 1);
        });
    }

    #[test]
    fn snapshot_hash_verification() {
        let env = Env::default();
        let contract_id = env.register_contract(None, TestContract);
        env.as_contract(&contract_id, || {
            init(&env);
            let ctx: Map<Symbol, soroban_sdk::Val> = Map::new(&env);
            record_snapshot(&env, 500, 2, String::from_str(&env, "withdrawal"), ctx);
            let snap = get_snapshot(&env, 1).unwrap();
            assert!(verify_snapshot(&env, &snap));
        });
    }

    #[test]
    #[should_panic]
    fn negative_balance_rejected() {
        let env = Env::default();
        let contract_id = env.register_contract(None, TestContract);
        env.as_contract(&contract_id, || {
            init(&env);
            let ctx: Map<Symbol, soroban_sdk::Val> = Map::new(&env);
            record_snapshot(&env, -1, 0, String::from_str(&env, "bad"), ctx);
        });
    }
}
