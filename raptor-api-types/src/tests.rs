use crate::*;
use serde_json::json;

// Round-trip through serde and compare Values: proves both key names
// (Deserialize) and output shape (Serialize) match the server's JSON.
fn round_trip<T: serde::Serialize + serde::de::DeserializeOwned>(v: serde_json::Value) {
    let t: T = serde_json::from_value(v.clone()).unwrap();
    assert_eq!(serde_json::to_value(&t).unwrap(), v);
}

#[test]
fn target_shape() {
    round_trip::<TargetRest>(json!({
        "controllerId": "d1", "name": "device one", "description": null,
        "updateStatus": "in_sync", "securityToken": "abc123",
        "createdAt": 1, "lastModifiedAt": 2, "address": null, "ipAddress": null,
        "lastControllerRequestAt": 3,
        "pollStatus": {"lastRequestAt": 3, "nextExpectedRequestAt": 4, "overdue": false},
        "requestAttributes": false,
        "_links": {"self": {"href": "http://x/rest/v1/targets/d1"}}
    }));
}

#[test]
fn target_update_carries_request_attributes() {
    round_trip::<TargetUpdate>(json!({"requestAttributes": true}));
    // omitted stays omitted, so a PUT that doesn't mention it leaves it alone
    let u: TargetUpdate = serde_json::from_value(json!({"name": "d1"})).unwrap();
    assert_eq!(u.request_attributes, None);
}

#[test]
fn software_module_shape() {
    round_trip::<SmRest>(json!({
        "id": 1, "name": "fw", "version": "1.0", "type": "os",
        "vendor": null, "description": null, "createdAt": 1, "lastModifiedAt": 2,
        "_links": {"self": {"href": "http://x/rest/v1/softwaremodules/1"}}
    }));
}

#[test]
fn distribution_set_shape() {
    round_trip::<DsRest>(json!({
        "id": 1, "name": "stable", "version": "1.0", "type": "os",
        "description": null, "requiredMigrationStep": false, "complete": true,
        "deleted": false, "valid": true, "createdAt": 1, "lastModifiedAt": 2,
        "modules": [{
            "id": 1, "name": "fw", "version": "1.0", "type": "os",
            "vendor": null, "description": null, "createdAt": 1, "lastModifiedAt": 2,
            "_links": {"self": {"href": "http://x/rest/v1/softwaremodules/1"}}
        }],
        "_links": {"self": {"href": "http://x/rest/v1/distributionsets/1"}}
    }));
}

#[test]
fn artifact_shape() {
    round_trip::<ArtifactRest>(json!({
        "id": 1, "providedFilename": "fw.bin", "size": 11,
        "hashes": {"sha1": "a", "md5": "b", "sha256": "c"},
        "_links": {"self": {"href": "http://x"}, "download": {"href": "http://x/download"}}
    }));
}

#[test]
fn action_shape() {
    round_trip::<ActionRest>(json!({
        "id": 1, "type": "update", "status": "pending", "detailStatus": "running",
        "forceType": "forced", "createdAt": 1, "lastModifiedAt": 2,
        "target": "d1",
        "_links": {"self": {"href": "http://x/rest/v1/actions/1"}}
    }));
}

#[test]
fn action_target_field_omitted_when_none() {
    let a = ActionRest {
        id: 1,
        action_type: "update".into(),
        status: "pending".into(),
        detail_status: "running".into(),
        force_type: "forced".into(),
        created_at: 1,
        last_modified_at: 2,
        target: None,
        links: serde_json::Value::Null,
    };
    let v = serde_json::to_value(&a).unwrap();
    assert!(v.get("target").is_none());
}

#[test]
fn action_status_shape() {
    round_trip::<ActionStatusRest>(json!({
        "id": 7, "type": "canceled",
        "messages": ["force canceled by operator"],
        "reportedAt": 1699999999000i64
    }));
}

