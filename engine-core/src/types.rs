use soroban_sdk::{contracttype, Address, BytesN, Map, String, Symbol, Vec};

#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ProposalState {
    Pending = 0,
    Approved = 1,
    Executed = 2,
    Expired = 3,
}

#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum BreakerState {
    Closed = 0,
    Open = 1,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StateCommitment {
    pub sequence: u64,
    pub state_hash: soroban_sdk::BytesN<32>,
    pub ledger: u32,
    pub author: Address,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TreasurySnapshot {
    pub id: u64,
    pub total_balance: i128,
    pub account_count: u32,
    pub ledger: u32,
    pub timestamp: String,
    pub state_hash: BytesN<32>,
    pub triggered_by: String,
    pub context: Map<Symbol, soroban_sdk::Val>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Proposal {
    pub id: u64,
    pub proposer: Address,
    pub action_hash: BytesN<32>,
    pub approved_by: Vec<Address>,
    pub state: u32,
    pub voting_deadline: u32,
}
