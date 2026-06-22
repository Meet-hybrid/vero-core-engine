use crate::event_utils::publish_event;
use crate::types::{Proposal, ProposalState};
use soroban_sdk::{
    contracterror, panic_with_error, symbol_short, token, vec, Address, BytesN, Env, IntoVal, Map,
    Symbol, Vec,
};

const KEY_PROPOSALS:  Symbol = symbol_short!("PROPS");
const KEY_SIGNERS:    Symbol = symbol_short!("SIGNERS");
const KEY_THRESH:     Symbol = symbol_short!("THRESH");
const KEY_MIN_STAKE:  Symbol = symbol_short!("MINSTAKE");
const KEY_STAKE_TOK:  Symbol = symbol_short!("STKTOK");
const TIMELOCK_LEDGERS: u32 = 720;
const MAX_THRESHOLD: u32 = 100;
const MAX_DURATION_LEDGERS: u32 = 5256000; // ~10 years
const MIN_DURATION_LEDGERS: u32 = 1;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum GovError {
    NotASigner = 1,
    AlreadyApproved = 2,
    ProposalNotFound = 3,
    InvalidStateTransition = 4,
    TimelockActive = 5,
    InsufficientStake = 6,
    InvalidThreshold = 7,
    InvalidStake = 8,
}

pub fn init(
    env: &Env,
    signers: Vec<Address>,
    threshold: u32,
    stake_token: Address,
    min_stake: i128,
) {
    if threshold == 0 || threshold > signers.len() || threshold > MAX_THRESHOLD {
        panic_with_error!(env, GovError::InvalidThreshold);
    }
    if min_stake < 0 {
        panic_with_error!(env, GovError::InvalidStake);
    }
    env.storage().instance().set(&KEY_SIGNERS, &signers);
    env.storage().instance().set(&KEY_THRESH, &threshold);
    env.storage().instance().set(&KEY_STAKE_TOK, &stake_token);
    env.storage().instance().set(&KEY_MIN_STAKE, &min_stake);
    let empty: Map<u64, (Proposal, u32)> = Map::new(env);
    env.storage().instance().set(&KEY_PROPOSALS, &empty);
}

pub fn propose(
    env: &Env,
    proposer: &Address,
    action_hash: BytesN<32>,
    duration_ledgers: u32,
) -> u64 {
    if duration_ledgers < MIN_DURATION_LEDGERS || duration_ledgers > MAX_DURATION_LEDGERS {
        panic_with_error!(env, GovError::InvalidStateTransition);
    }

    let current_ledger = env.ledger().sequence();
    let voting_deadline = current_ledger + duration_ledgers;

    let mut props: Map<u64, (Proposal, u32)> = env
        .storage()
        .instance()
        .get(&KEY_PROPOSALS)
        .unwrap_or(Map::new(env));

    let next_id = (props.len() as u64) + 1;

    let proposal = Proposal {
        id: next_id,
        proposer: proposer.clone(),
        action_hash,
        approved_by: Vec::new(env),
        state: ProposalState::Pending,
        voting_deadline,
    };

    let unlock_ledger = current_ledger + TIMELOCK_LEDGERS;
    props.set(proposal.id, (proposal.clone(), unlock_ledger));
    env.storage().instance().set(&KEY_PROPOSALS, &props);

    env.events().publish(
        (symbol_short!("GOV"), symbol_short!("propose")),
        proposal.id,
    );
    let mut payload = Map::new(env);
    payload.set(Symbol::new(env, "proposal_id"), proposal.id.into_val(env));
    publish_event(
        env,
        BytesN::from_array(env, &[0u8; 32]),
        BytesN::from_array(env, &[0u8; 32]),
        payload,
    );
    proposal.id
}

pub fn approve(env: &Env, voter: &Address, proposal_id: u64) {
    voter.require_auth();
    require_signer(env, voter);

    let mut props: Map<u64, (Proposal, u32)> = env
        .storage()
        .instance()
        .get(&KEY_PROPOSALS)
        .unwrap_or_else(|| panic_with_error!(env, GovError::ProposalNotFound));

    let threshold: u32 = env.storage().instance().get(&KEY_THRESH).unwrap_or(1);
    let min_stake: i128 = env.storage().instance().get(&KEY_MIN_STAKE).unwrap_or(0);
    let stake_token: Address = env.storage().instance().get(&KEY_STAKE_TOK).unwrap();

    let (mut prop, unlock) = props.get(proposal_id).unwrap_or_else(|| {
        panic_with_error!(env, GovError::ProposalNotFound)
    });

    if prop.state != ProposalState::Pending {
        panic_with_error!(env, GovError::InvalidStateTransition);
    }
    if prop.approved_by.contains(voter) {
        panic_with_error!(env, GovError::AlreadyApproved);
    }

    if min_stake > 0 {
        let balance = token::Client::new(env, &stake_token).balance(voter);
        if balance < min_stake {
            panic_with_error!(env, GovError::InsufficientStake);
        }
    }

    prop.approved_by.push_back(voter.clone());

    if prop.approved_by.len() >= threshold {
        prop.state = ProposalState::Approved;
        env.events().publish(
            (symbol_short!("GOV"), symbol_short!("approved")),
            proposal_id,
        );
        let mut payload = Map::new(env);
        payload.set(Symbol::new(env, "proposal_id"), proposal_id.into_val(env));
        publish_event(
            env,
            BytesN::from_array(env, &[0u8; 32]),
            BytesN::from_array(env, &[0u8; 32]),
            payload,
        );
    }

    props.set(proposal_id, (prop.clone(), unlock));
    env.storage().instance().set(&KEY_PROPOSALS, &props);

    if prop.state == ProposalState::Approved && env.ledger().sequence() >= unlock {
        execute(env, proposal_id);
    }
}

pub fn execute(env: &Env, proposal_id: u64) -> Proposal {
    let mut props: Map<u64, (Proposal, u32)> = env
        .storage()
        .instance()
        .get(&KEY_PROPOSALS)
        .unwrap_or_else(|| panic_with_error!(env, GovError::ProposalNotFound));

    let (mut prop, unlock) = props.get(proposal_id).unwrap_or_else(|| {
        panic_with_error!(env, GovError::ProposalNotFound)
    });

    if prop.state != ProposalState::Approved {
        panic_with_error!(env, GovError::InvalidStateTransition);
    }
    if env.ledger().sequence() < unlock {
        panic_with_error!(env, GovError::TimelockActive);
    }

    prop.state = ProposalState::Executed;
    props.set(proposal_id, (prop.clone(), unlock));
    env.storage().instance().set(&KEY_PROPOSALS, &props);

    env.events().publish(
        (symbol_short!("GOV"), symbol_short!("execute")),
        proposal_id,
    );
    let mut payload = Map::new(env);
    payload.set(Symbol::new(env, "proposal_id"), proposal_id.into_val(env));
    publish_event(
        env,
        BytesN::from_array(env, &[0u8; 32]),
        BytesN::from_array(env, &[0u8; 32]),
        payload,
    );
    prop
}

fn require_signer(env: &Env, addr: &Address) {
    let signers: Vec<Address> = env
        .storage()
        .instance()
        .get(&KEY_SIGNERS)
        .unwrap_or(vec![env]);
    if !signers.contains(addr) {
        panic_with_error!(env, GovError::NotASigner);
    }
}