#[test]
fn rollout_shape() {
    round_trip::<RolloutRest>(json!({
        "id": 1, "name": "r1", "description": null, "distributionSetId": 5,
        "targetFilterQuery": "name==*", "status": "running", "totalTargets": 10,
        "totalTargetsPerStatus": {
            "notstarted": 0, "scheduled": 5, "running": 3,
            "error": 1, "finished": 1, "cancelled": 0
        },
        "createdAt": 1, "lastModifiedAt": 2,
        "_links": {"self": {"href": "http://x/rest/v1/rollouts/1"}}
    }));
}

#[test]
fn rollout_group_shape() {
    round_trip::<RolloutGroupRest>(json!({
        "id": 6, "name": "group-1", "status": "running", "totalTargets": 5,
        "totalTargetsPerStatus": {
            "notstarted": 0, "scheduled": 0, "running": 3,
            "error": 1, "finished": 1, "cancelled": 0
        },
        "successCondition": {"condition": "THRESHOLD", "expression": "50"},
        "errorCondition": {"condition": "THRESHOLD", "expression": "50"},
        "_links": {"self": {"href": "http://x/rest/v1/rollouts/1/deploygroups/6"}}
    }));
}

#[test]
fn rollout_targets_per_status_defaults_when_absent() {
    let r: RolloutRest = serde_json::from_value(json!({
        "id": 1, "name": "r1", "description": null, "distributionSetId": 5,
        "targetFilterQuery": "name==*", "status": "ready", "totalTargets": 10,
        "createdAt": 1, "lastModifiedAt": 2
    }))
    .unwrap();
    assert_eq!(
        r.total_targets_per_status,
        RolloutTargetsPerStatus::default()
    );
}

#[test]
fn rollout_targets_per_status_sums() {
    let mut acc = RolloutTargetsPerStatus::default();
    acc += RolloutTargetsPerStatus {
        finished: 2,
        running: 1,
        ..Default::default()
    };
    acc += RolloutTargetsPerStatus {
        finished: 3,
        error: 1,
        ..Default::default()
    };
    assert_eq!(
        acc,
        RolloutTargetsPerStatus {
            finished: 5,
            running: 1,
            error: 1,
            ..Default::default()
        }
    );
}

#[test]
fn target_filter_shape() {
    round_trip::<TargetFilterRest>(json!({
        "id": 3, "name": "beta", "query": "controllerId==dev-*",
        "autoAssignDistributionSet": 7, "autoAssignActionType": "forced",
        "createdAt": 1, "lastModifiedAt": 2,
        "_links": {"self": {"href": "http://x/rest/v1/targetfilters/3"}}
    }));
}

#[test]
fn target_filter_no_auto_assign() {
    round_trip::<TargetFilterRest>(json!({
        "id": 3, "name": "beta", "query": "name==*",
        "autoAssignDistributionSet": null, "autoAssignActionType": null,
        "createdAt": 1, "lastModifiedAt": 2, "_links": null
    }));
}

#[test]
fn auto_assign_request_shape() {
    let a = AutoAssignRequest {
        id: 7,
        action_type: Some("soft".into()),
    };
    assert_eq!(
        serde_json::to_value(&a).unwrap(),
        json!({"id": 7, "type": "soft"})
    );
    // type omitted when None
    let a = AutoAssignRequest {
        id: 7,
        action_type: None,
    };
    assert_eq!(serde_json::to_value(&a).unwrap(), json!({"id": 7}));
}

#[test]
fn auto_confirm_state_shape() {
    round_trip::<AutoConfirmState>(json!({"active": true, "activatedAt": 5}));
    // activatedAt omitted when None
    let s = AutoConfirmState {
        active: false,
        activated_at: None,
    };
    assert_eq!(serde_json::to_value(&s).unwrap(), json!({"active": false}));
}

#[test]
fn sm_type_create_shape() {
    round_trip::<SmTypeCreate>(json!({
        "key": "container", "name": "Container",
        "description": "OCI images", "maxAssignments": 3
    }));
    // optional fields omitted when None
    let c = SmTypeCreate {
        key: "os".into(),
        name: "OS".into(),
        description: None,
        max_assignments: None,
    };
    assert_eq!(
        serde_json::to_value(&c).unwrap(),
        json!({"key": "os", "name": "OS"})
    );
}

