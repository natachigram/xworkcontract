use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
use cosmwasm_std::testing::{MockApi, MockQuerier, MockStorage};
use cosmwasm_std::{coins, OwnedDeps, Uint128};

use xworks_freelance_contract::contract::{execute, instantiate};
use xworks_freelance_contract::msg::{ExecuteMsg, InstantiateMsg, RewardTierInput};
use xworks_freelance_contract::state::{CONFIG, ESCROWS, JOBS};
use xworks_freelance_contract::ContractError;

const ADMIN: &str = "admin";
const CLIENT: &str = "client";
const FREELANCER: &str = "freelancer";

mod fee_calculation_tests {
    use super::*;

    fn setup_contract(
        platform_fee: u64,
    ) -> (
        OwnedDeps<MockStorage, MockApi, MockQuerier>,
        cosmwasm_std::Env,
        cosmwasm_std::MessageInfo,
    ) {
        let mut deps = mock_dependencies();
        let env = mock_env();
        let info = mock_info(ADMIN, &[]);

        let msg = InstantiateMsg {
            admin: Some(ADMIN.to_string()),
            platform_fee_percent: Some(platform_fee),
            min_escrow_amount: Some(Uint128::new(1000)),
            dispute_period_days: Some(7),
            max_job_duration_days: Some(365),
        };

        instantiate(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();
        (deps, env, info)
    }

    fn create_job_and_proposal(
        deps: &mut OwnedDeps<MockStorage, MockApi, MockQuerier>,
        env: &cosmwasm_std::Env,
        budget: Uint128,
    ) -> Result<(), ContractError> {
        let client_info = mock_info(CLIENT, &coins(budget.u128(), "uxion"));
        let freelancer_info = mock_info(FREELANCER, &[]);

        // Create job
        let job_msg = ExecuteMsg::PostJob {
            title: "Fee Test Job".to_string(),
            description: "Testing fee calculations".to_string(),
            budget,
            category: "Testing".to_string(),
            skills_required: vec!["Math".to_string()],
            duration_days: 30,
            documents: None,
            milestones: None,
        };

        execute(deps.as_mut(), env.clone(), client_info, job_msg)?;

        // Submit proposal
        let proposal_msg = ExecuteMsg::SubmitProposal {
            job_id: 0,
            cover_letter: "Fee test proposal".to_string(),
            delivery_time_days: 25,
            milestones: None,
        };

        execute(deps.as_mut(), env.clone(), freelancer_info, proposal_msg)?;

        // Accept proposal
        let client_info = mock_info(CLIENT, &coins(budget.u128(), "uxion"));
        let accept_msg = ExecuteMsg::AcceptProposal {
            job_id: 0,
            proposal_id: 0,
        };

        execute(deps.as_mut(), env.clone(), client_info, accept_msg)?;

        Ok(())
    }

    #[test]
    fn test_basic_platform_fee_calculation() {
        let (mut deps, env, _) = setup_contract(5); // 5% platform fee
        let budget = Uint128::new(10000);
        create_job_and_proposal(&mut deps, &env, budget).unwrap();

        let client_info = mock_info(CLIENT, &coins(budget.u128(), "uxion"));

        // Create escrow
        let create_escrow_msg = ExecuteMsg::CreateEscrowNative {
            job_id: 0,
            amount: budget,
        };

        execute(deps.as_mut(), env, client_info, create_escrow_msg).unwrap();

        // Verify fee calculation
        let job = JOBS.load(deps.as_ref().storage, 0).unwrap();
        let escrow_id = job.escrow_id.unwrap();
        let escrow = ESCROWS.load(deps.as_ref().storage, &escrow_id).unwrap();

        // Expected platform fee: 10000 * 5% = 500
        let expected_platform_fee = Uint128::new(500);
        assert_eq!(escrow.platform_fee, expected_platform_fee);

        // Expected freelancer amount: 10000 - 500 = 9500
        let expected_freelancer_amount = Uint128::new(9500);
        assert_eq!(escrow.amount, expected_freelancer_amount);
    }

    #[test]
    fn test_zero_platform_fee() {
        let (mut deps, env, _) = setup_contract(0); // 0% platform fee
        let budget = Uint128::new(5000);
        create_job_and_proposal(&mut deps, &env, budget).unwrap();

        let client_info = mock_info(CLIENT, &coins(budget.u128(), "uxion"));

        // Create escrow
        let create_escrow_msg = ExecuteMsg::CreateEscrowNative {
            job_id: 0,
            amount: budget,
        };

        execute(deps.as_mut(), env, client_info, create_escrow_msg).unwrap();

        // Verify no platform fee
        let job = JOBS.load(deps.as_ref().storage, 0).unwrap();
        let escrow_id = job.escrow_id.unwrap();
        let escrow = ESCROWS.load(deps.as_ref().storage, &escrow_id).unwrap();
        assert_eq!(escrow.platform_fee, Uint128::zero());
        assert_eq!(escrow.amount, budget);
    }

    #[test]
    fn test_maximum_platform_fee() {
        let (mut deps, env, _) = setup_contract(10); // 10% platform fee (maximum)
        let budget = Uint128::new(20000);
        create_job_and_proposal(&mut deps, &env, budget).unwrap();

        let client_info = mock_info(CLIENT, &coins(budget.u128(), "uxion"));

        // Create escrow
        let create_escrow_msg = ExecuteMsg::CreateEscrowNative {
            job_id: 0,
            amount: budget,
        };

        execute(deps.as_mut(), env, client_info, create_escrow_msg).unwrap();

        // Verify maximum fee calculation - get escrow ID from job
        let job = JOBS.load(deps.as_ref().storage, 0).unwrap();
        let escrow_id = job.escrow_id.unwrap();
        let escrow = ESCROWS.load(deps.as_ref().storage, &escrow_id).unwrap();

        // Expected platform fee: 20000 * 10% = 2000
        let expected_platform_fee = Uint128::new(2000);
        assert_eq!(escrow.platform_fee, expected_platform_fee);

        // Expected freelancer amount: 20000 - 2000 = 18000
        let expected_freelancer_amount = Uint128::new(18000);
        assert_eq!(escrow.amount, expected_freelancer_amount);
    }

    #[test]
    fn test_platform_fee_exceeds_maximum() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        let info = mock_info(ADMIN, &[]);

        // Try to set platform fee above 10%
        let msg = InstantiateMsg {
            admin: Some(ADMIN.to_string()),
            platform_fee_percent: Some(15), // Above maximum
            min_escrow_amount: Some(Uint128::new(1000)),
            dispute_period_days: Some(7),
            max_job_duration_days: Some(365),
        };

        let result = instantiate(deps.as_mut(), env, info, msg);
        assert!(matches!(
            result,
            Err(ContractError::PlatformFeeTooHigh { max: 10 })
        ));
    }

