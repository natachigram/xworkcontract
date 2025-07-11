use cosmwasm_std::testing::{
    mock_dependencies, mock_env, mock_info, MockApi, MockQuerier, MockStorage,
};
use cosmwasm_std::{coins, from_json, Addr, Env, MessageInfo, OwnedDeps, Uint128};

use xworks_freelance_contract::contract::{execute, instantiate, query};
use xworks_freelance_contract::msg::{
    ConfigResponse, ExecuteMsg, InstantiateMsg, JobResponse, ProposalsResponse, QueryMsg,
};
use xworks_freelance_contract::state::JobStatus;
use xworks_freelance_contract::ContractError;

const ADMIN: &str = "admin";
const CLIENT: &str = "client";
const FREELANCER: &str = "freelancer";

mod tests {
    use super::*;

    fn proper_instantiate() -> (
        OwnedDeps<MockStorage, MockApi, MockQuerier>,
        Env,
        MessageInfo,
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

        let _res = instantiate(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();
        (deps, env, info)
    }

    #[test]
    fn test_instantiate() {
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

        let res = instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // Verify config
        let config_query = QueryMsg::GetConfig {};
        let res = query(deps.as_ref(), env, config_query).unwrap();
        let config: ConfigResponse = from_json(&res).unwrap();

        assert_eq!(config.config.admin, Addr::unchecked(ADMIN));
        assert_eq!(config.config.platform_fee_percent, 5);
        assert_eq!(config.config.min_escrow_amount, Uint128::new(1000));
    }

    #[test]
    fn test_post_job() {
        let (mut deps, env, _info) = proper_instantiate();
        let client_info = mock_info(CLIENT, &[]);

        let msg = ExecuteMsg::PostJob {
            title: "Test Job".to_string(),
            description: "A test job description".to_string(),
            budget: Uint128::new(5000),
            category: "Development".to_string(),
            skills_required: vec!["Rust".to_string(), "CosmWasm".to_string()],
            duration_days: 30,
            documents: None,
            milestones: None,
        };

        let res = execute(deps.as_mut(), env.clone(), client_info, msg).unwrap();
        assert!(res.attributes.len() >= 4);

        // Verify job was created
        let job_query = QueryMsg::GetJob { job_id: 0 };
        let res = query(deps.as_ref(), env, job_query).unwrap();
        let job_response: JobResponse = from_json(&res).unwrap();

        assert_eq!(job_response.job.title, "Test Job");
        assert_eq!(job_response.job.status, JobStatus::Open);
        assert_eq!(job_response.job.budget, Uint128::new(5000));
    }

    #[test]
    fn test_submit_proposal() {
        let (mut deps, env, _info) = proper_instantiate();
        let client_info = mock_info(CLIENT, &[]);

        // First create a job
        let job_msg = ExecuteMsg::PostJob {
            title: "Test Job".to_string(),
            description: "A test job description".to_string(),
            budget: Uint128::new(5000),
            category: "Development".to_string(),
            skills_required: vec!["Rust".to_string()],
            duration_days: 30,
            documents: None,
            milestones: None,
        };

        execute(deps.as_mut(), env.clone(), client_info, job_msg).unwrap();

        // Now submit a proposal as freelancer
        let freelancer_info = mock_info(FREELANCER, &[]);
        let proposal_msg = ExecuteMsg::SubmitProposal {
            job_id: 0,
            cover_letter: "I am the best freelancer for this job".to_string(),
            delivery_time_days: 25,
            milestones: None,
        };

        let res = execute(deps.as_mut(), env.clone(), freelancer_info, proposal_msg).unwrap();
        assert!(res.attributes.len() >= 4);

        // Verify proposal was created
        let proposals_query = QueryMsg::GetJobProposals { job_id: 0 };
        let res = query(deps.as_ref(), env, proposals_query).unwrap();
        let proposals_response: ProposalsResponse = from_json(&res).unwrap();

        assert_eq!(proposals_response.proposals.len(), 1);
        // bid_amount removed; validate freelancer field
        assert_eq!(
            proposals_response.proposals[0].freelancer,
            Addr::unchecked(FREELANCER)
        );
    }

    #[test]
    fn test_accept_proposal() {
        let (mut deps, env, _info) = proper_instantiate();
        let client_info = mock_info(CLIENT, &[]);

        // Create job
        let job_msg = ExecuteMsg::PostJob {
            title: "Test Job".to_string(),
            description: "A test job description".to_string(),
            budget: Uint128::new(5000),
            category: "Development".to_string(),
            skills_required: vec!["Rust".to_string()],
            duration_days: 30,
            documents: None,
            milestones: None,
        };
        execute(deps.as_mut(), env.clone(), client_info.clone(), job_msg).unwrap();

        // Submit proposal
        let freelancer_info = mock_info(FREELANCER, &[]);
        let proposal_msg = ExecuteMsg::SubmitProposal {
            job_id: 0,
            cover_letter: "Great proposal".to_string(),
            delivery_time_days: 25,
            milestones: None,
        };
        execute(deps.as_mut(), env.clone(), freelancer_info, proposal_msg).unwrap();

        // Accept proposal as client
        let accept_msg = ExecuteMsg::AcceptProposal {
            job_id: 0,
            proposal_id: 0,
        };
        let res = execute(deps.as_mut(), env.clone(), client_info, accept_msg).unwrap();
        assert!(res.attributes.len() >= 3);

        // Verify job status changed
        let job_query = QueryMsg::GetJob { job_id: 0 };
        let res = query(deps.as_ref(), env, job_query).unwrap();
        let job_response: JobResponse = from_json(&res).unwrap();

        assert_eq!(job_response.job.status, JobStatus::InProgress);
        assert_eq!(
            job_response.job.assigned_freelancer,
            Some(Addr::unchecked(FREELANCER))
        );
    }

    #[test]
    fn test_complete_job_flow() {
        let (mut deps, env, _info) = proper_instantiate();

        // Post job
        let client_info = mock_info(CLIENT, &[]);
        let job_msg = ExecuteMsg::PostJob {
            title: "Complete Test Job".to_string(),
            description: "A test job for full workflow".to_string(),
            budget: Uint128::new(10000),
            category: "Development".to_string(),
            skills_required: vec!["Rust".to_string()],
            duration_days: 30,
            documents: None,
            milestones: None,
        };
        execute(deps.as_mut(), env.clone(), client_info.clone(), job_msg).unwrap();

        // Submit proposal
        let freelancer_info = mock_info(FREELANCER, &[]);
        let proposal_msg = ExecuteMsg::SubmitProposal {
            job_id: 0,
            // single cover_letter field
            cover_letter: "Expert Rust developer".to_string(),
            delivery_time_days: 25,
            milestones: None,
        };
        execute(
            deps.as_mut(),
            env.clone(),
            freelancer_info.clone(),
            proposal_msg,
        )
        .unwrap();

        // Accept proposal
        let accept_msg = ExecuteMsg::AcceptProposal {
            job_id: 0,
            proposal_id: 0,
        };
        execute(deps.as_mut(), env.clone(), client_info.clone(), accept_msg).unwrap();

        // Create escrow
        let escrow_info = mock_info(CLIENT, &coins(10000, "uxion"));
        let escrow_msg = ExecuteMsg::CreateEscrow { job_id: 0 };
        execute(deps.as_mut(), env.clone(), escrow_info, escrow_msg).unwrap();

        // Complete job
        let complete_msg = ExecuteMsg::CompleteJob { job_id: 0 };
        let res = execute(deps.as_mut(), env, freelancer_info, complete_msg).unwrap();

        // Should have messages for payment release
        assert!(!res.messages.is_empty());
    }

    #[test]
    fn test_input_validation() {
        let (mut deps, env, _info) = proper_instantiate();
        let client_info = mock_info(CLIENT, &[]);

        // Test empty title
        let invalid_job = ExecuteMsg::PostJob {
            title: "".to_string(), // Too short
            description: "Valid description".to_string(),
            budget: Uint128::new(5000),
            category: "Development".to_string(),
            skills_required: vec!["Rust".to_string()],
            duration_days: 30,
            documents: None,
            milestones: None,
        };

        // Execute and capture error directly
        let err =
            execute(deps.as_mut(), env.clone(), client_info.clone(), invalid_job).unwrap_err();
        match err {
            ContractError::InvalidInput { .. } => {} // Expected
            _ => panic!("Expected invalid input error"),
        }

        // Test zero budget free project should succeed
        let zero_budget_job = ExecuteMsg::PostJob {
            title: "Valid Title".to_string(),
            description: "Valid description".to_string(),
            budget: Uint128::zero(), // Zero budget for free project
            category: "Development".to_string(),
            skills_required: vec!["Rust".to_string()],
            duration_days: 30,
            documents: None,
            milestones: None,
        };

        // Execute should succeed for free projects
        assert!(
            execute(deps.as_mut(), env, client_info, zero_budget_job)
                .unwrap()
                .attributes
                .len()
                > 0
        );
    }

    #[test]
    fn test_unauthorized_access() {
        let (mut deps, env, _info) = proper_instantiate();
        let client_info = mock_info(CLIENT, &[]);

        // Try to delete a job that doesn't exist
        let delete_msg = ExecuteMsg::DeleteJob { job_id: 999 };
        let err = execute(deps.as_mut(), env.clone(), client_info.clone(), delete_msg).unwrap_err();
        assert_eq!(err, ContractError::JobNotFound {});

        // Create a job
        let job_msg = ExecuteMsg::PostJob {
            title: "Test Job".to_string(),
            description: "A test job description".to_string(),
            budget: Uint128::new(5000),
            category: "Development".to_string(),
            skills_required: vec!["Rust".to_string()],
            duration_days: 30,
            documents: None,
            milestones: None,
        };
        execute(deps.as_mut(), env.clone(), client_info, job_msg).unwrap();

        // Try to delete as unauthorized user
        let unauthorized_info = mock_info("unauthorized", &[]);
        let delete_msg = ExecuteMsg::DeleteJob { job_id: 0 };
        let err = execute(deps.as_mut(), env, unauthorized_info, delete_msg).unwrap_err();
        assert_eq!(err, ContractError::Unauthorized {});
    }

    #[test]
    fn test_admin_functions() {
        let (mut deps, env, info) = proper_instantiate();

        // Test pause contract
        let pause_msg = ExecuteMsg::PauseContract {};
        let res = execute(deps.as_mut(), env.clone(), info.clone(), pause_msg);
        assert!(res.is_ok());

        // Test unpause contract
        let unpause_msg = ExecuteMsg::UnpauseContract {};
        let res = execute(deps.as_mut(), env.clone(), info.clone(), unpause_msg);
        assert!(res.is_ok());

        // Test block address
        let block_msg = ExecuteMsg::BlockAddress {
            address: "malicious_user".to_string(),
            reason: "Spam".to_string(),
        };
        let res = execute(deps.as_mut(), env.clone(), info.clone(), block_msg);
        assert!(res.is_ok());

        // Test unblock address
        let unblock_msg = ExecuteMsg::UnblockAddress {
            address: "malicious_user".to_string(),
        };
        let res = execute(deps.as_mut(), env, info, unblock_msg);
        assert!(res.is_ok());
    }

    #[test]
    fn test_query_functions() {
        let (deps, env, _info) = proper_instantiate();

        // Test various query functions
        let queries = vec![
            QueryMsg::GetConfig {},
            QueryMsg::GetJobs {
                start_after: None,
                limit: None,
                category: None,
                status: None,
                poster: None,
            },
            QueryMsg::GetAllJobs {
                category: None,
                limit: None,
            },
            QueryMsg::GetPlatformStats {},
            QueryMsg::GetUserStats {
                user: CLIENT.to_string(),
            },
        ];

        for query_msg in queries {
            let res = query(deps.as_ref(), env.clone(), query_msg);
            assert!(res.is_ok(), "Query should succeed");
            assert!(!res.unwrap().is_empty(), "Query should return data");
        }
    }

    #[test]
    fn test_rate_limiting() {
        let (mut deps, env, _info) = proper_instantiate();
        let client_info = mock_info(CLIENT, &[]);

        // Try to post too many jobs (rate limit is 5 per day)
        for i in 0..6 {
            let job_msg = ExecuteMsg::PostJob {
                title: format!("Job {}", i),
                description: "Test job".to_string(),
                budget: Uint128::new(5000),
                category: "Development".to_string(),
                skills_required: vec!["Rust".to_string()],
                duration_days: 30,
                documents: None,
                milestones: None,
            };

            if i < 5 {
                // First 5 should succeed
                assert!(execute(deps.as_mut(), env.clone(), client_info.clone(), job_msg).is_ok());
            } else {
                // 6th should fail due to rate limiting
                let err =
                    execute(deps.as_mut(), env.clone(), client_info.clone(), job_msg).unwrap_err();
                match err {
                    ContractError::RateLimitExceeded { .. } => {} // Expected
                    _ => panic!("Expected rate limit error"),
                }
            }
        }
    }
}

// Integration tests for full workflow
mod integration_tests {
    use super::*;

