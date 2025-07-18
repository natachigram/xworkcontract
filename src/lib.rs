pub mod admin_management;
pub mod bounty_management;
pub mod category_skill_manager;
pub mod contract;
pub mod contract_helpers;
pub mod error;
pub mod escrow;
pub mod escrow_management;
pub mod hash_utils;
pub mod helpers;
pub mod job_management;
pub mod msg;
pub mod query_helpers;
pub mod security;
pub mod state;
pub mod user_management;

pub use crate::error::ContractError;
pub use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};

// Re-export helper functions for use in modules
pub use crate::helpers::{
    ensure_not_paused, get_future_timestamp, validate_budget, validate_duration,
    validate_job_description, validate_job_title,
};
pub use crate::security::{
    check_rate_limit, reentrancy_guard, validate_text_inputs, RateLimitAction,
};
// Note: macros with #[macro_export] are automatically available at crate root

#[cfg(not(feature = "library"))]
pub use crate::contract::{execute, instantiate, query};
