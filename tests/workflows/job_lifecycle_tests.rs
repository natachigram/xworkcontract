use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
use cosmwasm_std::{coins, Uint128, OwnedDeps};
use cosmwasm_std::testing::{MockStorage, MockApi, MockQuerier};

use xworks_freelance_contract::contract::{execute, instantiate};
use xworks_freelance_contract::msg::{ExecuteMsg, InstantiateMsg};
use xworks_freelance_contract::state::{JobStatus, JOBS, ESCROWS, RATINGS};

const ADMIN: &str = "admin";
const CLIENT: &str = "client1";
const FREELANCER: &str = "freelancer1";

mod job_lifecycle_tests {
    use super::*;

    fn setup_contract() -> (cosmwasm_std::OwnedDeps<cosmwasm_std::testing::MockStorage, cosmwasm_std::testing::MockApi, cosmwasm_std::testing::MockQuerier>, cosmwasm_std::Env) {
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

        instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();
        (deps, env)
    }

    #[test]
    fn test_complete_paid_job_lifecycle() {
        let (mut deps, env) = setup_contract();

        // 1. Client posts a paid job
        let client_info = mock_info(CLIENT, &coins(5000, "uxion"));
        let job_msg = ExecuteMsg::PostJob {
            title: "Build a DApp".to_string(),
            description: "Need a decentralized application with smart contracts".to_string(),
            budget: Uint128::new(5000),
            category: "Blockchain".to_string(),
            skills_required: vec!["Rust".to_string(), "CosmWasm".to_string()],
            duration_days: 30,
            documents: None,
            milestones: None,
        };

        let result = execute(deps.as_mut(), env.clone(), client_info.clone(), job_msg);
        assert!(result.is_ok());

        // Verify job is created and in Open status
        let job = JOBS.load(deps.as_ref().storage, 0).unwrap();
        assert_eq!(job.status, JobStatus::Open);
        assert_eq!(job.budget, Uint128::new(5000));

        // 2. Freelancer submits proposal
        let freelancer_info = mock_info(FREELANCER, &[]);
        let proposal_msg = ExecuteMsg::SubmitProposal {
            job_id: 0,
            cover_letter: "I'm an expert in Rust and CosmWasm development with 5 years experience.".to_string(),
            delivery_time_days: 25,
            milestones: None,
        };

        let result = execute(deps.as_mut(), env.clone(), freelancer_info.clone(), proposal_msg);
        assert!(result.is_ok());

        // 3. Client accepts proposal
        let accept_msg = ExecuteMsg::AcceptProposal {
            job_id: 0,
            proposal_id: 0,
        };

        let result = execute(deps.as_mut(), env.clone(), client_info.clone(), accept_msg);
        assert!(result.is_ok());

        // Verify job status changed to InProgress
        let job = JOBS.load(deps.as_ref().storage, 0).unwrap();
        assert_eq!(job.status, JobStatus::InProgress);
        assert_eq!(job.assigned_freelancer, Some(deps.api.addr_validate(FREELANCER).unwrap()));

        // 4. Client creates escrow
        let escrow_info = mock_info(CLIENT, &coins(5000, "uxion"));
        let create_escrow_msg = ExecuteMsg::CreateEscrowNative {
            job_id: 0,
            amount: Uint128::new(5000),
        };

        let result = execute(deps.as_mut(), env.clone(), escrow_info, create_escrow_msg);
        assert!(result.is_ok());

        // Verify escrow was created
        let job = JOBS.load(deps.as_ref().storage, 0).unwrap();
        assert!(job.escrow_id.is_some());

        // 5. Freelancer completes work and requests completion
        let complete_msg = ExecuteMsg::CompleteJob {
            job_id: 0,
            completion_notes: "DApp completed with all features implemented.".to_string(),
            deliverables: vec!["https://github.com/freelancer1/dapp".to_string()],
        };

        let result = execute(deps.as_mut(), env.clone(), freelancer_info.clone(), complete_msg);
        assert!(result.is_ok());

        // Verify job status changed to Completed
        let job = JOBS.load(deps.as_ref().storage, 0).unwrap();
        assert_eq!(job.status, JobStatus::Completed);

        // 6. Client releases escrow
        let release_msg = ExecuteMsg::ReleaseEscrow {
            escrow_id: job.escrow_id.unwrap(),
        };

        let result = execute(deps.as_mut(), env.clone(), client_info.clone(), release_msg);
        assert!(result.is_ok());

        // 7. Both parties submit ratings
        let client_rating_msg = ExecuteMsg::SubmitRating {
            job_id: 0,
            rating: 5,
            comment: "Excellent work, delivered on time!".to_string(),
        };

        let result = execute(deps.as_mut(), env.clone(), client_info, client_rating_msg);
        assert!(result.is_ok());

        let freelancer_rating_msg = ExecuteMsg::SubmitRating {
            job_id: 0,
            rating: 5,
            comment: "Great client, clear requirements and prompt payment!".to_string(),
        };

        let result = execute(deps.as_mut(), env, freelancer_info, freelancer_rating_msg);
        assert!(result.is_ok());

        // Verify ratings were created
        let ratings: Vec<_> = RATINGS
            .range(deps.as_ref().storage, None, None, cosmwasm_std::Order::Ascending)
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(ratings.len(), 2);
    }