    #[test]
    fn test_full_job_workflow() {
        let mut deps = mock_dependencies();
        let env = mock_env();

        // Initialize contract
        let init_msg = InstantiateMsg {
            admin: Some(ADMIN.to_string()),
            platform_fee_percent: Some(5),
            min_escrow_amount: Some(Uint128::new(1000)),
            dispute_period_days: Some(7),
            max_job_duration_days: Some(365),
        };
        let init_info = mock_info(ADMIN, &[]);
        instantiate(deps.as_mut(), env.clone(), init_info, init_msg).unwrap();

        // 1. Client posts job
        let post_info = mock_info(CLIENT, &[]);
        let post_msg = ExecuteMsg::PostJob {
            title: "Build a DApp".to_string(),
            description: "Need a decentralized application built on Xion".to_string(),
            budget: Uint128::new(10000),
            category: "Development".to_string(),
            skills_required: vec![
                "Rust".to_string(),
                "CosmWasm".to_string(),
                "React".to_string(),
            ],
            duration_days: 60,
            documents: Some(vec!["https://docs.example.com/requirements".to_string()]),
            milestones: None,
        };
        execute(deps.as_mut(), env.clone(), post_info, post_msg).unwrap();

        // 2. Freelancer submits proposal
        let proposal_info = mock_info(FREELANCER, &[]);
        let proposal_msg = ExecuteMsg::SubmitProposal {
            job_id: 0,
            // single cover_letter field
            cover_letter: "I have 5 years experience in blockchain development...".to_string(),
            delivery_time_days: 45,
            milestones: None,
        };
        execute(deps.as_mut(), env.clone(), proposal_info, proposal_msg).unwrap();

        // 3. Client accepts proposal
        let accept_info = mock_info(CLIENT, &[]);
        let accept_msg = ExecuteMsg::AcceptProposal {
            job_id: 0,
            proposal_id: 0,
        };
        execute(deps.as_mut(), env.clone(), accept_info, accept_msg).unwrap();

        // 4. Client creates and funds escrow
        let escrow_info = mock_info(CLIENT, &coins(10000, "uxion"));
        let escrow_msg = ExecuteMsg::CreateEscrow { job_id: 0 };
        execute(deps.as_mut(), env.clone(), escrow_info, escrow_msg).unwrap();

        // 5. Freelancer completes job
        let complete_info = mock_info(FREELANCER, &[]);
        let complete_msg = ExecuteMsg::CompleteJob { job_id: 0 };
        execute(deps.as_mut(), env.clone(), complete_info, complete_msg).unwrap();

        // 6. Both parties rate each other
        let client_rating_info = mock_info(CLIENT, &[]);
        let client_rating_msg = ExecuteMsg::SubmitRating {
            job_id: 0,
            rating: 5,
            comment: "Excellent work, delivered on time!".to_string(),
        };
        execute(
            deps.as_mut(),
            env.clone(),
            client_rating_info,
            client_rating_msg,
        )
        .unwrap();

        let freelancer_rating_info = mock_info(FREELANCER, &[]);
        let freelancer_rating_msg = ExecuteMsg::SubmitRating {
            job_id: 0,
            rating: 5,
            comment: "Great client, clear requirements and prompt payment!".to_string(),
        };
        execute(
            deps.as_mut(),
            env,
            freelancer_rating_info,
            freelancer_rating_msg,
        )
        .unwrap();

        // Verify final state
        let job_query = QueryMsg::GetJob { job_id: 0 };
        let res = query(deps.as_ref(), mock_env(), job_query).unwrap();
        let job_response: JobResponse = from_json(&res).unwrap();

        assert_eq!(job_response.job.status, JobStatus::Completed);
    }
}

// Performance and stress tests
mod performance_tests {
    use super::*;

