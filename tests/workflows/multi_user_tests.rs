use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
use cosmwasm_std::{coins, Uint128};

use xworks_freelance_contract::contract::{execute, instantiate};
use xworks_freelance_contract::msg::{ExecuteMsg, InstantiateMsg};
use xworks_freelance_contract::state::{JobStatus, JOBS, DISPUTES, RATINGS};

const ADMIN: &str = "admin";
const CLIENT1: &str = "client1";
const CLIENT2: &str = "client2";
const FREELANCER1: &str = "freelancer1";
const FREELANCER2: &str = "freelancer2";

mod multi_user_tests {
    use super::*;

    fn setup_contract() -> (cosmwasm_std::testing::MockApi, cosmwasm_std::testing::MockStorage, cosmwasm_std::testing::MockQuerier, cosmwasm_std::Env) {
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
        (deps.api, deps.storage, deps.querier, env)
    }

    #[test]
    fn test_concurrent_multiple_jobs() {
        let (api, mut storage, querier, env) = setup_contract();
        let mut deps = cosmwasm_std::OwnedDeps { api, storage: &mut storage, querier, custom_query_type: std::marker::PhantomData };

        // Client 1 posts job 1
        let client1_info = mock_info(CLIENT1, &coins(5000, "uxion"));
        let job1_msg = ExecuteMsg::PostJob {
            title: "E-commerce Platform".to_string(),
            description: "Build a full e-commerce solution".to_string(),
            budget: Uint128::new(5000),
            category: "Web Development".to_string(),
            skills_required: vec!["React".to_string(), "Node.js".to_string()],
            duration_days: 60,
            documents: None,
            milestones: None,
        };
        execute(deps.as_mut(), env.clone(), client1_info.clone(), job1_msg).unwrap();

        // Client 2 posts job 2
        let client2_info = mock_info(CLIENT2, &coins(3000, "uxion"));
        let job2_msg = ExecuteMsg::PostJob {
            title: "Mobile Game".to_string(),
            description: "Develop a 2D mobile game".to_string(),
            budget: Uint128::new(3000),
            category: "Game Development".to_string(),
            skills_required: vec!["Unity".to_string(), "C#".to_string()],
            duration_days: 45,
            documents: None,
            milestones: None,
        };
        execute(deps.as_mut(), env.clone(), client2_info.clone(), job2_msg).unwrap();

        // Freelancer 1 applies to job 1
        let freelancer1_info = mock_info(FREELANCER1, &[]);
        let proposal1_msg = ExecuteMsg::SubmitProposal {
            job_id: 0,
            cover_letter: "Expert in React and Node.js with e-commerce experience".to_string(),
            delivery_time_days: 55,
            milestones: None,
        };
        execute(deps.as_mut(), env.clone(), freelancer1_info.clone(), proposal1_msg).unwrap();

        // Freelancer 2 applies to job 2
        let freelancer2_info = mock_info(FREELANCER2, &[]);
        let proposal2_msg = ExecuteMsg::SubmitProposal {
            job_id: 1,
            cover_letter: "Unity game developer with 4 years experience".to_string(),
            delivery_time_days: 40,
            milestones: None,
        };
        execute(deps.as_mut(), env.clone(), freelancer2_info.clone(), proposal2_msg).unwrap();

        // Both clients accept their respective proposals
        let accept1_msg = ExecuteMsg::AcceptProposal {
            job_id: 0,
            proposal_id: 0,
        };
        execute(deps.as_mut(), env.clone(), client1_info.clone(), accept1_msg).unwrap();

        let accept2_msg = ExecuteMsg::AcceptProposal {
            job_id: 1,
            proposal_id: 1,
        };
        execute(deps.as_mut(), env.clone(), client2_info.clone(), accept2_msg).unwrap();

        // Verify both jobs are in progress with correct freelancers
        let job1 = JOBS.load(deps.as_ref().storage, 0).unwrap();
        assert_eq!(job1.status, JobStatus::InProgress);
        assert_eq!(job1.assigned_freelancer, Some(deps.api.addr_validate(FREELANCER1).unwrap()));

        let job2 = JOBS.load(deps.as_ref().storage, 1).unwrap();
        assert_eq!(job2.status, JobStatus::InProgress);
        assert_eq!(job2.assigned_freelancer, Some(deps.api.addr_validate(FREELANCER2).unwrap()));

        // Both clients create escrows
        let escrow1_info = mock_info(CLIENT1, &coins(5000, "uxion"));
        let create_escrow1_msg = ExecuteMsg::CreateEscrowNative {
            job_id: 0,
            amount: Uint128::new(5000),
        };
        execute(deps.as_mut(), env.clone(), escrow1_info, create_escrow1_msg).unwrap();

        let escrow2_info = mock_info(CLIENT2, &coins(3000, "uxion"));
        let create_escrow2_msg = ExecuteMsg::CreateEscrowNative {
            job_id: 1,
            amount: Uint128::new(3000),
        };
        execute(deps.as_mut(), env.clone(), escrow2_info, create_escrow2_msg).unwrap();

        // Both freelancers complete their jobs
        let complete1_msg = ExecuteMsg::CompleteJob {
            job_id: 0,
            completion_notes: "E-commerce platform completed with all features".to_string(),
            deliverables: vec!["https://github.com/freelancer1/ecommerce".to_string()],
        };
        execute(deps.as_mut(), env.clone(), freelancer1_info, complete1_msg).unwrap();

        let complete2_msg = ExecuteMsg::CompleteJob {
            job_id: 1,
            completion_notes: "Mobile game completed and tested".to_string(),
            deliverables: vec!["Game APK and source code".to_string()],
        };
        execute(deps.as_mut(), env, freelancer2_info, complete2_msg).unwrap();

        // Verify both jobs completed
        let job1 = JOBS.load(deps.as_ref().storage, 0).unwrap();
        assert_eq!(job1.status, JobStatus::Completed);

        let job2 = JOBS.load(deps.as_ref().storage, 1).unwrap();
        assert_eq!(job2.status, JobStatus::Completed);
    }

