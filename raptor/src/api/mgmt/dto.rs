use crate::entity::software_module;
use serde::Serialize;
use serde_json::json;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SmRest {
    pub id: i64,
    pub name: String,
    pub version: String,
    #[serde(rename = "type")]
    pub module_type: String,
    pub vendor: Option<String>,
    pub description: Option<String>,
    pub created_at: i64,
    pub last_modified_at: i64,
    #[serde(rename = "_links")]
    pub links: serde_json::Value,
}

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
