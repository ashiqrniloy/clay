//! Window-layout persistence commands. Validation lives in
//! `clay::shell::layout_persist`; hostile documents never reach disk.

#[cfg(test)]
mod tests {
    #[test]
    fn hostile_layout_is_rejected() {
        let hostile = serde_json::json!({
            "version": 2,
            "tabs": [{ "workspaceRoot": "", "splitTree": { "leaf": { "paneId": 0 } } }]
        });
        assert!(clay::shell::parse_window_state_json(&hostile).is_none());
        assert!(clay::shell::save_window_state_from_json(&hostile).is_err());
    }

    #[test]
    fn tree_without_pane_one_degrades() {
        let missing_pane_one = serde_json::json!({
            "version": 2,
            "activeTab": 0,
            "tabs": [{
                "workspaceRoot": "/tmp/ws",
                "activePane": 9,
                "splitTree": {
                    "split": {
                        "orientation": "horizontal",
                        "ratio": 0.5,
                        "first": { "leaf": { "paneId": 9 } },
                        "second": { "leaf": { "paneId": 10 } }
                    }
                },
                "slots": [],
                "panes": {}
            }]
        });
        let parsed = clay::shell::parse_window_state_json(&missing_pane_one)
            .expect("workspace root is enough to keep the tab");
        assert_eq!(parsed["tabs"][0]["splitTree"], serde_json::Value::Null);
    }
}
