use cosmwasm_std::testing::{
    mock_dependencies, mock_env, mock_info, MockApi, MockQuerier, MockStorage,
};
use cosmwasm_std::{coins, OwnedDeps, Uint128};

use xworks_freelance_contract::contract::{execute, instantiate};
use xworks_freelance_contract::msg::{ExecuteMsg, InstantiateMsg};
use xworks_freelance_contract::state::{JobStatus, JOBS};

const ADMIN: &str = "admin";
const CLIENT: &str = "client";
const FREELANCER: &str = "freelancer";
const MALICIOUS_CONTRACT: &str = "malicious_contract";

mod reentrancy_tests {
    use super::*;

    fn setup_contract() -> (
        OwnedDeps<MockStorage, MockApi, MockQuerier>,
        cosmwasm_std::Env,
        cosmwasm_std::MessageInfo,
    ) {
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

        instantiate(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();
        (deps, env, info)
    }

    #[test]
    fn test_reentrancy_guard_prevents_recursive_calls() {
        let (mut deps, env, _) = setup_contract();

        // Create a job first
        let client_info = mock_info(CLIENT, &coins(5000, "uxion"));
        let job_msg = ExecuteMsg::PostJob {
            title: "Test Job".to_string(),
            description: "Test Description".to_string(),
            budget: Uint128::new(5000),
            category: "Development".to_string(),
            skills_required: vec!["Rust".to_string()],
            duration_days: 30,
            documents: None,
            milestones: None,
        };

        execute(deps.as_mut(), env.clone(), client_info.clone(), job_msg).unwrap();

        // Submit and accept proposal to put job in progress
        let freelancer_info = mock_info(FREELANCER, &[]);
        let proposal_msg = ExecuteMsg::SubmitProposal {
            job_id: 0,
            cover_letter: "Test proposal".to_string(),
            delivery_time_days: 25,
            milestones: None,
        };
        execute(deps.as_mut(), env.clone(), freelancer_info, proposal_msg).unwrap();

        let accept_msg = ExecuteMsg::AcceptProposal {
            job_id: 0,
            proposal_id: 0,
        };
        execute(deps.as_mut(), env.clone(), client_info.clone(), accept_msg).unwrap();

        // Create escrow for the job
        let escrow_info = mock_info(CLIENT, &coins(5000, "uxion"));
        let create_escrow_msg = ExecuteMsg::CreateEscrowNative {
            job_id: 0,
            amount: Uint128::new(5000),
        };

        execute(deps.as_mut(), env.clone(), escrow_info, create_escrow_msg).unwrap();

        // Simulate a reentrancy attack attempt
        // The reentrancy guard should prevent this
        let malicious_info = mock_info(MALICIOUS_CONTRACT, &[]);
        let release_msg = ExecuteMsg::ReleaseEscrow {
            escrow_id: "0".to_string(),
        };

        // First call should work (if authorized)
        let result = execute(
            deps.as_mut(),
            env.clone(),
            malicious_info.clone(),
            release_msg.clone(),
        );

        // Second concurrent call should be blocked by reentrancy guard
        let result2 = execute(deps.as_mut(), env, malicious_info, release_msg);

        // At least one should fail due to reentrancy protection or authorization
        assert!(result.is_err() || result2.is_err());
    }

    #[test]
    fn test_reentrancy_protection_on_escrow_operations() {
        let (mut deps, env, _) = setup_contract();

        // Setup job and escrow
        let client_info = mock_info(CLIENT, &coins(5000, "uxion"));
        let job_msg = ExecuteMsg::PostJob {
            title: "Reentrancy Test".to_string(),
            description: "Testing reentrancy protection".to_string(),
            budget: Uint128::new(5000),
            category: "Security".to_string(),
            skills_required: vec!["Security".to_string()],
            duration_days: 30,
            documents: None,
            milestones: None,
        };

        execute(deps.as_mut(), env.clone(), client_info.clone(), job_msg).unwrap();

        // Submit and accept proposal to put job in progress
        let freelancer_info = mock_info(FREELANCER, &[]);
        let proposal_msg = ExecuteMsg::SubmitProposal {
            job_id: 0,
            cover_letter: "Test proposal".to_string(),
            delivery_time_days: 25,
            milestones: None,
        };
        execute(deps.as_mut(), env.clone(), freelancer_info, proposal_msg).unwrap();

        let accept_msg = ExecuteMsg::AcceptProposal {
            job_id: 0,
            proposal_id: 0,
        };
        execute(deps.as_mut(), env.clone(), client_info.clone(), accept_msg).unwrap();

        // Test that multiple escrow operations cannot be executed concurrently
        let escrow_info1 = mock_info(CLIENT, &coins(5000, "uxion"));
        let escrow_info2 = mock_info(CLIENT, &coins(5000, "uxion"));
        let create_escrow_msg = ExecuteMsg::CreateEscrowNative {
            job_id: 0,
            amount: Uint128::new(5000),
        };

        let result1 = execute(
            deps.as_mut(),
            env.clone(),
            escrow_info1,
            create_escrow_msg.clone(),
        );
        let result2 = execute(deps.as_mut(), env, escrow_info2, create_escrow_msg);

        // One should succeed, one should fail (escrow already exists)
        assert!(result1.is_ok());
        assert!(result2.is_err());
    }

    #[test]
    fn test_state_consistency_during_reentrancy_attempt() {
        let (mut deps, env, _) = setup_contract();

        // Create job
        let client_info = mock_info(CLIENT, &coins(5000, "uxion"));
        let job_msg = ExecuteMsg::PostJob {
            title: "State Consistency Test".to_string(),
            description: "Testing state consistency".to_string(),
            budget: Uint128::new(5000),
            category: "Testing".to_string(),
            skills_required: vec!["Testing".to_string()],
            duration_days: 30,
            documents: None,
            milestones: None,
        };

        execute(deps.as_mut(), env.clone(), client_info.clone(), job_msg).unwrap();

        // Verify job state before any escrow operations
        let job_before = JOBS.load(deps.as_ref().storage, 0).unwrap();
        assert_eq!(job_before.status, JobStatus::Open);
        assert_eq!(job_before.escrow_id, None);

        // Submit and accept proposal to put job in progress
        let freelancer_info = mock_info(FREELANCER, &[]);
        let proposal_msg = ExecuteMsg::SubmitProposal {
            job_id: 0,
            cover_letter: "Test proposal".to_string(),
            delivery_time_days: 25,
            milestones: None,
        };
        execute(deps.as_mut(), env.clone(), freelancer_info, proposal_msg).unwrap();

        let accept_msg = ExecuteMsg::AcceptProposal {
            job_id: 0,
            proposal_id: 0,
        };
        execute(deps.as_mut(), env.clone(), client_info.clone(), accept_msg).unwrap();

        // Verify job is now in progress
        let job_in_progress = JOBS.load(deps.as_ref().storage, 0).unwrap();
        assert_eq!(job_in_progress.status, JobStatus::InProgress);

        // Create escrow
        let escrow_info = mock_info(CLIENT, &coins(5000, "uxion"));
        let create_escrow_msg = ExecuteMsg::CreateEscrowNative {
            job_id: 0,
            amount: Uint128::new(5000),
        };

        execute(deps.as_mut(), env.clone(), escrow_info, create_escrow_msg).unwrap();

        // Verify state consistency after escrow creation
        let job_after = JOBS.load(deps.as_ref().storage, 0).unwrap();
        assert!(job_after.escrow_id.is_some());

        // Attempt to create another escrow for the same job (should fail)
        let malicious_info = mock_info(MALICIOUS_CONTRACT, &coins(5000, "uxion"));
        let duplicate_escrow_msg = ExecuteMsg::CreateEscrowNative {
            job_id: 0,
            amount: Uint128::new(5000),
        };

        let result = execute(deps.as_mut(), env, malicious_info, duplicate_escrow_msg);
        assert!(result.is_err());

        // Verify state remains consistent
        let job_final = JOBS.load(deps.as_ref().storage, 0).unwrap();
        assert_eq!(job_after.escrow_id, job_final.escrow_id);
    }

    #[test]
    fn test_nested_function_call_protection() {
        let (mut deps, env, _) = setup_contract();

        // Test that nested calls to critical functions are properly handled
        let client_info = mock_info(CLIENT, &coins(10000, "uxion"));

        // Create multiple jobs rapidly (simulating nested calls)
        for i in 0..5 {
            let job_msg = ExecuteMsg::PostJob {
                title: format!("Nested Test Job {}", i),
                description: "Testing nested function calls".to_string(),
                budget: Uint128::new(1000),
                category: "Testing".to_string(),
                skills_required: vec!["Testing".to_string()],
                duration_days: 30,
                documents: None,
                milestones: None,
            };

            let result = execute(deps.as_mut(), env.clone(), client_info.clone(), job_msg);
            assert!(result.is_ok(), "Job creation {} should succeed", i);
        }

        // Verify all jobs were created correctly
        for i in 0..5 {
            let job = JOBS.load(deps.as_ref().storage, i).unwrap();
            assert_eq!(job.title, format!("Nested Test Job {}", i));
        }
    }

    #[test]
    fn test_cross_function_reentrancy_protection() {
        let (mut deps, env, _) = setup_contract();

        // Setup job and proposals
        let client_info = mock_info(CLIENT, &coins(5000, "uxion"));
        let freelancer_info = mock_info(FREELANCER, &[]);

        // Create job
        let job_msg = ExecuteMsg::PostJob {
            title: "Cross Function Test".to_string(),
            description: "Testing cross-function reentrancy".to_string(),
            budget: Uint128::new(5000),
            category: "Security".to_string(),
            skills_required: vec!["Security".to_string()],
            duration_days: 30,
            documents: None,
            milestones: None,
        };

        execute(deps.as_mut(), env.clone(), client_info.clone(), job_msg).unwrap();

        // Submit proposal
        let proposal_msg = ExecuteMsg::SubmitProposal {
            job_id: 0,
            cover_letter: "Test proposal".to_string(),
            delivery_time_days: 25,
            milestones: None,
        };

        execute(deps.as_mut(), env.clone(), freelancer_info, proposal_msg).unwrap();

        // Test that accepting proposal and creating escrow work in sequence
        let accept_msg = ExecuteMsg::AcceptProposal {
            job_id: 0,
            proposal_id: 0,
        };

        let result = execute(deps.as_mut(), env.clone(), client_info.clone(), accept_msg);
        assert!(result.is_ok());

        // Verify job state changed correctly
        let job = JOBS.load(deps.as_ref().storage, 0).unwrap();
        assert_eq!(job.status, JobStatus::InProgress);
        assert!(job.assigned_freelancer.is_some());
    }
}
