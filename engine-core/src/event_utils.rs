use soroban_sdk::{Env, symbol_short, BytesN, Map, Symbol, Val};
use crate::event_struct::Event;

pub fn publish_event(env: &Env, event_type: BytesN<32>, action: BytesN<32>, payload: Map<Symbol, Val>) {
    let ev = Event {
        event_type,
        action,
        payload,
    };
    // Emit the event with a generic identifier.
    env.events().publish((symbol_short!("EVENT"), symbol_short!("LOG")), ev);
}
