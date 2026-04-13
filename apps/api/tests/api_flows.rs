mod common;

use common::TestApp;
use domain::auth::UserRole;
use http::StatusCode;
use serde_json::json;

#[tokio::test]
async fn register_and_login_flow_returns_versioned_envelope() {
    let Some(app) = TestApp::new().await else {
        eprintln!("skipping api flow test: TEST_DATABASE_URL or DATABASE_URL not set");
        return;
    };

    let (register_status, register_body) = app
        .json_request(
            "POST",
            "/api/v1/auth/register",
            None,
            json!({
                "email": "parent@example.com",
                "password": "correct-horse-battery-staple",
                "role": "parent"
            }),
        )
        .await;

    assert_eq!(register_status, StatusCode::OK);
    assert_eq!(register_body["data"]["user"]["email"], "parent@example.com");
    assert_eq!(register_body["meta"]["docs"]["version"], "v1");

    let (login_status, login_body) = app
        .json_request(
            "POST",
            "/api/v1/auth/login",
            None,
            json!({
                "email": "parent@example.com",
                "password": "correct-horse-battery-staple"
            }),
        )
        .await;

    assert_eq!(login_status, StatusCode::OK);
    assert!(login_body["data"]["tokens"]["access_token"].as_str().is_some());
}

#[tokio::test]
async fn contribution_to_payout_approval_flow_works_end_to_end() {
    let Some(app) = TestApp::new().await else {
        eprintln!("skipping api flow test: TEST_DATABASE_URL or DATABASE_URL not set");
        return;
    };
    let parent = app.seed_user("owner@example.com", UserRole::Parent).await;
    let admin = app.seed_user("admin@example.com", UserRole::PlatformAdmin).await;
    let parent_token = app.access_token(&parent);
    let admin_token = app.access_token(&admin);
    let (_, _, vault_id) = app.seed_vault_bundle(parent.id).await;
    let milestone_id = app.seed_milestone(vault_id).await;
    let school_id = app.seed_verified_school(admin.id).await;

    let (contribution_status, contribution_body) = app
        .json_request(
            "POST",
            &format!("/api/v1/vaults/{vault_id}/contributions"),
            Some(&parent_token),
            json!({
                "amount_minor": 10000,
                "currency": "USD",
                "source_type": "manual",
                "external_reference": null,
                "idempotency_key": "contrib-1"
            }),
        )
        .await;
    assert_eq!(contribution_status, StatusCode::OK);
    let contribution_id =
        contribution_body["data"]["contribution"]["id"].as_str().unwrap().to_owned();

    let (confirm_status, _) = app
        .json_request(
            "POST",
            &format!("/api/v1/contributions/{contribution_id}/confirm"),
            Some(&admin_token),
            json!({ "external_reference": "manual-confirm-1" }),
        )
        .await;
    assert_eq!(confirm_status, StatusCode::OK);

    let (request_status, request_body) = app
        .json_request(
            "POST",
            &format!("/api/v1/vaults/{vault_id}/payout-requests"),
            Some(&parent_token),
            json!({
                "milestone_id": milestone_id,
                "school_id": school_id,
                "amount_minor": 5000,
                "currency": "USD",
                "idempotency_key": "payout-1"
            }),
        )
        .await;
    assert_eq!(request_status, StatusCode::OK);
    let payout_id = request_body["data"]["payout"]["id"].as_str().unwrap().to_owned();

    let (review_status, _) = app
        .json_request(
            "POST",
            &format!("/api/v1/payouts/{payout_id}/under-review"),
            Some(&admin_token),
            json!({ "review_notes": "checking docs", "external_payout_reference": null }),
        )
        .await;
    assert_eq!(review_status, StatusCode::OK);

    let (approve_status, approve_body) = app
        .json_request(
            "POST",
            &format!("/api/v1/payouts/{payout_id}/approve"),
            Some(&admin_token),
            json!({ "review_notes": "approved", "external_payout_reference": "payout-001" }),
        )
        .await;
    assert_eq!(approve_status, StatusCode::OK);
    assert_eq!(approve_body["data"]["payout"]["status"], "approved");
}