    #[test]
    fn test_cross_user_interactions() {
        let (api, mut storage, querier, env) = setup_contract();
        let mut deps = cosmwasm_std::OwnedDeps { api, storage: &mut storage, querier, custom_query_type: std::marker::PhantomData };

        // Client 1 posts a job
        let client1_info = mock_info(CLIENT1, &coins(2000, "uxion"));
        let job_msg = ExecuteMsg::PostJob {
            title: "Logo Design".to_string(),
            description: "Need a professional logo design".to_string(),
            budget: Uint128::new(2000),
            category: "Design".to_string(),
            skills_required: vec!["Graphic Design".to_string()],
            duration_days: 10,
            documents: None,
            milestones: None,
        };
        execute(deps.as_mut(), env.clone(), client1_info.clone(), job_msg).unwrap();

        // Freelancer 1 submits proposal
        let freelancer1_info = mock_info(FREELANCER1, &[]);
        let proposal_msg = ExecuteMsg::SubmitProposal {
            job_id: 0,
            cover_letter: "Professional graphic designer with 6 years experience".to_string(),
            delivery_time_days: 7,
            milestones: None,
        };
        execute(deps.as_mut(), env.clone(), freelancer1_info.clone(), proposal_msg).unwrap();

        // Test unauthorized access - Client 2 tries to accept proposal for Client 1's job
        let client2_info = mock_info(CLIENT2, &[]);
        let unauthorized_accept = ExecuteMsg::AcceptProposal {
            job_id: 0,
            proposal_id: 0,
        };
        let result = execute(deps.as_mut(), env.clone(), client2_info.clone(), unauthorized_accept);
        assert!(result.is_err()); // Should fail - wrong client

        // Test unauthorized access - Freelancer 2 tries to complete Freelancer 1's job
        let freelancer2_info = mock_info(FREELANCER2, &[]);
        
        // First accept the proposal properly
        let accept_msg = ExecuteMsg::AcceptProposal {
            job_id: 0,
            proposal_id: 0,
        };
        execute(deps.as_mut(), env.clone(), client1_info, accept_msg).unwrap();

        // Now try unauthorized completion
        let unauthorized_complete = ExecuteMsg::CompleteJob {
            job_id: 0,
            completion_notes: "Unauthorized completion attempt".to_string(),
            deliverables: vec!["Fake deliverable".to_string()],
        };
        let result = execute(deps.as_mut(), env, freelancer2_info, unauthorized_complete);
        assert!(result.is_err()); // Should fail - wrong freelancer
    }