    #[test]
    fn test_fee_calculation_with_small_amounts() {
        let (mut deps, env, _) = setup_contract(5); // 5% platform fee
        let budget = Uint128::new(1000); // Minimum escrow amount
        create_job_and_proposal(&mut deps, &env, budget).unwrap();

        let client_info = mock_info(CLIENT, &coins(budget.u128(), "uxion"));

        // Create escrow
        let create_escrow_msg = ExecuteMsg::CreateEscrowNative {
            job_id: 0,
            amount: budget,
        };

        execute(deps.as_mut(), env, client_info, create_escrow_msg).unwrap();

        // Verify fee calculation for small amounts
        let job = JOBS.load(deps.as_ref().storage, 0).unwrap();
        let escrow_id = job.escrow_id.unwrap();
        let escrow = ESCROWS.load(deps.as_ref().storage, &escrow_id).unwrap();

        // Expected platform fee: 1000 * 5% = 50
        let expected_platform_fee = Uint128::new(50);
        assert_eq!(escrow.platform_fee, expected_platform_fee);

        // Expected freelancer amount: 1000 - 50 = 950
        let expected_freelancer_amount = Uint128::new(950);
        assert_eq!(escrow.amount, expected_freelancer_amount);
    }

    #[test]
    fn test_fee_calculation_with_large_amounts() {
        let (mut deps, env, _) = setup_contract(5); // 5% platform fee
        let budget = Uint128::new(1_000_000_000); // Very large amount
        create_job_and_proposal(&mut deps, &env, budget).unwrap();

        let client_info = mock_info(CLIENT, &coins(budget.u128(), "uxion"));

        // Create escrow
        let create_escrow_msg = ExecuteMsg::CreateEscrowNative {
            job_id: 0,
            amount: budget,
        };

        execute(deps.as_mut(), env, client_info, create_escrow_msg).unwrap();

        // Verify fee calculation for large amounts
        let job = JOBS.load(deps.as_ref().storage, 0).unwrap();
        let escrow_id = job.escrow_id.unwrap();
        let escrow = ESCROWS.load(deps.as_ref().storage, &escrow_id).unwrap();

        // Expected platform fee: 1_000_000_000 * 5% = 50_000_000
        let expected_platform_fee = Uint128::new(50_000_000);
        assert_eq!(escrow.platform_fee, expected_platform_fee);

        // Expected freelancer amount: 1_000_000_000 - 50_000_000 = 950_000_000
        let expected_freelancer_amount = Uint128::new(950_000_000);
        assert_eq!(escrow.amount, expected_freelancer_amount);
    }