    #[test]
    fn test_complete_free_job_lifecycle() {
        let (api, mut storage, querier, env) = setup_contract();
        let mut deps = cosmwasm_std::OwnedDeps { api, storage: &mut storage, querier, custom_query_type: std::marker::PhantomData };

        // 1. Client posts a free job (budget = 0)
        let client_info = mock_info(CLIENT, &[]); // No payment for free job
        let job_msg = ExecuteMsg::PostJob {
            title: "Code Review".to_string(),
            description: "Need a quick code review for my smart contract".to_string(),
            budget: Uint128::zero(),
            category: "Review".to_string(),
            skills_required: vec!["Rust".to_string()],
            duration_days: 7,
            documents: None,
            milestones: None,
        };

        let result = execute(deps.as_mut(), env.clone(), client_info.clone(), job_msg);
        assert!(result.is_ok());

        // Verify job is created and in Open status
        let job = JOBS.load(deps.as_ref().storage, 0).unwrap();
        assert_eq!(job.status, JobStatus::Open);
        assert_eq!(job.budget, Uint128::zero());

        // 2. Freelancer submits proposal
        let freelancer_info = mock_info(FREELANCER, &[]);
        let proposal_msg = ExecuteMsg::SubmitProposal {
            job_id: 0,
            cover_letter: "I can do a quick code review for you, happy to help!".to_string(),
            delivery_time_days: 3,
            milestones: None,
        };

        let result = execute(deps.as_mut(), env.clone(), freelancer_info.clone(), proposal_msg);
        assert!(result.is_ok());

        // 3. Client accepts proposal
        let accept_msg = ExecuteMsg::AcceptProposal {
            job_id: 0,
            proposal_id: 0,
        };

        let result = execute(deps.as_mut(), env.clone(), client_info.clone(), accept_msg);
        assert!(result.is_ok());

        // Verify job status changed to InProgress
        let job = JOBS.load(deps.as_ref().storage, 0).unwrap();
        assert_eq!(job.status, JobStatus::InProgress);
        assert_eq!(job.assigned_freelancer, Some(deps.api.addr_validate(FREELANCER).unwrap()));

        // 4. Try to create escrow (should fail for free jobs)
        let escrow_info = mock_info(CLIENT, &[]);
        let create_escrow_msg = ExecuteMsg::CreateEscrowNative {
            job_id: 0,
            amount: Uint128::zero(),
        };

        let result = execute(deps.as_mut(), env.clone(), escrow_info, create_escrow_msg);
        assert!(result.is_err()); // Should fail for free projects

        // 5. Freelancer completes work
        let complete_msg = ExecuteMsg::CompleteJob {
            job_id: 0,
            completion_notes: "Code review completed. Found 2 minor issues and provided suggestions.".to_string(),
            deliverables: vec!["Review document with recommendations".to_string()],
        };

        let result = execute(deps.as_mut(), env.clone(), freelancer_info.clone(), complete_msg);
        assert!(result.is_ok());

        // Verify job status changed to Completed
        let job = JOBS.load(deps.as_ref().storage, 0).unwrap();
        assert_eq!(job.status, JobStatus::Completed);

        // 6. Both parties submit ratings
        let client_rating_msg = ExecuteMsg::SubmitRating {
            job_id: 0,
            rating: 4,
            comment: "Good review, helpful suggestions!".to_string(),
        };

        let result = execute(deps.as_mut(), env.clone(), client_info, client_rating_msg);
        assert!(result.is_ok());

        let freelancer_rating_msg = ExecuteMsg::SubmitRating {
            job_id: 0,
            rating: 5,
            comment: "Nice client, interesting project!".to_string(),
        };

        let result = execute(deps.as_mut(), env, freelancer_info, freelancer_rating_msg);
        assert!(result.is_ok());
    }

