use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
use cosmwasm_std::{coins, Uint128};
use xworks_freelance_contract::contract::{execute, instantiate};
use xworks_freelance_contract::msg::{ExecuteMsg, InstantiateMsg};
use xworks_freelance_contract::state::ESCROWS;

const ADMIN: &str = "admin";
const CLIENT: &str = "client";
const FREELANCER: &str = "freelancer";

mod basic_financial_tests {
    use super::*;

    #[test]
    fn test_platform_fee_calculation() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        let info = mock_info(ADMIN, &[]);
        
        let msg = InstantiateMsg {
            admin: Some(ADMIN.to_string()),
            platform_fee_percent: Some(5), // 5% platform fee
            min_escrow_amount: Some(Uint128::new(1000)),
            dispute_period_days: Some(7),
            max_job_duration_days: Some(365),
        };
        instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();

        let client_info = mock_info(CLIENT, &coins(10000, "uxion"));
        let freelancer_info = mock_info(FREELANCER, &[]);

        // Create job
        let job_msg = ExecuteMsg::PostJob {
            title: "Fee Test Job".to_string(),
            description: "Testing platform fee calculation".to_string(),
            budget: Uint128::new(10000),
            category: "Testing".to_string(),
            skills_required: vec!["Math".to_string()],
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

        let accept_msg = ExecuteMsg::AcceptProposal {
            job_id: 0,
            proposal_id: 0,
        };
        execute(deps.as_mut(), env.clone(), client_info.clone(), accept_msg).unwrap();

        // Create escrow
        let create_escrow_msg = ExecuteMsg::CreateEscrowNative {
            job_id: 0,
            amount: Uint128::new(10000),
        };
        let result = execute(deps.as_mut(), env, client_info, create_escrow_msg);
        assert!(result.is_ok());

        // Verify fee calculation
        if let Ok(escrow) = ESCROWS.load(deps.as_ref().storage, "0") {
            // Expected platform fee: 10000 * 5% = 500
            let expected_platform_fee = Uint128::new(500);
            assert_eq!(escrow.platform_fee, expected_platform_fee);
            // Expected freelancer amount: 10000 - 500 = 9500
            let expected_freelancer_amount = Uint128::new(9500);
            assert_eq!(escrow.amount, expected_freelancer_amount);
        }
    }

    #[test]
    fn test_maximum_platform_fee() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        let info = mock_info(ADMIN, &[]);

        // Test that platform fee cannot exceed 10%
        let msg = InstantiateMsg {
            admin: Some(ADMIN.to_string()),
            platform_fee_percent: Some(15), // Above maximum
            min_escrow_amount: Some(Uint128::new(1000)),
            dispute_period_days: Some(7),
            max_job_duration_days: Some(365),
        };
        let result = instantiate(deps.as_mut(), env, info, msg);
        assert!(result.is_err());
    }

    #[test]
    fn test_zero_platform_fee() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        let info = mock_info(ADMIN, &[]);

        let msg = InstantiateMsg {
            admin: Some(ADMIN.to_string()),
            platform_fee_percent: Some(0), // No platform fee
            min_escrow_amount: Some(Uint128::new(1000)),
            dispute_period_days: Some(7),
            max_job_duration_days: Some(365),
        };
        let result = instantiate(deps.as_mut(), env, info, msg);
        assert!(result.is_ok());
    }

    #[test]
    fn test_minimum_escrow_amount_validation() {
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

        let client_info = mock_info(CLIENT, &coins(500, "uxion"));

        // Try to create job with budget below minimum
        let job_msg = ExecuteMsg::PostJob {
            title: "Small Job".to_string(),
            description: "Job below minimum escrow".to_string(),
            budget: Uint128::new(500), // Below 1000 minimum
            category: "Small".to_string(),
            skills_required: vec!["Testing".to_string()],
            duration_days: 30,
            documents: None,
            milestones: None,
        };
        let result = execute(deps.as_mut(), env, client_info, job_msg);
        assert!(result.is_err());
    }

    #[test]
    fn test_fee_precision_with_small_amounts() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        let info = mock_info(ADMIN, &[]);

        let msg = InstantiateMsg {
            admin: Some(ADMIN.to_string()),
            platform_fee_percent: Some(3), // 3% platform fee
            min_escrow_amount: Some(Uint128::new(1000)),
            dispute_period_days: Some(7),
            max_job_duration_days: Some(365),
        };
        instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();

        let client_info = mock_info(CLIENT, &coins(1001, "uxion"));
        let freelancer_info = mock_info(FREELANCER, &[]);

        // Create job with amount that could cause precision issues
        let job_msg = ExecuteMsg::PostJob {
            title: "Precision Test".to_string(),
            description: "Testing fee precision".to_string(),
            budget: Uint128::new(1001),
            category: "Testing".to_string(),
            skills_required: vec!["Math".to_string()],
            duration_days: 30,
            documents: None,
            milestones: None,
        };
        execute(deps.as_mut(), env.clone(), client_info.clone(), job_msg).unwrap();

        // Submit and accept proposal
        let proposal_msg = ExecuteMsg::SubmitProposal {
            job_id: 0,
            cover_letter: "Precision test proposal".to_string(),
            delivery_time_days: 25,
            milestones: None,
        };
        execute(deps.as_mut(), env.clone(), freelancer_info, proposal_msg).unwrap();

        let accept_msg = ExecuteMsg::AcceptProposal {
            job_id: 0,
            proposal_id: 0,
        };
        execute(deps.as_mut(), env.clone(), client_info.clone(), accept_msg).unwrap();

        // Create escrow
        let create_escrow_info = mock_info(CLIENT, &coins(1001, "uxion"));
        let create_escrow_msg = ExecuteMsg::CreateEscrowNative {
            job_id: 0,
            amount: Uint128::new(1001),
        };
        let result = execute(deps.as_mut(), env, create_escrow_info, create_escrow_msg);
        assert!(result.is_ok());

        // Verify precision handling
        if let Ok(escrow) = ESCROWS.load(deps.as_ref().storage, "0") {
            // 3% of 1001 = 30.03, should round down to 30
            let expected_platform_fee = Uint128::new(30);
            assert_eq!(escrow.platform_fee, expected_platform_fee);
            
            // Verify total equals original amount
            let total = escrow.platform_fee + escrow.amount;
            assert_eq!(total, Uint128::new(1001));
        }
    }
}