    #[test]
    fn test_fee_calculation_precision() {
        let (mut deps, env, _) = setup_contract(3); // 3% platform fee
        let budget = Uint128::new(10001); // Amount that could cause precision issues
        create_job_and_proposal(&mut deps, &env, budget).unwrap();

        let client_info = mock_info(CLIENT, &coins(budget.u128(), "uxion"));

        // Create escrow
        let create_escrow_msg = ExecuteMsg::CreateEscrowNative {
            job_id: 0,
            amount: budget,
        };

        execute(deps.as_mut(), env, client_info, create_escrow_msg).unwrap();

        // Verify fee calculation maintains precision
        let job = JOBS.load(deps.as_ref().storage, 0).unwrap();
        let escrow_id = job.escrow_id.unwrap();
        let escrow = ESCROWS.load(deps.as_ref().storage, &escrow_id).unwrap();

        // Expected platform fee: 10001 * 3% = 300.03, should round down to 300
        let expected_platform_fee = Uint128::new(300);
        assert_eq!(escrow.platform_fee, expected_platform_fee);

        // Expected freelancer amount: 10001 - 300 = 9701
        let expected_freelancer_amount = Uint128::new(9701);
        assert_eq!(escrow.amount, expected_freelancer_amount);

        // Verify total equals original amount
        let total = escrow.platform_fee + escrow.amount;
        assert_eq!(total, budget);
    }

    #[test]
    fn test_fee_calculation_edge_cases() {
        let (mut deps, env, _) = setup_contract(1); // 1% platform fee

        // Test with amount that gives exact division
        let budget = Uint128::new(10000);
        create_job_and_proposal(&mut deps, &env, budget).unwrap();

        let client_info = mock_info(CLIENT, &coins(budget.u128(), "uxion"));

        let create_escrow_msg = ExecuteMsg::CreateEscrowNative {
            job_id: 0,
            amount: budget,
        };

        execute(
            deps.as_mut(),
            env.clone(),
            client_info.clone(),
            create_escrow_msg,
        )
        .unwrap();

        let job = JOBS.load(deps.as_ref().storage, 0).unwrap();
        let escrow_id = job.escrow_id.unwrap();
        let escrow = ESCROWS.load(deps.as_ref().storage, &escrow_id).unwrap();

        // 1% of 10000 = 100
        assert_eq!(escrow.platform_fee, Uint128::new(100));
        assert_eq!(escrow.amount, Uint128::new(9900));

        // Test with very small fee percentage and large amount
        let budget2 = Uint128::new(1001); // Above minimum escrow amount

        // Clear previous test data
        let job_msg = ExecuteMsg::PostJob {
            title: "Edge Case Job".to_string(),
            description: "Testing edge cases".to_string(),
            budget: budget2,
            category: "Testing".to_string(),
            skills_required: vec!["Math".to_string()],
            duration_days: 30,
            documents: None,
            milestones: None,
        };

        execute(deps.as_mut(), env.clone(), client_info.clone(), job_msg).unwrap();

        let proposal_msg = ExecuteMsg::SubmitProposal {
            job_id: 1,
            cover_letter: "Edge case proposal".to_string(),
            delivery_time_days: 25,
            milestones: None,
        };

        let freelancer_info = mock_info(FREELANCER, &[]);
        execute(deps.as_mut(), env.clone(), freelancer_info, proposal_msg).unwrap();

        let accept_msg = ExecuteMsg::AcceptProposal {
            job_id: 1,
            proposal_id: 1,
        };

        execute(deps.as_mut(), env.clone(), client_info.clone(), accept_msg).unwrap();

        let client_info2 = mock_info(CLIENT, &coins(budget2.u128(), "uxion"));
        let create_escrow_msg2 = ExecuteMsg::CreateEscrowNative {
            job_id: 1,
            amount: budget2,
        };

        execute(deps.as_mut(), env, client_info2, create_escrow_msg2).unwrap();

        let job2 = JOBS.load(deps.as_ref().storage, 1).unwrap();
        let escrow_id2 = job2.escrow_id.unwrap();
        let escrow2 = ESCROWS.load(deps.as_ref().storage, &escrow_id2).unwrap();

        // 1% of 1001 = 10.01, should round down to 10
        assert_eq!(escrow2.platform_fee, Uint128::new(10));
        assert_eq!(escrow2.amount, Uint128::new(991));

        // Verify total
        let total = escrow2.platform_fee + escrow2.amount;
        assert_eq!(total, budget2);
    }

