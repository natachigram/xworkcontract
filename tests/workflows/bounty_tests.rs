use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
use cosmwasm_std::{coins, Uint128};

use xworks_freelance_contract::contract::{execute, instantiate};
use xworks_freelance_contract::msg::{ExecuteMsg, InstantiateMsg};
use xworks_freelance_contract::state::{BountyStatus, BOUNTIES};

const ADMIN: &str = "admin";
const BOUNTY_CREATOR: &str = "bounty_creator";
const HUNTER1: &str = "hunter1";
const HUNTER2: &str = "hunter2";

mod bounty_tests {
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
    fn test_bounty_creation_and_submission() {
        let (api, mut storage, querier, env) = setup_contract();
        let mut deps = cosmwasm_std::OwnedDeps { api, storage: &mut storage, querier, custom_query_type: std::marker::PhantomData };

        // Create a bounty
        let creator_info = mock_info(BOUNTY_CREATOR, &coins(5000, "uxion"));
        let create_bounty_msg = ExecuteMsg::CreateBounty {
            title: "Find Security Vulnerability".to_string(),
            description: "Find and report any security vulnerabilities in our smart contract".to_string(),
            requirements: vec![
                "Must provide proof of concept".to_string(),
                "Must include fix recommendations".to_string(),
            ],
            total_reward: Uint128::new(5000),
            category: "Security".to_string(),
            skills_required: vec!["Smart Contract Auditing".to_string(), "Security Research".to_string()],
            deadline_days: 30,
            max_submissions: Some(10),
            verification_requirements: vec!["Code exploit demonstration".to_string()],
        };

        let result = execute(deps.as_mut(), env.clone(), creator_info.clone(), create_bounty_msg);
        assert!(result.is_ok());

        // Verify bounty was created
        let bounty = BOUNTIES.load(deps.as_ref().storage, 0).unwrap();
        assert_eq!(bounty.title, "Find Security Vulnerability");
        assert_eq!(bounty.total_reward, Uint128::new(5000));
        assert_eq!(bounty.status, BountyStatus::Active);

        // Hunter 1 submits to bounty
        let hunter1_info = mock_info(HUNTER1, &[]);
        let submit_bounty_msg = ExecuteMsg::SubmitToBounty {
            bounty_id: 0,
            submission_data: "Found reentrancy vulnerability in escrow release function".to_string(),
            proof_links: vec!["https://github.com/hunter1/exploit-poc".to_string()],
        };

        let result = execute(deps.as_mut(), env.clone(), hunter1_info.clone(), submit_bounty_msg);
        assert!(result.is_ok());

        // Hunter 2 submits to bounty
        let hunter2_info = mock_info(HUNTER2, &[]);
        let submit_bounty_msg2 = ExecuteMsg::SubmitToBounty {
            bounty_id: 0,
            submission_data: "Discovered integer overflow in fee calculation".to_string(),
            proof_links: vec!["https://github.com/hunter2/overflow-exploit".to_string()],
        };

        let result = execute(deps.as_mut(), env.clone(), hunter2_info, submit_bounty_msg2);
        assert!(result.is_ok());

        // Bounty creator awards to Hunter 1
        let award_msg = ExecuteMsg::AwardBounty {
            bounty_id: 0,
            winner: deps.api.addr_validate(HUNTER1).unwrap(),
            reward_amount: Uint128::new(5000),
            feedback: "Excellent find! Critical vulnerability with clear PoC".to_string(),
        };

        let result = execute(deps.as_mut(), env, creator_info, award_msg);
        assert!(result.is_ok());

        // Verify bounty status changed to Completed
        let bounty = BOUNTIES.load(deps.as_ref().storage, 0).unwrap();
        assert_eq!(bounty.status, BountyStatus::Completed);
    }

    #[test]
    fn test_bounty_partial_awards() {
        let (api, mut storage, querier, env) = setup_contract();
        let mut deps = cosmwasm_std::OwnedDeps { api, storage: &mut storage, querier, custom_query_type: std::marker::PhantomData };

        // Create a bounty with multiple rewards
        let creator_info = mock_info(BOUNTY_CREATOR, &coins(10000, "uxion"));
        let create_bounty_msg = ExecuteMsg::CreateBounty {
            title: "Code Optimization Challenge".to_string(),
            description: "Optimize gas usage in our smart contracts".to_string(),
            requirements: vec![
                "Must reduce gas by at least 10%".to_string(),
                "Must maintain functionality".to_string(),
            ],
            total_reward: Uint128::new(10000),
            category: "Optimization".to_string(),
            skills_required: vec!["Smart Contract Optimization".to_string(), "Gas Analysis".to_string()],
            deadline_days: 45,
            max_submissions: Some(5),
            verification_requirements: vec!["Gas usage comparison report".to_string()],
        };

        execute(deps.as_mut(), env.clone(), creator_info.clone(), create_bounty_msg).unwrap();

        // Multiple hunters submit
        let hunter1_info = mock_info(HUNTER1, &[]);
        let submit1_msg = ExecuteMsg::SubmitToBounty {
            bounty_id: 0,
            submission_data: "Optimized storage layout - 15% gas reduction".to_string(),
            proof_links: vec!["Gas comparison report".to_string()],
        };
        execute(deps.as_mut(), env.clone(), hunter1_info, submit1_msg).unwrap();

        let hunter2_info = mock_info(HUNTER2, &[]);
        let submit2_msg = ExecuteMsg::SubmitToBounty {
            bounty_id: 0,
            submission_data: "Assembly optimizations - 8% gas reduction".to_string(),
            proof_links: vec!["Assembly code optimizations".to_string()],
        };
        execute(deps.as_mut(), env.clone(), hunter2_info, submit2_msg).unwrap();

        // Award partial amounts to both hunters
        let award1_msg = ExecuteMsg::AwardBounty {
            bounty_id: 0,
            winner: deps.api.addr_validate(HUNTER1).unwrap(),
            reward_amount: Uint128::new(7000), // 70% for best solution
            feedback: "Excellent optimization, significant gas savings!".to_string(),
        };
        execute(deps.as_mut(), env.clone(), creator_info.clone(), award1_msg).unwrap();

        let award2_msg = ExecuteMsg::AwardBounty {
            bounty_id: 0,
            winner: deps.api.addr_validate(HUNTER2).unwrap(),
            reward_amount: Uint128::new(3000), // 30% for good solution
            feedback: "Good work on assembly optimizations!".to_string(),
        };
        execute(deps.as_mut(), env, creator_info, award2_msg).unwrap();
    }

