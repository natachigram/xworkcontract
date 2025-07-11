use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
use cosmwasm_std::{coins, Uint128};

use xworks_freelance_contract::contract::execute;
use xworks_freelance_contract::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use xworks_freelance_contract::state::JobStatus;
use xworks_freelance_contract::ContractError;

const ADMIN: &str = "admin";
const CLIENT: &str = "client";
const FREELANCER: &str = "freelancer";

fn init() -> (DepsMutMock, EnvMock) {
    let mut deps = mock_dependencies();
    let env = mock_env();
    let info = mock_info(ADMIN, &[]);
    let msg = InstantiateMsg {
        admin: Some(ADMIN.to_string()),
        platform_fee_percent: Some(5),
        min_escrow_amount: Some(Uint128::new(1000)),
        dispute_period_days: Some(7),
        max_job_duration_days: Some(365),
    };
    execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();
    (deps, env)
}

#[test]
fn bulk_post_jobs() {
    let (mut deps, env) = init();
    let client_info = mock_info(CLIENT, &[]);
    for i in 0..20u8 {
        let msg = ExecuteMsg::PostJob {
            title: format!("Job {}", i),
            description: "Performance test job".to_string(),
            budget: Uint128::new(5000),
            category: "Dev".to_string(),
            skills_required: vec!["Rust".to_string()],
            duration_days: 10,
            documents: None,
            milestones: None,
        };
        let res = execute(deps.as_mut(), env.clone(), client_info.clone(), msg).unwrap();
        assert!(!res.attributes.is_empty());
    }
}

#[test]
fn bulk_submit_proposals() {
    let (mut deps, env) = init();
    let client_info = mock_info(CLIENT, &[]);
    // Post one job
    let post = ExecuteMsg::PostJob {
        title: "Perf Job".to_string(),
        description: "Performance test job".to_string(),
        budget: Uint128::new(5000),
        category: "Dev".to_string(),
        skills_required: vec!["Rust".to_string()],
        duration_days: 10,
        documents: None,
        milestones: None,
    };
    execute(deps.as_mut(), env.clone(), client_info.clone(), post).unwrap();

    for i in 0..20u8 {
        let freelancer_info = mock_info(FREELANCER, &[]);
        let msg = ExecuteMsg::SubmitProposal {
            job_id: 0,
            cover_letter: format!("Proposal {}", i),
            delivery_time_days: 5,
            milestones: None,
        };
        let res = execute(deps.as_mut(), env.clone(), freelancer_info.clone(), msg).unwrap();
        assert!(!res.attributes.is_empty());
    }
}
