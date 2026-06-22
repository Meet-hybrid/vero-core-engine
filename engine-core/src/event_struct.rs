use soroban_sdk::{contracttype, BytesN, Map, Symbol, Val};

#[contracttype]
#[derive(Clone, Debug)]
pub struct Event {
    /// Short identifier of the event source (e.g., "AUDIT", "TREASURY").
    pub event_type: BytesN<32>,
    /// Action name within the source (e.g., "commit", "snapshot").
    pub action: BytesN<32>,
    /// Arbitrary payload data attached to the event.
    pub payload: Map<Symbol, Val>,
}
