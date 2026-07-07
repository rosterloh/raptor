use crate::entity::{distribution_set, software_module, target};
pub use raptor_api_types::{DsRest, PollStatus, SmRest, TargetRest};
use serde_json::json;
use std::time::Duration;

pub fn sm_rest(m: &software_module::Model, type_key: &str, base: &str) -> SmRest {
    SmRest {
        id: m.id,
        name: m.name.clone(),
        version: m.version.clone(),
        module_type: type_key.to_string(),
        vendor: m.vendor.clone(),
        description: m.description.clone(),
        created_at: m.created_at,
        last_modified_at: m.updated_at,
        links: json!({"self": {"href": format!("{base}/rest/v1/softwaremodules/{}", m.id)}}),
    }
}

pub fn target_rest(t: &target::Model, poll_interval: Duration, base: &str) -> TargetRest {
    let poll_status = t.last_poll_at.map(|last| {
        let next = last + poll_interval.as_millis() as i64;
        PollStatus {
            last_request_at: last,
            next_expected_request_at: next,
            overdue: crate::util::now_ms() > next,
        }
    });
    TargetRest {
        controller_id: t.controller_id.clone(),
        name: t.name.clone(),
        description: t.description.clone(),
        update_status: t.update_status.clone(),
        security_token: t.security_token.clone(),
        created_at: t.created_at,
        last_modified_at: t.updated_at,
        address: t.address.clone(),
        ip_address: t.address.clone(),
        last_controller_request_at: t.last_poll_at,
        poll_status,
        links: json!({"self": {"href": format!("{base}/rest/v1/targets/{}", t.controller_id)}}),
    }
}

pub fn ds_rest(
    ds: &distribution_set::Model,
    type_key: &str,
    modules: Vec<SmRest>,
    base: &str,
) -> DsRest {
    DsRest {
        id: ds.id,
        name: ds.name.clone(),
        version: ds.version.clone(),
        ds_type: type_key.to_string(),
        description: ds.description.clone(),
        required_migration_step: ds.required_migration_step,
        complete: ds.complete,
        deleted: false,
        created_at: ds.created_at,
        last_modified_at: ds.updated_at,
        modules,
        links: json!({"self": {"href": format!("{base}/rest/v1/distributionsets/{}", ds.id)}}),
    }
}
