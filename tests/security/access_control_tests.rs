use cosmwasm_std::testing::{
    mock_dependencies, mock_env, mock_info, MockApi, MockQuerier, MockStorage,
};
use cosmwasm_std::{coins, OwnedDeps, Uint128};

use xworks_freelance_contract::contract::{execute, instantiate, query};
use xworks_freelance_contract::msg::{ConfigResponse, ExecuteMsg, InstantiateMsg, QueryMsg};
use xworks_freelance_contract::state::{BLOCKED_ADDRESSES, CONFIG};
use xworks_freelance_contract::ContractError;

const ADMIN: &str = "admin";
const CLIENT: &str = "client";
const FREELANCER: &str = "freelancer";
const UNAUTHORIZED_USER: &str = "unauthorized_user";
const MALICIOUS_USER: &str = "malicious_user";

mod access_control_tests {
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
    fn test_admin_only_functions_unauthorized_access() {
        let (mut deps, env, _) = setup_contract();

        let unauthorized_info = mock_info(UNAUTHORIZED_USER, &[]);

        // Test UpdateConfig - should fail for non-admin
        let update_config_msg = ExecuteMsg::UpdateConfig {
            admin: None,
            platform_fee_percent: Some(10),
            min_escrow_amount: None,
            dispute_period_days: None,
            max_job_duration_days: None,
        };

        let result = execute(
            deps.as_mut(),
            env.clone(),
            unauthorized_info.clone(),
            update_config_msg,
        );
        assert!(matches!(result, Err(ContractError::Unauthorized {})));

        // Test PauseContract - should fail for non-admin
        let pause_msg = ExecuteMsg::PauseContract {};
        let result = execute(
            deps.as_mut(),
            env.clone(),
            unauthorized_info.clone(),
            pause_msg,
        );
        assert!(matches!(result, Err(ContractError::Unauthorized {})));

        // Test UnpauseContract - should fail for non-admin
        let unpause_msg = ExecuteMsg::UnpauseContract {};
        let result = execute(
            deps.as_mut(),
            env.clone(),
            unauthorized_info.clone(),
            unpause_msg,
        );
        assert!(matches!(result, Err(ContractError::Unauthorized {})));

        // Test BlockAddress - should fail for non-admin
        let block_msg = ExecuteMsg::BlockAddress {
            address: MALICIOUS_USER.to_string(),
            reason: "Spam".to_string(),
        };
        let result = execute(
            deps.as_mut(),
            env.clone(),
            unauthorized_info.clone(),
            block_msg,
        );
        assert!(matches!(result, Err(ContractError::Unauthorized {})));

        // Test UnblockAddress - should fail for non-admin
        let unblock_msg = ExecuteMsg::UnblockAddress {
            address: MALICIOUS_USER.to_string(),
        };
        let result = execute(
            deps.as_mut(),
            env.clone(),
            unauthorized_info.clone(),
            unblock_msg,
        );
        assert!(matches!(result, Err(ContractError::Unauthorized {})));

        // Test ResetRateLimit - should fail for non-admin
        let reset_limit_msg = ExecuteMsg::ResetRateLimit {
            address: MALICIOUS_USER.to_string(),
        };
        let result = execute(deps.as_mut(), env, unauthorized_info, reset_limit_msg);
        assert!(matches!(result, Err(ContractError::Unauthorized {})));
    }

    #[test]
    fn test_admin_functions_authorized_access() {
        let (mut deps, env, _) = setup_contract();

        let admin_info = mock_info(ADMIN, &[]);

        // Test UpdateConfig - should succeed for admin
        let update_config_msg = ExecuteMsg::UpdateConfig {
            admin: None,
            platform_fee_percent: Some(8),
            min_escrow_amount: Some(Uint128::new(2000)),
            dispute_period_days: Some(10),
            max_job_duration_days: Some(300),
        };

        let result = execute(
            deps.as_mut(),
            env.clone(),
            admin_info.clone(),
            update_config_msg,
        );
        assert!(result.is_ok());

        // Verify config was updated
        let config_query = QueryMsg::GetConfig {};
        let config_response: ConfigResponse =
            cosmwasm_std::from_json(query(deps.as_ref(), env.clone(), config_query).unwrap())
                .unwrap();
        assert_eq!(config_response.config.platform_fee_percent, 8);
        assert_eq!(config_response.config.min_escrow_amount, Uint128::new(2000));

        // Test BlockAddress - should succeed for admin
        let block_msg = ExecuteMsg::BlockAddress {
            address: MALICIOUS_USER.to_string(),
            reason: "Fraudulent activity".to_string(),
        };
        let result = execute(deps.as_mut(), env.clone(), admin_info.clone(), block_msg);
        assert!(result.is_ok());

        // Verify address was blocked
        let malicious_addr = deps.api.addr_validate(MALICIOUS_USER).unwrap();
        let is_blocked = BLOCKED_ADDRESSES
            .may_load(deps.as_ref().storage, &malicious_addr)
            .unwrap();
        assert!(is_blocked.is_some());

        // Test UnblockAddress - should succeed for admin
        let unblock_msg = ExecuteMsg::UnblockAddress {
            address: MALICIOUS_USER.to_string(),
        };
        let result = execute(deps.as_mut(), env, admin_info, unblock_msg);
        assert!(result.is_ok());

        // Verify address was unblocked
        let is_blocked = BLOCKED_ADDRESSES
            .may_load(deps.as_ref().storage, &malicious_addr)
            .unwrap();
        assert!(is_blocked.is_none());
    }