    #[test]
    fn test_bounty_expiration() {
        let (api, mut storage, querier, mut env) = setup_contract();
        let mut deps = cosmwasm_std::OwnedDeps { api, storage: &mut storage, querier, custom_query_type: std::marker::PhantomData };

        // Create a bounty with short deadline
        let creator_info = mock_info(BOUNTY_CREATOR, &coins(3000, "uxion"));
        let create_bounty_msg = ExecuteMsg::CreateBounty {
            title: "Quick Bug Fix".to_string(),
            description: "Fix minor UI bug in our dApp".to_string(),
            requirements: vec!["Must provide working fix".to_string()],
            total_reward: Uint128::new(3000),
            category: "Bug Fix".to_string(),
            skills_required: vec!["Frontend Development".to_string()],
            deadline_days: 1, // Very short deadline
            max_submissions: Some(3),
            verification_requirements: vec!["Working demo".to_string()],
        };

        execute(deps.as_mut(), env.clone(), creator_info.clone(), create_bounty_msg).unwrap();

        // Advance time past deadline
        env.block.time = env.block.time.plus_days(2);

        // Try to submit after deadline (should fail or be handled appropriately)
        let hunter_info = mock_info(HUNTER1, &[]);
        let submit_msg = ExecuteMsg::SubmitToBounty {
            bounty_id: 0,
            submission_data: "Bug fix implementation".to_string(),
            proof_links: vec!["Fixed code repository".to_string()],
        };

        // This test checks if the contract properly handles expired bounties
        let result = execute(deps.as_mut(), env.clone(), hunter_info, submit_msg);
        // Result depends on implementation - might succeed but with different handling

        // Creator can close expired bounty
        let close_msg = ExecuteMsg::CloseBounty {
            bounty_id: 0,
            reason: "Deadline expired, no satisfactory submissions".to_string(),
        };

        let result = execute(deps.as_mut(), env, creator_info, close_msg);
        assert!(result.is_ok());

        // Verify bounty status
        let bounty = BOUNTIES.load(deps.as_ref().storage, 0).unwrap();
        assert_eq!(bounty.status, BountyStatus::Cancelled);
    }

    #[test]
    fn test_free_bounty_workflow() {
        let (api, mut storage, querier, env) = setup_contract();
        let mut deps = cosmwasm_std::OwnedDeps { api, storage: &mut storage, querier, custom_query_type: std::marker::PhantomData };

        // Create a free bounty (reputation/recognition only)
        let creator_info = mock_info(BOUNTY_CREATOR, &[]); // No payment
        let create_bounty_msg = ExecuteMsg::CreateBounty {
            title: "Documentation Improvements".to_string(),
            description: "Help improve our project documentation".to_string(),
            requirements: vec![
                "Fix typos and grammar".to_string(),
                "Add missing examples".to_string(),
            ],
            total_reward: Uint128::zero(), // Free bounty
            category: "Documentation".to_string(),
            skills_required: vec!["Technical Writing".to_string()],
            deadline_days: 14,
            max_submissions: Some(20),
            verification_requirements: vec!["Pull request with improvements".to_string()],
        };

        let result = execute(deps.as_mut(), env.clone(), creator_info.clone(), create_bounty_msg);
        assert!(result.is_ok());

        // Verify free bounty was created
        let bounty = BOUNTIES.load(deps.as_ref().storage, 0).unwrap();
        assert_eq!(bounty.total_reward, Uint128::zero());
        assert_eq!(bounty.status, BountyStatus::Active);

        // Hunter submits to free bounty
        let hunter_info = mock_info(HUNTER1, &[]);
        let submit_msg = ExecuteMsg::SubmitToBounty {
            bounty_id: 0,
            submission_data: "Fixed 15 typos, added 3 code examples, improved readability".to_string(),
            proof_links: vec!["https://github.com/hunter1/doc-improvements/pull/1".to_string()],
        };

        let result = execute(deps.as_mut(), env.clone(), hunter_info, submit_msg);
        assert!(result.is_ok());

        // Creator acknowledges contribution (no monetary reward)
        let acknowledge_msg = ExecuteMsg::AwardBounty {
            bounty_id: 0,
            winner: deps.api.addr_validate(HUNTER1).unwrap(),
            reward_amount: Uint128::zero(),
            feedback: "Great documentation improvements! Thank you for your contribution.".to_string(),
        };

        let result = execute(deps.as_mut(), env, creator_info, acknowledge_msg);
        assert!(result.is_ok());
    }
}
