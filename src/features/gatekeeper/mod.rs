//! Native approval authority. UI decisions never carry executable inputs or wire IDs.
mod adapters;
mod broker;
pub mod mcp;
pub mod support;
mod types;
mod whitelist;

pub use broker::{Delivery, Gatekeeper, PendingReply};
pub use types::*;
pub use whitelist::{ExactRule, WHITELIST_COMMANDS};

#[cfg(test)]
mod tests;