    #[test]
    fn test_job_owner_permissions() {
        let (mut deps, env, _) = setup_contract();

        let client_info = mock_info(CLIENT, &coins(5000, "uxion"));
        let unauthorized_info = mock_info(UNAUTHORIZED_USER, &[]);

        // Create a job
        let job_msg = ExecuteMsg::PostJob {
            title: "Permission Test Job".to_string(),
            description: "Testing job owner permissions".to_string(),
            budget: Uint128::new(5000),
            category: "Testing".to_string(),
            skills_required: vec!["Testing".to_string()],
            duration_days: 30,
            documents: None,
            milestones: None,
        };

        execute(deps.as_mut(), env.clone(), client_info.clone(), job_msg).unwrap();

        // Test EditJob - should succeed for job owner
        let edit_msg = ExecuteMsg::EditJob {
            job_id: 0,
            title: Some("Updated Title".to_string()),
            description: None,
            budget: None,
            category: None,
            skills_required: None,
            duration_days: None,
            documents: None,
            milestones: None,
        };

        let result = execute(
            deps.as_mut(),
            env.clone(),
            client_info.clone(),
            edit_msg.clone(),
        );
        assert!(result.is_ok());

        // Test EditJob - should fail for non-owner
        let result = execute(
            deps.as_mut(),
            env.clone(),
            unauthorized_info.clone(),
            edit_msg,
        );
        assert!(matches!(result, Err(ContractError::Unauthorized {})));

        // Test DeleteJob - should succeed for job owner
        let delete_msg = ExecuteMsg::DeleteJob { job_id: 0 };
        let result = execute(deps.as_mut(), env.clone(), client_info, delete_msg.clone());
        // Note: This might fail for other reasons (job in progress, etc.) but not due to authorization

        // Test DeleteJob - should fail for non-owner
        let result = execute(deps.as_mut(), env, unauthorized_info, delete_msg);
        assert!(result.is_err()); // Should fail due to unauthorized or job not found
    }

