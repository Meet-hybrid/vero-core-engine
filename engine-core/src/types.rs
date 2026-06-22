use soroban_sdk::{contracttype, contracterror, Address, BytesN, Map, String, Symbol, Val};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum TreasuryError {
    InvalidBalance = 1,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum BurnError {
    ZeroAddress = 1,
}

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
    pub state_hash: BytesN<32>,
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
    pub context: Map<Symbol, Val>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Proposal {
    pub id: u64,
    pub proposer: soroban_sdk::Address,
    pub action_hash: soroban_sdk::BytesN<32>,
    pub approved_by: soroban_sdk::Vec<soroban_sdk::Address>,
    pub state: ProposalState,
    pub voting_deadline: u32,
}