    #[test]
    fn test_dispute_workflow() {
        let (api, mut storage, querier, env) = setup_contract();
        let mut deps = cosmwasm_std::OwnedDeps { api, storage: &mut storage, querier, custom_query_type: std::marker::PhantomData };

        // Setup: Client posts job, freelancer applies, gets accepted, escrow created
        let client_info = mock_info(CLIENT1, &coins(4000, "uxion"));
        let job_msg = ExecuteMsg::PostJob {
            title: "API Development".to_string(),
            description: "Develop REST API for mobile app".to_string(),
            budget: Uint128::new(4000),
            category: "Backend".to_string(),
            skills_required: vec!["Python".to_string(), "FastAPI".to_string()],
            duration_days: 21,
            documents: None,
            milestones: None,
        };
        execute(deps.as_mut(), env.clone(), client_info.clone(), job_msg).unwrap();

        let freelancer_info = mock_info(FREELANCER1, &[]);
        let proposal_msg = ExecuteMsg::SubmitProposal {
            job_id: 0,
            cover_letter: "Backend developer with Python expertise".to_string(),
            delivery_time_days: 18,
            milestones: None,
        };
        execute(deps.as_mut(), env.clone(), freelancer_info.clone(), proposal_msg).unwrap();

        let accept_msg = ExecuteMsg::AcceptProposal {
            job_id: 0,
            proposal_id: 0,
        };
        execute(deps.as_mut(), env.clone(), client_info.clone(), accept_msg).unwrap();

        let escrow_info = mock_info(CLIENT1, &coins(4000, "uxion"));
        let create_escrow_msg = ExecuteMsg::CreateEscrowNative {
            job_id: 0,
            amount: Uint128::new(4000),
        };
        execute(deps.as_mut(), env.clone(), escrow_info, create_escrow_msg).unwrap();

        // Client raises a dispute
        let dispute_msg = ExecuteMsg::RaiseDispute {
            job_id: 0,
            reason: "Freelancer has not delivered quality work as specified".to_string(),
            evidence: vec!["Chat logs showing poor communication".to_string()],
        };
        let result = execute(deps.as_mut(), env.clone(), client_info.clone(), dispute_msg);
        assert!(result.is_ok());

        // Verify job status changed to Disputed
        let job = JOBS.load(deps.as_ref().storage, 0).unwrap();
        assert_eq!(job.status, JobStatus::Disputed);

        // Verify dispute was created
        let disputes: Vec<_> = DISPUTES
            .range(deps.as_ref().storage, None, None, cosmwasm_std::Order::Ascending)
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(disputes.len(), 1);

        // Admin resolves dispute in favor of client
        let admin_info = mock_info(ADMIN, &[]);
        let resolve_msg = ExecuteMsg::ResolveDispute {
            dispute_id: "dispute_0".to_string(),
            resolution: "Refund client".to_string(),
            refund_percentage: 100,
        };
        let result = execute(deps.as_mut(), env, admin_info, resolve_msg);
        assert!(result.is_ok());
    }

    #[test]
    fn test_rating_and_reputation_workflow() {
        let (api, mut storage, querier, env) = setup_contract();
        let mut deps = cosmwasm_std::OwnedDeps { api, storage: &mut storage, querier, custom_query_type: std::marker::PhantomData };

        // Complete workflow for first job
        let client_info = mock_info(CLIENT1, &coins(2500, "uxion"));
        let job_msg = ExecuteMsg::PostJob {
            title: "Data Analysis".to_string(),
            description: "Analyze sales data and create report".to_string(),
            budget: Uint128::new(2500),
            category: "Data Science".to_string(),
            skills_required: vec!["Python".to_string(), "Pandas".to_string()],
            duration_days: 14,
            documents: None,
            milestones: None,
        };
        execute(deps.as_mut(), env.clone(), client_info.clone(), job_msg).unwrap();

        let freelancer_info = mock_info(FREELANCER1, &[]);
        let proposal_msg = ExecuteMsg::SubmitProposal {
            job_id: 0,
            cover_letter: "Data scientist with expertise in Python and data visualization".to_string(),
            delivery_time_days: 12,
            milestones: None,
        };
        execute(deps.as_mut(), env.clone(), freelancer_info.clone(), proposal_msg).unwrap();

        let accept_msg = ExecuteMsg::AcceptProposal {
            job_id: 0,
            proposal_id: 0,
        };
        execute(deps.as_mut(), env.clone(), client_info.clone(), accept_msg).unwrap();

        let escrow_info = mock_info(CLIENT1, &coins(2500, "uxion"));
        let create_escrow_msg = ExecuteMsg::CreateEscrowNative {
            job_id: 0,
            amount: Uint128::new(2500),
        };
        execute(deps.as_mut(), env.clone(), escrow_info, create_escrow_msg).unwrap();

        let complete_msg = ExecuteMsg::CompleteJob {
            job_id: 0,
            completion_notes: "Data analysis completed with detailed report and visualizations".to_string(),
            deliverables: vec!["Analysis report PDF".to_string(), "Python scripts".to_string()],
        };
        execute(deps.as_mut(), env.clone(), freelancer_info.clone(), complete_msg).unwrap();

        // Both parties submit ratings
        let client_rating_msg = ExecuteMsg::SubmitRating {
            job_id: 0,
            rating: 5,
            comment: "Outstanding work, very detailed analysis!".to_string(),
        };
        execute(deps.as_mut(), env.clone(), client_info, client_rating_msg).unwrap();

        let freelancer_rating_msg = ExecuteMsg::SubmitRating {
            job_id: 0,
            rating: 4,
            comment: "Good client, but requirements could have been clearer".to_string(),
        };
        execute(deps.as_mut(), env, freelancer_info, freelancer_rating_msg).unwrap();

        // Verify ratings were recorded
        let ratings: Vec<_> = xworks_freelance_contract::state::RATINGS
            .range(deps.as_ref().storage, None, None, cosmwasm_std::Order::Ascending)
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(ratings.len(), 2);
    }
}