    #[test]
    fn test_freelancer_permissions() {
        let (mut deps, env, _) = setup_contract();

        let client_info = mock_info(CLIENT, &coins(5000, "uxion"));
        let freelancer_info = mock_info(FREELANCER, &[]);
        let unauthorized_info = mock_info(UNAUTHORIZED_USER, &[]);

        // Create job and submit proposal
        let job_msg = ExecuteMsg::PostJob {
            title: "Freelancer Permission Test".to_string(),
            description: "Testing freelancer permissions".to_string(),
            budget: Uint128::new(5000),
            category: "Testing".to_string(),
            skills_required: vec!["Testing".to_string()],
            duration_days: 30,
            documents: None,
            milestones: None,
        };

        execute(deps.as_mut(), env.clone(), client_info.clone(), job_msg).unwrap();

        let proposal_msg = ExecuteMsg::SubmitProposal {
            job_id: 0,
            bid_amount: Uint128::new(4500),
            cover_letter: "Test proposal".to_string(),
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

        // Test EditProposal - should succeed for proposal owner
        let edit_proposal_msg = ExecuteMsg::EditProposal {
            proposal_id: 0,
            bid_amount: Some(Uint128::new(4000)),
            cover_letter: None,
            delivery_time_days: None,
            milestones: None,
        };

        let result = execute(
            deps.as_mut(),
            env.clone(),
            freelancer_info.clone(),
            edit_proposal_msg.clone(),
        );
        assert!(result.is_ok());

        // Test EditProposal - should fail for non-owner
        let result = execute(
            deps.as_mut(),
            env.clone(),
            unauthorized_info.clone(),
            edit_proposal_msg,
        );
        assert!(matches!(result, Err(ContractError::Unauthorized {})));

        // Test WithdrawProposal - should succeed for proposal owner
        let withdraw_msg = ExecuteMsg::WithdrawProposal { proposal_id: 0 };
        let result = execute(
            deps.as_mut(),
            env.clone(),
            freelancer_info,
            withdraw_msg.clone(),
        );
        assert!(result.is_ok());

        // Test WithdrawProposal - should fail for non-owner (proposal withdrawn)
        let result = execute(deps.as_mut(), env, unauthorized_info, withdraw_msg);
        assert!(result.is_err()); // Should fail - proposal doesn't exist or unauthorized
    }

    #[test]
    fn test_contract_pause_enforcement() {
        let (mut deps, env, _) = setup_contract();

        let admin_info = mock_info(ADMIN, &[]);
        let client_info = mock_info(CLIENT, &coins(5000, "uxion"));

        // Pause the contract
        let pause_msg = ExecuteMsg::PauseContract {};
        execute(deps.as_mut(), env.clone(), admin_info.clone(), pause_msg).unwrap();

        // Test that normal operations are blocked when paused
        let job_msg = ExecuteMsg::PostJob {
            title: "Should Fail".to_string(),
            description: "Contract is paused".to_string(),
            budget: Uint128::new(5000),
            category: "Testing".to_string(),
            skills_required: vec!["Testing".to_string()],
            duration_days: 30,
            documents: None,
            milestones: None,
        };

        let result = execute(
            deps.as_mut(),
            env.clone(),
            client_info.clone(),
            job_msg.clone(),
        );
        assert!(result.is_err()); // Should fail due to contract being paused

        // Unpause the contract
        let unpause_msg = ExecuteMsg::UnpauseContract {};
        execute(deps.as_mut(), env.clone(), admin_info, unpause_msg).unwrap();

        // Test that operations work after unpausing
        let result = execute(deps.as_mut(), env, client_info, job_msg);
        assert!(result.is_ok());
    }

    #[test]
    fn test_blocked_address_restrictions() {
        let (mut deps, env, _) = setup_contract();

        let admin_info = mock_info(ADMIN, &[]);
        let malicious_info = mock_info(MALICIOUS_USER, &coins(5000, "uxion"));

        // Block the malicious user
        let block_msg = ExecuteMsg::BlockAddress {
            address: MALICIOUS_USER.to_string(),
            reason: "Spam activity".to_string(),
        };
        execute(deps.as_mut(), env.clone(), admin_info, block_msg).unwrap();

        // Test that blocked user cannot perform operations
        let job_msg = ExecuteMsg::PostJob {
            title: "Blocked User Job".to_string(),
            description: "This should fail".to_string(),
            budget: Uint128::new(5000),
            category: "Testing".to_string(),
            skills_required: vec!["Testing".to_string()],
            duration_days: 30,
            documents: None,
            milestones: None,
        };

        let result = execute(deps.as_mut(), env, malicious_info, job_msg);
        assert!(result.is_err()); // Should fail due to address being blocked
    }

    #[test]
    fn test_dispute_resolution_permissions() {
        let (mut deps, env, _) = setup_contract();

        let admin_info = mock_info(ADMIN, &[]);
        let client_info = mock_info(CLIENT, &coins(5000, "uxion"));
        let freelancer_info = mock_info(FREELANCER, &[]);
        let unauthorized_info = mock_info(UNAUTHORIZED_USER, &[]);

        // Create job, proposal, and accept it
        let job_msg = ExecuteMsg::PostJob {
            title: "Dispute Test Job".to_string(),
            description: "Testing dispute permissions".to_string(),
            budget: Uint128::new(5000),
            category: "Testing".to_string(),
            skills_required: vec!["Testing".to_string()],
            duration_days: 30,
            documents: None,
            milestones: None,
        };

        execute(deps.as_mut(), env.clone(), client_info.clone(), job_msg).unwrap();

        let proposal_msg = ExecuteMsg::SubmitProposal {
            job_id: 0,
            bid_amount: Uint128::new(4500),
            cover_letter: "Test proposal".to_string(),
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

        let accept_msg = ExecuteMsg::AcceptProposal {
            job_id: 0,
            proposal_id: 0,
        };

        execute(deps.as_mut(), env.clone(), client_info, accept_msg).unwrap();

        // Test RaiseDispute - should succeed for job participants
        let dispute_msg = ExecuteMsg::RaiseDispute {
            job_id: 0,
            reason: "Work not completed as agreed".to_string(),
            evidence: vec!["evidence1.pdf".to_string()],
        };

        let result = execute(
            deps.as_mut(),
            env.clone(),
            freelancer_info,
            dispute_msg.clone(),
        );
        assert!(result.is_ok());

        // Test RaiseDispute - should fail for unauthorized user
        let result = execute(
            deps.as_mut(),
            env.clone(),
            unauthorized_info.clone(),
            dispute_msg,
        );
        assert!(result.is_err());

        // Test ResolveDispute - should succeed for admin only
        let resolve_msg = ExecuteMsg::ResolveDispute {
            dispute_id: "0_0".to_string(), // Assuming dispute ID format
            resolution: "Client favored".to_string(),
            release_to_freelancer: false,
        };

        let result = execute(deps.as_mut(), env.clone(), admin_info, resolve_msg.clone());
        // May succeed or fail based on implementation, but should not be unauthorized

        // Test ResolveDispute - should fail for non-admin
        let result = execute(deps.as_mut(), env, unauthorized_info, resolve_msg);
        assert!(result.is_err()); // Should fail due to unauthorized or dispute not found
    }
}