    #[test]
    fn test_batch_job_creation_performance() {
        let mut deps = mock_dependencies();
        let env = mock_env();

        // Initialize
        let init_msg = InstantiateMsg {
            admin: Some(ADMIN.to_string()),
            platform_fee_percent: Some(5),
            min_escrow_amount: Some(Uint128::new(1000)),
            dispute_period_days: Some(7),
            max_job_duration_days: Some(365),
        };
        let init_info = mock_info(ADMIN, &[]);
        instantiate(deps.as_mut(), env.clone(), init_info, init_msg).unwrap();

        // Create 100 jobs to test performance
        let start = std::time::Instant::now();
        for i in 0..100 {
            let post_info = mock_info(&format!("client_{}", i), &[]);
            let post_msg = ExecuteMsg::PostJob {
                title: format!("Job {}", i),
                description: format!("Description for job {}", i),
                budget: Uint128::new(1000 + i as u128),
                category: "Development".to_string(),
                skills_required: vec!["Rust".to_string()],
                duration_days: 30,
                documents: None,
                milestones: None,
            };
            execute(deps.as_mut(), env.clone(), post_info, post_msg).unwrap();
        }
        let duration = start.elapsed();

        println!("Created 100 jobs in {:?}", duration);
        assert!(duration.as_millis() < 1000); // Should complete in under 1 second
    }