    #[test]
    fn test_job_cancellation_workflow() {
        let (api, mut storage, querier, env) = setup_contract();
        let mut deps = cosmwasm_std::OwnedDeps { api, storage: &mut storage, querier, custom_query_type: std::marker::PhantomData };

        // 1. Client posts job
        let client_info = mock_info(CLIENT, &coins(3000, "uxion"));
        let job_msg = ExecuteMsg::PostJob {
            title: "Website Design".to_string(),
            description: "Need a modern website design".to_string(),
            budget: Uint128::new(3000),
            category: "Design".to_string(),
            skills_required: vec!["UI/UX".to_string()],
            duration_days: 14,
            documents: None,
            milestones: None,
        };

        execute(deps.as_mut(), env.clone(), client_info.clone(), job_msg).unwrap();

        // 2. Client cancels job before any proposals
        let cancel_msg = ExecuteMsg::CancelJob { job_id: 0 };
        let result = execute(deps.as_mut(), env.clone(), client_info, cancel_msg);
        assert!(result.is_ok());

        // Verify job status changed to Cancelled
        let job = JOBS.load(deps.as_ref().storage, 0).unwrap();
        assert_eq!(job.status, JobStatus::Cancelled);
    }

    #[test]
    fn test_multiple_proposals_workflow() {
        let (api, mut storage, querier, env) = setup_contract();
        let mut deps = cosmwasm_std::OwnedDeps { api, storage: &mut storage, querier, custom_query_type: std::marker::PhantomData };

        let freelancer2 = "freelancer2";
        let freelancer3 = "freelancer3";

        // 1. Client posts job
        let client_info = mock_info(CLIENT, &coins(4000, "uxion"));
        let job_msg = ExecuteMsg::PostJob {
            title: "Mobile App Development".to_string(),
            description: "Need a React Native mobile app".to_string(),
            budget: Uint128::new(4000),
            category: "Mobile".to_string(),
            skills_required: vec!["React Native".to_string(), "JavaScript".to_string()],
            duration_days: 45,
            documents: None,
            milestones: None,
        };

        execute(deps.as_mut(), env.clone(), client_info.clone(), job_msg).unwrap();

        // 2. Multiple freelancers submit proposals
        let freelancer1_info = mock_info(FREELANCER, &[]);
        let proposal1_msg = ExecuteMsg::SubmitProposal {
            job_id: 0,
            cover_letter: "I have 3 years React Native experience".to_string(),
            delivery_time_days: 40,
            milestones: None,
        };
        execute(deps.as_mut(), env.clone(), freelancer1_info, proposal1_msg).unwrap();

        let freelancer2_info = mock_info(freelancer2, &[]);
        let proposal2_msg = ExecuteMsg::SubmitProposal {
            job_id: 0,
            cover_letter: "Expert in React Native with 5 years experience".to_string(),
            delivery_time_days: 35,
            milestones: None,
        };
        execute(deps.as_mut(), env.clone(), freelancer2_info, proposal2_msg).unwrap();

        let freelancer3_info = mock_info(freelancer3, &[]);
        let proposal3_msg = ExecuteMsg::SubmitProposal {
            job_id: 0,
            cover_letter: "Full-stack developer specializing in mobile apps".to_string(),
            delivery_time_days: 42,
            milestones: None,
        };
        execute(deps.as_mut(), env.clone(), freelancer3_info, proposal3_msg).unwrap();

        // 3. Client accepts the second proposal
        let accept_msg = ExecuteMsg::AcceptProposal {
            job_id: 0,
            proposal_id: 1, // Accept freelancer2's proposal
        };

        let result = execute(deps.as_mut(), env, client_info, accept_msg);
        assert!(result.is_ok());

        // Verify correct freelancer was assigned
        let job = JOBS.load(deps.as_ref().storage, 0).unwrap();
        assert_eq!(job.status, JobStatus::InProgress);
        assert_eq!(job.assigned_freelancer, Some(deps.api.addr_validate(freelancer2).unwrap()));
    }
}