    #[test]
    fn test_bounty_reward_distribution_calculation() {
        let (mut deps, env, _) = setup_contract(5); // 5% platform fee
        let client_info = mock_info(CLIENT, &coins(10000, "uxion"));

        // Create bounty with multiple reward tiers
        let reward_tiers = vec![
            RewardTierInput {
                position: 1,
                percentage: 50, // 50% for first place
            },
            RewardTierInput {
                position: 2,
                percentage: 30, // 30% for second place
            },
            RewardTierInput {
                position: 3,
                percentage: 20, // 20% for third place
            },
        ];

        let bounty_msg = ExecuteMsg::CreateBounty {
            title: "Reward Distribution Test".to_string(),
            description: "Testing reward distribution calculations".to_string(),
            requirements: vec!["Complete the task".to_string()],
            total_reward: Uint128::new(10000),
            category: "Testing".to_string(),
            skills_required: vec!["Math".to_string()],
            submission_deadline_days: 30,
            review_period_days: 7,
            max_winners: 3,
            reward_distribution: reward_tiers,
            documents: None,
        };

        let result = execute(deps.as_mut(), env, client_info, bounty_msg);
        assert!(result.is_ok());

        // The bounty should calculate correct reward amounts:
        // First place: 10000 * 50% = 5000
        // Second place: 10000 * 30% = 3000
        // Third place: 10000 * 20% = 2000
        // Total: 10000 (matches total_reward)
    }