#[test]
fn ds_type_create_shape() {
    round_trip::<DsTypeCreate>(json!({
        "key": "os_app", "name": "OS with apps", "description": "d",
        "mandatorymodules": [{"id": 1}], "optionalmodules": [{"id": 4}]
    }));
}

#[test]
fn target_type_create_shape() {
    round_trip::<TargetTypeCreate>(json!({
        "name": "gateway", "description": "edge", "colour": "#ff0000",
        "compatibledistributionsettypes": [{"id": 1}, {"id": 2}]
    }));
}

#[test]
fn target_type_field_omitted_when_none() {
    let t = TargetRest {
        controller_id: "d1".into(),
        name: "d1".into(),
        description: None,
        update_status: "unknown".into(),
        security_token: "x".into(),
        created_at: 1,
        last_modified_at: 2,
        address: None,
        ip_address: None,
        last_controller_request_at: None,
        poll_status: None,
        target_type: None,
        request_attributes: true,
        links: serde_json::Value::Null,
    };
    let v = serde_json::to_value(&t).unwrap();
    assert!(v.get("targetType").is_none());
}

#[test]
fn tag_rest_shape() {
    round_trip::<TagRest>(json!({
        "id": 3, "name": "beta", "description": null, "colour": "#00ff00",
        "createdAt": 1, "lastModifiedAt": 2,
        "_links": {"self": {"href": "/rest/v1/targettags/3"}}
    }));
}

#[test]
fn tag_shapes() {
    round_trip::<TagCreate>(json!({
        "name": "beta", "description": "early access", "colour": "#00ff00"
    }));
    // optional fields omitted when None
    let c = TagCreate {
        name: "beta".into(),
        description: None,
        colour: None,
    };
    assert_eq!(serde_json::to_value(&c).unwrap(), json!({"name": "beta"}));
    round_trip::<TagUpdate>(json!({"colour": "#ff0000"}));
}

#[test]
fn metadata_shape() {
    // target / DS metadata: targetVisible omitted
    round_trip::<MetadataRest>(json!({"key": "region", "value": "eu"}));
    // software-module metadata: targetVisible present
    round_trip::<MetadataRest>(json!({
        "key": "region", "value": "eu", "targetVisible": true
    }));
}

#[test]
fn metadata_create_defaults_target_visible() {
    let c: MetadataCreate = serde_json::from_value(json!({"key": "k", "value": "v"})).unwrap();
    assert!(!c.target_visible);
}

#[test]
fn metadata_update_shape() {
    round_trip::<MetadataUpdate>(json!({"value": "v"}));
    round_trip::<MetadataUpdate>(json!({"value": "v", "targetVisible": false}));
}

#[test]
fn paged_envelope() {
    let p = PagedList::new(vec![1, 2, 3], 10);
    let v = serde_json::to_value(&p).unwrap();
    assert_eq!(v, json!({"content": [1, 2, 3], "total": 10, "size": 3}));
    let back: PagedList<i64> = serde_json::from_value(v).unwrap();
    assert_eq!(back.content, vec![1, 2, 3]);
}

#[test]
fn error_body_shape() {
    round_trip::<ErrorBody>(json!({
        "exceptionClass": "x.Y", "errorCode": "hawkbit.server.error.z", "message": "boom"
    }));
}

#[test]
fn assignment_request_shape() {
    let a = DsAssignment {
        id: 5,
        assign_type: Some("forced".into()),
    };
    assert_eq!(
        serde_json::to_value(&a).unwrap(),
        json!({"id": 5, "type": "forced"})
    );
}

#[test]
fn assign_result_shape() {
    round_trip::<AssignResult>(json!({
        "assigned": 1, "alreadyAssigned": 0, "total": 1, "assignedActions": [{"id": 7}]
    }));
}