#[tokio::test]
async fn scholarship_application_and_decision_flow_works_end_to_end() {
    let Some(app) = TestApp::new().await else {
        eprintln!("skipping api flow test: TEST_DATABASE_URL or DATABASE_URL not set");
        return;
    };
    let donor = app.seed_user("donor@example.com", UserRole::Donor).await;
    let parent = app.seed_user("parent2@example.com", UserRole::Parent).await;
    let admin = app.seed_user("admin2@example.com", UserRole::PlatformAdmin).await;
    let donor_token = app.access_token(&donor);
    let parent_token = app.access_token(&parent);
    let admin_token = app.access_token(&admin);
    let (child_id, _, _) = app.seed_vault_bundle(parent.id).await;

    let (pool_status, pool_body) = app
        .json_request(
            "POST",
            "/api/v1/scholarship-pools",
            Some(&donor_token),
            json!({
                "name": "Girls in STEM",
                "description": "STEM support",
                "currency": "USD",
                "geography_restriction": null,
                "education_level_restriction": null,
                "school_id_restriction": null,
                "category_restriction": null
            }),
        )
        .await;
    assert_eq!(pool_status, StatusCode::OK);
    let pool_id = pool_body["data"]["pool"]["id"].as_str().unwrap().to_owned();

    let (fund_status, _) = app
        .json_request(
            "POST",
            &format!("/api/v1/scholarship-pools/{pool_id}/fund"),
            Some(&donor_token),
            json!({
                "amount_minor": 15000,
                "currency": "USD",
                "external_reference": null,
                "idempotency_key": "donation-1"
            }),
        )
        .await;
    assert_eq!(fund_status, StatusCode::OK);

    let (application_status, application_body) = app
        .json_request(
            "POST",
            &format!("/api/v1/scholarship-pools/{pool_id}/applications"),
            Some(&parent_token),
            json!({
                "child_profile_id": child_id,
                "student_country": "NG",
                "education_level": "secondary",
                "school_id": null,
                "category": "stem",
                "notes": "Needs support"
            }),
        )
        .await;
    assert_eq!(application_status, StatusCode::OK);
    let application_id = application_body["data"]["application"]["id"].as_str().unwrap().to_owned();

    let (approve_status, approve_body) = app
        .json_request(
            "POST",
            &format!("/api/v1/scholarship-applications/{application_id}/approve"),
            Some(&admin_token),
            json!({
                "amount_minor": 5000,
                "currency": "USD",
                "decision_notes": "Approved for 2026 cycle"
            }),
        )
        .await;
    assert_eq!(approve_status, StatusCode::OK);
    assert_eq!(approve_body["data"]["award"]["status"], "approved");
}

#[tokio::test]
async fn admin_can_query_audit_logs_for_sensitive_flow() {
    let Some(app) = TestApp::new().await else {
        eprintln!("skipping api flow test: TEST_DATABASE_URL or DATABASE_URL not set");
        return;
    };
    let parent = app.seed_user("audit-parent@example.com", UserRole::Parent).await;
    let admin = app.seed_user("audit-admin@example.com", UserRole::PlatformAdmin).await;
    let parent_token = app.access_token(&parent);
    let admin_token = app.access_token(&admin);
    let (_, _, vault_id) = app.seed_vault_bundle(parent.id).await;

    let (status, headers, body) = app
        .json_request_with_headers(
            "POST",
            &format!("/api/v1/vaults/{vault_id}/contributions"),
            Some(&parent_token),
            json!({
                "amount_minor": 10000,
                "currency": "USD",
                "source_type": "manual",
                "external_reference": null,
                "idempotency_key": "audit-contrib-1"
            }),
        )
        .await;

    assert_eq!(status, StatusCode::OK);
    let correlation_id = headers
        .get("x-correlation-id")
        .and_then(|value| value.to_str().ok())
        .unwrap()
        .to_owned();

    let (audit_status, _, audit_body) = app
        .get_json(
            &format!(
                "/api/v1/admin/audit-logs?correlation_id={correlation_id}&action=contribution.recorded"
            ),
            Some(&admin_token),
        )
        .await;

    assert_eq!(audit_status, StatusCode::OK);
    assert_eq!(audit_body["data"]["items"][0]["action"], "contribution.recorded");
    assert_eq!(
        audit_body["data"]["items"][0]["correlation_id"],
        correlation_id
    );
    assert_eq!(body["data"]["contribution"]["status"], "pending");
}

#[tokio::test]
async fn achievement_credentials_can_be_issued_and_viewed_by_guardian_and_student() {
    let Some(app) = TestApp::new().await else {
        eprintln!("skipping api flow test: TEST_DATABASE_URL or DATABASE_URL not set");
        return;
    };
    let admin = app.seed_user("credential-admin@example.com", UserRole::PlatformAdmin).await;
    let parent = app.seed_user("credential-parent@example.com", UserRole::Parent).await;
    let student = app.seed_user("credential-student@example.com", UserRole::Student).await;
    let admin_token = app.access_token(&admin);
    let parent_token = app.access_token(&parent);
    let student_token = app.access_token(&student);
    let (child_id, _, _) = app.seed_vault_bundle(parent.id).await;

    let (issue_status, issue_body) = app
        .json_request(
            "POST",
            "/api/v1/credentials",
            Some(&admin_token),
            json!({
                "child_profile_id": child_id,
                "recipient_user_id": student.id,
                "school_id": null,
                "achievement_type": "fee_fully_funded",
                "title": "2026 Term fully funded",
                "description": "All required fees were covered in full",
                "achievement_date": "2026-04-13",
                "issuance_notes": null,
                "evidence_uri": null,
                "attestation_anchor": null,
                "attestation_anchor_network": null,
                "metadata": { "term": "2026-T1" }
            }),
        )
        .await;
    assert_eq!(issue_status, StatusCode::OK);
    let credential_id = issue_body["data"]["credential"]["id"].as_str().unwrap().to_owned();
    assert_eq!(
        issue_body["data"]["credential"]["achievement_type"],
        "fee_fully_funded"
    );

    let (parent_list_status, _, parent_list_body) =
        app.get_json("/api/v1/credentials", Some(&parent_token)).await;
    assert_eq!(parent_list_status, StatusCode::OK);
    assert_eq!(parent_list_body["data"]["items"][0]["id"], credential_id);

    let (student_get_status, _, student_get_body) = app
        .get_json(&format!("/api/v1/credentials/{credential_id}"), Some(&student_token))
        .await;
    assert_eq!(student_get_status, StatusCode::OK);
    assert_eq!(
        student_get_body["data"]["credential"]["credential_ref"]
            .as_str()
            .unwrap()
            .len(),
        36
    );
}
