use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
use cosmwasm_std::{coins, Uint128};

use xworks_freelance_contract::contract::{execute, instantiate};
use xworks_freelance_contract::msg::{ExecuteMsg, InstantiateMsg, MilestoneInput, RewardTierInput};
use xworks_freelance_contract::ContractError;

const ADMIN: &str = "admin";
const CLIENT: &str = "client";
const FREELANCER: &str = "freelancer";

mod input_validation_tests {
    use super::*;

    fn setup_contract() -> (
        cosmwasm_std::testing::OwnedDeps<
            cosmwasm_std::testing::MockStorage,
            cosmwasm_std::testing::MockApi,
            cosmwasm_std::testing::MockQuerier,
        >,
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
    fn test_string_length_validation() {
        let (mut deps, env, _) = setup_contract();
        let client_info = mock_info(CLIENT, &coins(5000, "uxion"));

        // Test empty title
        let job_msg = ExecuteMsg::PostJob {
            title: "".to_string(),
            description: "Valid description".to_string(),
            budget: Uint128::new(5000),
            category: "Development".to_string(),
            skills_required: vec!["Rust".to_string()],
            duration_days: 30,
            documents: None,
            milestones: None,
        };

        let result = execute(deps.as_mut(), env.clone(), client_info.clone(), job_msg);
        assert!(result.is_err());

        // Test overly long title (>100 characters)
        let long_title = "a".repeat(101);
        let job_msg = ExecuteMsg::PostJob {
            title: long_title,
            description: "Valid description".to_string(),
            budget: Uint128::new(5000),
            category: "Development".to_string(),
            skills_required: vec!["Rust".to_string()],
            duration_days: 30,
            documents: None,
            milestones: None,
        };

        let result = execute(deps.as_mut(), env.clone(), client_info.clone(), job_msg);
        assert!(result.is_err());

        // Test empty description
        let job_msg = ExecuteMsg::PostJob {
            title: "Valid Title".to_string(),
            description: "".to_string(),
            budget: Uint128::new(5000),
            category: "Development".to_string(),
            skills_required: vec!["Rust".to_string()],
            duration_days: 30,
            documents: None,
            milestones: None,
        };

        let result = execute(deps.as_mut(), env.clone(), client_info.clone(), job_msg);
        assert!(result.is_err());

        // Test overly long description (>1000 characters)
        let long_description = "a".repeat(1001);
        let job_msg = ExecuteMsg::PostJob {
            title: "Valid Title".to_string(),
            description: long_description,
            budget: Uint128::new(5000),
            category: "Development".to_string(),
            skills_required: vec!["Rust".to_string()],
            duration_days: 30,
            documents: None,
            milestones: None,
        };

        let result = execute(deps.as_mut(), env, client_info, job_msg);
        assert!(result.is_err());
    }

    #[test]
    fn test_special_characters_and_malformed_inputs() {
        let (mut deps, env, _) = setup_contract();
        let client_info = mock_info(CLIENT, &coins(5000, "uxion"));

        // Test SQL injection attempt in title
        let job_msg = ExecuteMsg::PostJob {
            title: "'; DROP TABLE jobs; --".to_string(),
            description: "Valid description".to_string(),
            budget: Uint128::new(5000),
            category: "Development".to_string(),
            skills_required: vec!["Rust".to_string()],
            duration_days: 30,
            documents: None,
            milestones: None,
        };

        let result = execute(deps.as_mut(), env.clone(), client_info.clone(), job_msg);
        // Should either succeed (sanitized) or fail (rejected), but not crash

        // Test XSS attempt in description
        let job_msg = ExecuteMsg::PostJob {
            title: "Valid Title".to_string(),
            description: "<script>alert('xss')</script>".to_string(),
            budget: Uint128::new(5000),
            category: "Development".to_string(),
            skills_required: vec!["Rust".to_string()],
            duration_days: 30,
            documents: None,
            milestones: None,
        };

        let result = execute(deps.as_mut(), env.clone(), client_info.clone(), job_msg);
        // Should either succeed (sanitized) or fail (rejected), but not crash

        // Test unicode characters
        let job_msg = ExecuteMsg::PostJob {
            title: "Unicode Test 🚀 ñáéíóú".to_string(),
            description: "Testing unicode characters: 中文 العربية русский".to_string(),
            budget: Uint128::new(5000),
            category: "Development".to_string(),
            skills_required: vec!["Rust".to_string()],
            duration_days: 30,
            documents: None,
            milestones: None,
        };

        let result = execute(deps.as_mut(), env.clone(), client_info.clone(), job_msg);
        // Unicode should be allowed

        // Test null bytes and control characters
        let job_msg = ExecuteMsg::PostJob {
            title: "Title with\0null".to_string(),
            description: "Description with\ttab and\nnewline".to_string(),
            budget: Uint128::new(5000),
            category: "Development".to_string(),
            skills_required: vec!["Rust".to_string()],
            duration_days: 30,
            documents: None,
            milestones: None,
        };

        let result = execute(deps.as_mut(), env, client_info, job_msg);
        // Should handle control characters appropriately
    }

    #[test]
    fn test_numeric_validation_edge_cases() {
        let (mut deps, env, _) = setup_contract();
        let client_info = mock_info(CLIENT, &coins(5000, "uxion"));

        // Test zero budget
        let job_msg = ExecuteMsg::PostJob {
            title: "Zero Budget Job".to_string(),
            description: "Testing zero budget".to_string(),
            budget: Uint128::zero(),
            category: "Development".to_string(),
            skills_required: vec!["Rust".to_string()],
            duration_days: 30,
            documents: None,
            milestones: None,
        };

        let result = execute(deps.as_mut(), env.clone(), client_info.clone(), job_msg);
        assert!(result.is_err());

        // Test maximum value budget
        let job_msg = ExecuteMsg::PostJob {
            title: "Max Budget Job".to_string(),
            description: "Testing maximum budget".to_string(),
            budget: Uint128::MAX,
            category: "Development".to_string(),
            skills_required: vec!["Rust".to_string()],
            duration_days: 30,
            documents: None,
            milestones: None,
        };

        let result = execute(deps.as_mut(), env.clone(), client_info.clone(), job_msg);
        // Should handle very large numbers appropriately

        // Test zero duration
        let job_msg = ExecuteMsg::PostJob {
            title: "Zero Duration Job".to_string(),
            description: "Testing zero duration".to_string(),
            budget: Uint128::new(5000),
            category: "Development".to_string(),
            skills_required: vec!["Rust".to_string()],
            duration_days: 0,
            documents: None,
            milestones: None,
        };

        let result = execute(deps.as_mut(), env.clone(), client_info.clone(), job_msg);
        assert!(result.is_err());

        // Test excessive duration (> max allowed)
        let job_msg = ExecuteMsg::PostJob {
            title: "Long Duration Job".to_string(),
            description: "Testing excessive duration".to_string(),
            budget: Uint128::new(5000),
            category: "Development".to_string(),
            skills_required: vec!["Rust".to_string()],
            duration_days: 1000, // > 365 max
            documents: None,
            milestones: None,
        };

        let result = execute(deps.as_mut(), env, client_info, job_msg);
        assert!(result.is_err());
    }

    #[test]
    fn test_array_validation() {
        let (mut deps, env, _) = setup_contract();
        let client_info = mock_info(CLIENT, &coins(5000, "uxion"));

        // Test empty skills array
        let job_msg = ExecuteMsg::PostJob {
            title: "No Skills Job".to_string(),
            description: "Testing empty skills".to_string(),
            budget: Uint128::new(5000),
            category: "Development".to_string(),
            skills_required: vec![],
            duration_days: 30,
            documents: None,
            milestones: None,
        };

        let result = execute(deps.as_mut(), env.clone(), client_info.clone(), job_msg);
        assert!(result.is_err());

        // Test too many skills (> 20)
        let many_skills: Vec<String> = (0..25).map(|i| format!("Skill{}", i)).collect();
        let job_msg = ExecuteMsg::PostJob {
            title: "Many Skills Job".to_string(),
            description: "Testing too many skills".to_string(),
            budget: Uint128::new(5000),
            category: "Development".to_string(),
            skills_required: many_skills,
            duration_days: 30,
            documents: None,
            milestones: None,
        };

        let result = execute(deps.as_mut(), env.clone(), client_info.clone(), job_msg);
        assert!(result.is_err());

        // Test empty skill names
        let job_msg = ExecuteMsg::PostJob {
            title: "Empty Skill Job".to_string(),
            description: "Testing empty skill names".to_string(),
            budget: Uint128::new(5000),
            category: "Development".to_string(),
            skills_required: vec!["".to_string(), "Valid Skill".to_string()],
            duration_days: 30,
            documents: None,
            milestones: None,
        };

        let result = execute(deps.as_mut(), env, client_info, job_msg);
        assert!(result.is_err());
    }

    #[test]
    fn test_milestone_validation() {
        let (mut deps, env, _) = setup_contract();
        let client_info = mock_info(CLIENT, &coins(5000, "uxion"));

        // Test milestone amounts not summing to budget
        let milestones = vec![
            MilestoneInput {
                title: "Milestone 1".to_string(),
                description: "First milestone".to_string(),
                amount: Uint128::new(2000),
                deadline_days: 15,
            },
            MilestoneInput {
                title: "Milestone 2".to_string(),
                description: "Second milestone".to_string(),
                amount: Uint128::new(2000), // Total 4000, but budget is 5000
                deadline_days: 30,
            },
        ];

        let job_msg = ExecuteMsg::PostJob {
            title: "Milestone Test Job".to_string(),
            description: "Testing milestone validation".to_string(),
            budget: Uint128::new(5000),
            category: "Development".to_string(),
            skills_required: vec!["Rust".to_string()],
            duration_days: 30,
            documents: None,
            milestones: Some(milestones),
        };

        let result = execute(deps.as_mut(), env.clone(), client_info.clone(), job_msg);
        assert!(result.is_err());

        // Test empty milestone title
        let milestones = vec![MilestoneInput {
            title: "".to_string(),
            description: "Empty title milestone".to_string(),
            amount: Uint128::new(5000),
            deadline_days: 30,
        }];

        let job_msg = ExecuteMsg::PostJob {
            title: "Empty Milestone Title Job".to_string(),
            description: "Testing empty milestone title".to_string(),
            budget: Uint128::new(5000),
            category: "Development".to_string(),
            skills_required: vec!["Rust".to_string()],
            duration_days: 30,
            documents: None,
            milestones: Some(milestones),
        };

        let result = execute(deps.as_mut(), env.clone(), client_info.clone(), job_msg);
        assert!(result.is_err());

        // Test milestone with zero amount
        let milestones = vec![MilestoneInput {
            title: "Zero Amount Milestone".to_string(),
            description: "Testing zero amount".to_string(),
            amount: Uint128::zero(),
            deadline_days: 30,
        }];

        let job_msg = ExecuteMsg::PostJob {
            title: "Zero Milestone Job".to_string(),
            description: "Testing zero milestone amount".to_string(),
            budget: Uint128::new(5000),
            category: "Development".to_string(),
            skills_required: vec!["Rust".to_string()],
            duration_days: 30,
            documents: None,
            milestones: Some(milestones),
        };

        let result = execute(deps.as_mut(), env, client_info, job_msg);
        assert!(result.is_err());
    }

    #[test]
    fn test_address_validation() {
        let (mut deps, env, _) = setup_contract();
        let admin_info = mock_info(ADMIN, &[]);

        // Test invalid address format in BlockAddress
        let block_msg = ExecuteMsg::BlockAddress {
            address: "invalid_address_format".to_string(),
            reason: "Testing invalid address".to_string(),
        };

        let result = execute(deps.as_mut(), env.clone(), admin_info.clone(), block_msg);
        assert!(result.is_err());

        // Test empty address
        let block_msg = ExecuteMsg::BlockAddress {
            address: "".to_string(),
            reason: "Testing empty address".to_string(),
        };

        let result = execute(deps.as_mut(), env.clone(), admin_info.clone(), block_msg);
        assert!(result.is_err());

        // Test very long address
        let long_address = "a".repeat(100);
        let block_msg = ExecuteMsg::BlockAddress {
            address: long_address,
            reason: "Testing long address".to_string(),
        };

        let result = execute(deps.as_mut(), env, admin_info, block_msg);
        assert!(result.is_err());
    }

    #[test]
    fn test_category_validation() {
        let (mut deps, env, _) = setup_contract();
        let client_info = mock_info(CLIENT, &coins(5000, "uxion"));

        // Test empty category
        let job_msg = ExecuteMsg::PostJob {
            title: "Empty Category Job".to_string(),
            description: "Testing empty category".to_string(),
            budget: Uint128::new(5000),
            category: "".to_string(),
            skills_required: vec!["Rust".to_string()],
            duration_days: 30,
            documents: None,
            milestones: None,
        };

        let result = execute(deps.as_mut(), env.clone(), client_info.clone(), job_msg);
        assert!(result.is_err());

        // Test overly long category (>50 characters)
        let long_category = "a".repeat(51);
        let job_msg = ExecuteMsg::PostJob {
            title: "Long Category Job".to_string(),
            description: "Testing long category".to_string(),
            budget: Uint128::new(5000),
            category: long_category,
            skills_required: vec!["Rust".to_string()],
            duration_days: 30,
            documents: None,
            milestones: None,
        };

        let result = execute(deps.as_mut(), env, client_info, job_msg);
        assert!(result.is_err());
    }

    #[test]
    fn test_bounty_validation() {
        let (mut deps, env, _) = setup_contract();
        let client_info = mock_info(CLIENT, &coins(10000, "uxion"));

        // Test bounty with invalid reward distribution
        let invalid_rewards = vec![
            RewardTierInput {
                position: 1,
                percentage: 60, // Total will be 120%
            },
            RewardTierInput {
                position: 2,
                percentage: 60,
            },
        ];

        let bounty_msg = ExecuteMsg::CreateBounty {
            title: "Invalid Reward Bounty".to_string(),
            description: "Testing invalid reward distribution".to_string(),
            requirements: vec!["Complete task".to_string()],
            total_reward: Uint128::new(10000),
            category: "Development".to_string(),
            skills_required: vec!["Rust".to_string()],
            submission_deadline_days: 30,
            review_period_days: 7,
            max_winners: 2,
            reward_distribution: invalid_rewards,
            documents: None,
        };

        let result = execute(deps.as_mut(), env.clone(), client_info.clone(), bounty_msg);
        assert!(result.is_err());

        // Test bounty with zero max winners
        let valid_rewards = vec![RewardTierInput {
            position: 1,
            percentage: 100,
        }];

        let bounty_msg = ExecuteMsg::CreateBounty {
            title: "Zero Winners Bounty".to_string(),
            description: "Testing zero max winners".to_string(),
            requirements: vec!["Complete task".to_string()],
            total_reward: Uint128::new(10000),
            category: "Development".to_string(),
            skills_required: vec!["Rust".to_string()],
            submission_deadline_days: 30,
            review_period_days: 7,
            max_winners: 0,
            reward_distribution: valid_rewards,
            documents: None,
        };

        let result = execute(deps.as_mut(), env, client_info, bounty_msg);
        assert!(result.is_err());
    }

    #[test]
    fn test_config_validation_on_instantiate() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        let info = mock_info(ADMIN, &[]);

        // Test platform fee too high
        let msg = InstantiateMsg {
            admin: Some(ADMIN.to_string()),
            platform_fee_percent: Some(15), // > 10% max
            min_escrow_amount: Some(Uint128::new(1000)),
            dispute_period_days: Some(7),
            max_job_duration_days: Some(365),
        };

        let result = instantiate(deps.as_mut(), env.clone(), info.clone(), msg);
        assert!(matches!(
            result,
            Err(ContractError::PlatformFeeTooHigh { max: 10 })
        ));

        // Test valid config
        let msg = InstantiateMsg {
            admin: Some(ADMIN.to_string()),
            platform_fee_percent: Some(5),
            min_escrow_amount: Some(Uint128::new(1000)),
            dispute_period_days: Some(7),
            max_job_duration_days: Some(365),
        };

        let result = instantiate(deps.as_mut(), env, info, msg);
        assert!(result.is_ok());
    }
}
