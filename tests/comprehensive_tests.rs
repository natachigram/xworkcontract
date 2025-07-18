use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
use cosmwasm_std::{coins, from_json, Addr, Uint128};
use xworks_freelance_contract::contract::{execute, instantiate, query};
use xworks_freelance_contract::msg::{
    BountyResponse, ConfigResponse, DisputesResponse, EscrowResponse, ExecuteMsg, InstantiateMsg,
    JobResponse, MilestoneInput, ProposalResponse, QueryMsg, RewardTierInput,
};
use xworks_freelance_contract::state::{
    BountyStatus, ContactPreference, JobStatus, ProposalMilestone, Rating,
};

#[test]
fn full_contract_flow() {
    let mut deps = mock_dependencies();
    let env = mock_env();
    let info = mock_info("admin", &coins(1000, "uxion"));
    // Instantiate contract
    let init = InstantiateMsg {
        admin: Some("admin".to_string()),
        platform_fee_percent: Some(5),
        min_escrow_amount: Some(Uint128::new(100)),
        dispute_period_days: Some(3),
        max_job_duration_days: Some(30),
    };
    instantiate(deps.as_mut(), env.clone(), info.clone(), init).unwrap();
    // Query and verify config
    let cfg_resp: ConfigResponse =
        from_json(&query(deps.as_ref(), env.clone(), QueryMsg::GetConfig {}).unwrap()).unwrap();
    let cfg = cfg_resp.config;
    assert_eq!(cfg.admin, Addr::unchecked("admin"));
    assert_eq!(cfg.platform_fee_percent, 5);

    // Post a new job (ID = 0)
    let post = ExecuteMsg::PostJob {
        title: "Title".to_string(),
        description: "Desc".to_string(),
        company: None,
        location: None,
        category: "cat".to_string(),
        skills_required: vec!["rust".to_string()],
        documents: None,
        milestones: Some(vec![MilestoneInput {
            title: "ms1".to_string(),
            description: "d1".to_string(),
            amount: Uint128::new(500),
            deadline_days: 5,
        }]),
        budget: Uint128::new(1000),
        duration_days: 10,
        experience_level: 2,
        is_remote: true,
        urgency_level: 1,
        off_chain_storage_key: "key1".to_string(),
    };
    execute(deps.as_mut(), env.clone(), info.clone(), post).unwrap();
    // Verify job stored
    let j_resp: JobResponse =
        from_json(&query(deps.as_ref(), env.clone(), QueryMsg::GetJob { job_id: 0 }).unwrap())
            .unwrap();
    let j = j_resp.job;
    assert_eq!(j.status, JobStatus::Open);
    assert_eq!(j.budget.u128(), 1000);

    // Submit a proposal
    let prop = ExecuteMsg::SubmitProposal {
        job_id: 0,
        cover_letter: "cover".to_string(),
        milestones: Some(vec![ProposalMilestone {
            title: "pm1".to_string(),
            description: "pd1".to_string(),
            amount: Uint128::new(500),
            deadline_days: 5,
        }]),
        portfolio_samples: None,
        delivery_time_days: 7,
        contact_preference: ContactPreference::Email,
        agreed_to_terms: true,
        agreed_to_escrow: true,
        estimated_hours: Some(40),
        off_chain_storage_key: "key2".to_string(),
    };
    execute(deps.as_mut(), env.clone(), info.clone(), prop).unwrap();
    let p_resp: ProposalResponse = from_json(
        &query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::GetProposal { proposal_id: 0 },
        )
        .unwrap(),
    )
    .unwrap();
    let p = p_resp.proposal;
    assert_eq!(p.job_id, 0);

    // Accept proposal
    let acc = ExecuteMsg::AcceptProposal {
        job_id: 0,
        proposal_id: 0,
    };
    execute(deps.as_mut(), env.clone(), info.clone(), acc).unwrap();
    let j2_resp: JobResponse =
        from_json(&query(deps.as_ref(), env.clone(), QueryMsg::GetJob { job_id: 0 }).unwrap())
            .unwrap();
    let j2 = j2_resp.job;
    assert_eq!(j2.status, JobStatus::InProgress);

    // Query existing escrow created during posting
    let es_resp: EscrowResponse = from_json(
        &query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::GetJobEscrow { job_id: 0 },
        )
        .unwrap(),
    )
    .unwrap();
    let es = es_resp.escrow;
    // Escrow auto-funded with full budget
    assert_eq!(es.amount.u128(), 1000);
    let _escrow_id = es.id.clone();

    // Complete the job which triggers escrow release on-chain
    let cj = ExecuteMsg::CompleteJob { job_id: 0 };
    execute(deps.as_mut(), env.clone(), info.clone(), cj).unwrap();
    // Verify job status updated to Completed
    let j3_resp: JobResponse =
        from_json(&query(deps.as_ref(), env.clone(), QueryMsg::GetJob { job_id: 0 }).unwrap())
            .unwrap();
    assert_eq!(j3_resp.job.status, JobStatus::Completed);
    // Verify escrow released flag
    let es2_resp: EscrowResponse = from_json(
        &query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::GetJobEscrow { job_id: 0 },
        )
        .unwrap(),
    )
    .unwrap();
    assert!(es2_resp.escrow.released);

    // Submit rating
    let rt = ExecuteMsg::SubmitRating {
        job_id: 0,
        rating: 5,
        comment: "good".to_string(),
    };
    execute(deps.as_mut(), env.clone(), info.clone(), rt).unwrap();
    // Retrieve single rating
    let r: Rating = from_json(
        &query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::GetJobRating {
                job_id: 0,
                rater: "admin".to_string(),
            },
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(r.rating, 5);

    // Raise dispute
    let rd = ExecuteMsg::RaiseDispute {
        job_id: 0,
        reason: "issue".to_string(),
        evidence: vec![],
    };
    execute(deps.as_mut(), env.clone(), info.clone(), rd).unwrap();
    // Fetch disputes for job
    let dr: DisputesResponse = from_json(
        &query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::GetJobDisputes { job_id: 0 },
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(dr.disputes.len(), 1);
    let dispute = &dr.disputes[0];
    assert_eq!(dispute.job_id, 0);
    // Resolve dispute
    let resd = ExecuteMsg::ResolveDispute {
        dispute_id: dispute.id.clone(),
        resolution: "ok".to_string(),
        release_to_freelancer: true,
    };
    execute(deps.as_mut(), env.clone(), info.clone(), resd).unwrap();

    // Admin pause and unpause
    execute(
        deps.as_mut(),
        env.clone(),
        info.clone(),
        ExecuteMsg::PauseContract {},
    )
    .unwrap();
    let pause_resp: ConfigResponse =
        from_json(&query(deps.as_ref(), env.clone(), QueryMsg::GetConfig {}).unwrap()).unwrap();
    let pause_q = pause_resp.config;
    assert!(pause_q.paused);
    execute(
        deps.as_mut(),
        env.clone(),
        info.clone(),
        ExecuteMsg::UnpauseContract {},
    )
    .unwrap();
    let unpause_resp: ConfigResponse =
        from_json(&query(deps.as_ref(), env.clone(), QueryMsg::GetConfig {}).unwrap()).unwrap();
    let unpause_q = unpause_resp.config;
    assert!(!unpause_q.paused);

    // Update user profile
    let up = ExecuteMsg::UpdateUserProfile {
        display_name: Some("Alice".to_string()),
        bio: None,
        skills: Some(vec!["rust".to_string()]),
        location: None,
        website: None,
        portfolio_links: None,
        hourly_rate: Some(Uint128::new(50)),
        availability: None,
        off_chain_storage_key: "key3".to_string(),
    };
    execute(deps.as_mut(), env.clone(), mock_info("alice", &[]), up).unwrap();
    // User profile querying not available; skip direct profile check

    // Create a bounty
    let bounty_funds = coins(2000, "token"); // funds attach for bounty
    let cb = ExecuteMsg::CreateBounty {
        title: "b1".to_string(),
        description: "bd".to_string(),
        requirements: vec!["req".to_string()],
        total_reward: Uint128::new(2000),
        category: "cat".to_string(),
        skills_required: vec!["rust".to_string()],
        submission_deadline_days: 7,
        review_period_days: 3,
        max_winners: 1,
        reward_distribution: vec![RewardTierInput {
            position: 1,
            percentage: 100,
        }],
        documents: None,
    };
    execute(
        deps.as_mut(),
        env.clone(),
        mock_info("admin", &bounty_funds),
        cb,
    )
    .unwrap();
    let b_resp: BountyResponse = from_json(
        &query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::GetBounty { bounty_id: 0 },
        )
        .unwrap(),
    )
    .unwrap();
    let b = b_resp.bounty;
    assert_eq!(b.status, BountyStatus::Open);
}
