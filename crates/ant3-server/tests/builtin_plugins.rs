use ant3_platform::{organization, store::Store};
use serde_json::json;

#[test]
fn retired_tools_are_unavailable_for_preset_binding() {
    let store = Store::memory().unwrap();
    for id in ["text", "web.fetch", "web.search"] {
        assert!(organization::package(id).is_none());
        assert!(store.publish_definition(json!({
            "name":"official","version":"1","purpose":"test","objective":"test",
            "instructions":"test","protocol":"demo","plugins":[{"id":id,"version":"1.0.0"}]
        })).is_err());
    }
}