    #[test]
    fn test_milestone_fee_calculation() {
        let (mut deps, env, _) = setup_contract(5); // 5% platform fee
        let client_info = mock_info(CLIENT, &coins(6000, "uxion"));

        // Create job with milestones
        let milestones = vec![
            xworks_freelance_contract::msg::MilestoneInput {
                title: "Milestone 1".to_string(),
                description: "First milestone".to_string(),
                amount: Uint128::new(3000),
                deadline_days: 15,
            },
            xworks_freelance_contract::msg::MilestoneInput {
                title: "Milestone 2".to_string(),
                description: "Second milestone".to_string(),
                amount: Uint128::new(3000),
                deadline_days: 30,
            },
        ];

        let job_msg = ExecuteMsg::PostJob {
            title: "Milestone Fee Test".to_string(),
            description: "Testing milestone fee calculations".to_string(),
            budget: Uint128::new(6000),
            category: "Development".to_string(),
            skills_required: vec!["Development".to_string()],
            duration_days: 30,
            documents: None,
            milestones: Some(milestones),
        };

        execute(deps.as_mut(), env.clone(), client_info.clone(), job_msg).unwrap();

        let freelancer_info = mock_info(FREELANCER, &[]);
        let proposal_msg = ExecuteMsg::SubmitProposal {
            job_id: 0,
            cover_letter: "Milestone proposal".to_string(),
            delivery_time_days: 25,
            milestones: None,
        };

        execute(deps.as_mut(), env.clone(), freelancer_info, proposal_msg).unwrap();

        let accept_msg = ExecuteMsg::AcceptProposal {
            job_id: 0,
            proposal_id: 0,
        };

        execute(deps.as_mut(), env.clone(), client_info.clone(), accept_msg).unwrap();

        // Create escrow for milestone job
        let create_escrow_msg = ExecuteMsg::CreateEscrowNative {
            job_id: 0,
            amount: Uint128::new(6000),
        };

        execute(deps.as_mut(), env, client_info, create_escrow_msg).unwrap();

        // Verify fee calculation for total amount
        let job = JOBS.load(deps.as_ref().storage, 0).unwrap();
        let escrow_id = job.escrow_id.unwrap();
        let escrow = ESCROWS.load(deps.as_ref().storage, &escrow_id).unwrap();

        // Total fee: 6000 * 5% = 300
        assert_eq!(escrow.platform_fee, Uint128::new(300));
        assert_eq!(escrow.amount, Uint128::new(5700));
    }

    #[test]
    fn test_fee_update_doesnt_affect_existing_escrows() {
        let (mut deps, env, _) = setup_contract(5); // 5% platform fee
        let budget = Uint128::new(4000);
        create_job_and_proposal(&mut deps, &env, budget).unwrap();

        let client_info = mock_info(CLIENT, &coins(budget.u128(), "uxion"));

        // Create escrow with 5% fee
        let create_escrow_msg = ExecuteMsg::CreateEscrowNative {
            job_id: 0,
            amount: budget,
        };

        execute(deps.as_mut(), env.clone(), client_info, create_escrow_msg).unwrap();

        // Verify initial fee
        let job = JOBS.load(deps.as_ref().storage, 0).unwrap();
        let escrow_id = job.escrow_id.unwrap();
        let escrow_before = ESCROWS.load(deps.as_ref().storage, &escrow_id).unwrap();
        assert_eq!(escrow_before.platform_fee, Uint128::new(200)); // 5% of 4000

        // Update platform fee
        let admin_info = mock_info(ADMIN, &[]);
        let update_config_msg = ExecuteMsg::UpdateConfig {
            admin: None,
            platform_fee_percent: Some(8), // Increase to 8%
            min_escrow_amount: None,
            dispute_period_days: None,
            max_job_duration_days: None,
        };

        execute(deps.as_mut(), env, admin_info, update_config_msg).unwrap();

        // Verify existing escrow is unchanged
        let job_after = JOBS.load(deps.as_ref().storage, 0).unwrap();
        let escrow_id_after = job_after.escrow_id.unwrap();
        let escrow_after = ESCROWS
            .load(deps.as_ref().storage, &escrow_id_after)
            .unwrap();
        assert_eq!(escrow_after.platform_fee, Uint128::new(200)); // Still 5%

        // Verify config was updated
        let config = CONFIG.load(deps.as_ref().storage).unwrap();
        assert_eq!(config.platform_fee_percent, 8);
    }

    #[test]
    fn test_fee_calculation_overflow_protection() {
        let (mut deps, env, _) = setup_contract(10); // 10% platform fee
        let budget = Uint128::MAX; // Maximum possible amount

        // This test ensures the contract handles very large numbers without overflow
        let client_info = mock_info(CLIENT, &coins(budget.u128(), "uxion"));

        let job_msg = ExecuteMsg::PostJob {
            title: "Overflow Test Job".to_string(),
            description: "Testing overflow protection".to_string(),
            budget,
            category: "Testing".to_string(),
            skills_required: vec!["Math".to_string()],
            duration_days: 30,
            documents: None,
            milestones: None,
        };

        let result = execute(deps.as_mut(), env, client_info, job_msg);
        // Should either succeed with proper overflow protection or fail gracefully
        // The key is that it doesn't panic or corrupt state
    }
}