    #[test]
    fn test_query_performance() {
        let mut deps = mock_dependencies();
        let env = mock_env();

        // Initialize and create some test data
        let init_msg = InstantiateMsg {
            admin: Some(ADMIN.to_string()),
            platform_fee_percent: Some(5),
            min_escrow_amount: Some(Uint128::new(1000)),
            dispute_period_days: Some(7),
            max_job_duration_days: Some(365),
        };
        let init_info = mock_info(ADMIN, &[]);
        instantiate(deps.as_mut(), env.clone(), init_info, init_msg).unwrap();

        // Create some jobs
        for i in 0..10 {
            let post_info = mock_info(&format!("client_{}", i), &[]);
            let post_msg = ExecuteMsg::PostJob {
                title: format!("Job {}", i),
                description: format!("Description for job {}", i),
                budget: Uint128::new(1000 + i as u128),
                category: "Development".to_string(),
                skills_required: vec!["Rust".to_string()],
                duration_days: 30,
                documents: None,
                milestones: None,
            };
            execute(deps.as_mut(), env.clone(), post_info, post_msg).unwrap();
        }

        // Test query performance
        let start = std::time::Instant::now();
        for _ in 0..100 {
            let query_msg = QueryMsg::GetJobs {
                start_after: None,
                limit: Some(10),
                category: None,
                status: None,
                poster: None,
            };
            let _res = query(deps.as_ref(), env.clone(), query_msg).unwrap();
        }
        let duration = start.elapsed();

        println!("Executed 100 queries in {:?}", duration);
        assert!(duration.as_millis() < 500); // Should complete reasonably quickly
    }
}
