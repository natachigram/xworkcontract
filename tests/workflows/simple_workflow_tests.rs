use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
use cosmwasm_std::{coins, Uint128};

use xworks_freelance_contract::contract::{execute, instantiate};
use xworks_freelance_contract::msg::{ExecuteMsg, InstantiateMsg};
use xworks_freelance_contract::state::{JobStatus, JOBS};

const ADMIN: &str = "admin";
const CLIENT: &str = "client";
const FREELANCER: &str = "freelancer";

mod workflow_integration_tests {
    use super::*;

    #[test]
    fn test_complete_job_workflow() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        
        // Setup contract
        let info = mock_info(ADMIN, &[]);
        let msg = InstantiateMsg {
            admin: Some(ADMIN.to_string()),
            platform_fee_percent: Some(5),
            min_escrow_amount: Some(Uint128::new(1000)),
            dispute_period_days: Some(7),
            max_job_duration_days: Some(365),
        };
        instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();

        // 1. Client posts a job
        let client_info = mock_info(CLIENT, &coins(5000, "uxion"));
        let job_msg = ExecuteMsg::PostJob {
            title: "Build a DApp".to_string(),
            description: "Build a decentralized application".to_string(),
            budget: Uint128::new(5000),
            category: "Development".to_string(),
            skills_required: vec!["Rust".to_string(), "CosmWasm".to_string()],
            duration_days: 30,
            documents: None,
            milestones: None,
        };
        execute(deps.as_mut(), env.clone(), client_info.clone(), job_msg).unwrap();

        // 2. Freelancer submits proposal
        let freelancer_info = mock_info(FREELANCER, &[]);
        let proposal_msg = ExecuteMsg::SubmitProposal {
            job_id: 0,
            cover_letter: "I can build this DApp for you".to_string(),
            delivery_time_days: 25,
            milestones: None,
        };
        execute(deps.as_mut(), env.clone(), freelancer_info.clone(), proposal_msg).unwrap();

        // 3. Client accepts proposal
        let accept_msg = ExecuteMsg::AcceptProposal {
            job_id: 0,
            proposal_id: 0,
        };
        execute(deps.as_mut(), env.clone(), client_info.clone(), accept_msg).unwrap();

        // 4. Create escrow
        let create_escrow_msg = ExecuteMsg::CreateEscrowNative {
            job_id: 0,
            amount: Uint128::new(5000),
        };
        execute(deps.as_mut(), env.clone(), client_info.clone(), create_escrow_msg).unwrap();

        // 5. Freelancer completes job
        let complete_msg = ExecuteMsg::CompleteJob {
            job_id: 0,
        };
        execute(deps.as_mut(), env.clone(), freelancer_info, complete_msg).unwrap();

        // 6. Verify job status
        let job = JOBS.load(deps.as_ref().storage, 0).unwrap();
        assert_eq!(job.status, JobStatus::Completed);

        // Note: Escrow is automatically released when job is completed
        // No need to manually release escrow
    }

    #[test]
    fn test_free_project_workflow() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        
        // Setup contract
        let info = mock_info(ADMIN, &[]);
        let msg = InstantiateMsg {
            admin: Some(ADMIN.to_string()),
            platform_fee_percent: Some(5),
            min_escrow_amount: Some(Uint128::new(1000)),
            dispute_period_days: Some(7),
            max_job_duration_days: Some(365),
        };
        instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();

        // 1. Client posts a free job
        let client_info = mock_info(CLIENT, &[]);
        let job_msg = ExecuteMsg::PostJob {
            title: "Open Source Contribution".to_string(),
            description: "Help with documentation".to_string(),
            budget: Uint128::zero(), // Free project
            category: "Documentation".to_string(),
            skills_required: vec!["Writing".to_string()],
            duration_days: 7,
            documents: None,
            milestones: None,
        };
        execute(deps.as_mut(), env.clone(), client_info.clone(), job_msg).unwrap();

        // 2. Freelancer submits proposal
        let freelancer_info = mock_info(FREELANCER, &[]);
        let proposal_msg = ExecuteMsg::SubmitProposal {
            job_id: 0,
            cover_letter: "I'd be happy to help with documentation".to_string(),
            delivery_time_days: 5,
            milestones: None,
        };
        execute(deps.as_mut(), env.clone(), freelancer_info.clone(), proposal_msg).unwrap();

        // 3. Client accepts proposal
        let accept_msg = ExecuteMsg::AcceptProposal {
            job_id: 0,
            proposal_id: 0,
        };
        execute(deps.as_mut(), env.clone(), client_info.clone(), accept_msg).unwrap();

        // 4. Verify no escrow can be created for free projects
        let create_escrow_msg = ExecuteMsg::CreateEscrowNative {
            job_id: 0,
            amount: Uint128::zero(),
        };
        let result = execute(deps.as_mut(), env.clone(), client_info.clone(), create_escrow_msg);
        assert!(result.is_err()); // Should fail for free projects

        // 5. Freelancer completes job
        let complete_msg = ExecuteMsg::CompleteJob {
            job_id: 0,
        };
        execute(deps.as_mut(), env.clone(), freelancer_info, complete_msg).unwrap();

        // 6. Verify job status
        let job = JOBS.load(deps.as_ref().storage, 0).unwrap();
        assert_eq!(job.status, JobStatus::Completed);
        assert!(job.escrow_id.is_none()); // No escrow for free projects
    }
}
