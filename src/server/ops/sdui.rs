use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use deno_core::{OpState, op2};
use deno_error::JsErrorBox;
use serde_json::{Map, Value};

use crate::{
    protocol::{
        SduiActionArgument, SduiActionIntent, SduiActionSource, SduiActionValue, SduiEditorBinding,
        SduiFlexDirection, SduiListItem, SduiNode, SduiNodeId, SduiNodeKind, SduiTree,
    },
    server::{
        ops::ClayOpState,
        sdui::{self, SduiValidationError},
    },
};

const SUPPORTED_NODE_KINDS: &[&str] = &[
    "panel",
    "label",
    "button",
    "list",
    "editorView",
    "flex",
    "stack",
];
const DEFAULT_RUNTIME_DOCUMENT_ID: u64 = 1;
const DEFAULT_RUNTIME_UI_VERSION: u64 = 1;

#[op2]
#[string]
pub(super) fn op_clay_sdui_define_node(
    _state: &mut OpState,
    #[string] kind: String,
    #[string] options_json: String,
) -> Result<String, JsErrorBox> {
    if !SUPPORTED_NODE_KINDS.contains(&kind.as_str()) {
        return Err(sdui_error(format!(
            "clay.sdui.invalid_node: unsupported SDUI node kind `{kind}`"
        )));
    }

    let options = if options_json.trim().is_empty() {
        Value::Object(Map::new())
    } else {
        serde_json::from_str::<Value>(&options_json).map_err(|error| {
            sdui_error(format!(
                "clay.sdui.invalid_node: options must be JSON-serializable ({error})"
            ))
        })?
    };

    let mut object = match options {
        Value::Object(object) => object,
        _ => {
            return Err(sdui_error(
                "clay.sdui.invalid_node: node options must be an object",
            ));
        }
    };
    object.insert("kind".to_string(), Value::String(kind));

    serde_json::to_string(&Value::Object(object)).map_err(|error| {
        sdui_error(format!(
            "clay.sdui.invalid_node: failed to encode node definition ({error})"
        ))
    })
}

#[op2(fast)]
pub(super) fn op_clay_sdui_publish_tree(
    state: &mut OpState,
    #[string] tree_json: String,
) -> Result<(), JsErrorBox> {
    let op_state = state.borrow::<Arc<ClayOpState>>();
    let tree = runtime_tree_from_json(&tree_json, op_state.registered_command_ids())?;
    op_state.publish_sdui_tree(tree);
    Ok(())
}

fn runtime_tree_from_json(
    tree_json: &str,
    registered_command_ids: Vec<String>,
) -> Result<SduiTree, JsErrorBox> {
    let root = serde_json::from_str::<Value>(tree_json).map_err(|error| {
        sdui_error(format!(
            "clay.sdui.invalid_tree: published tree must be valid JSON ({error})"
        ))
    })?;
    let mut builder = RuntimeTreeBuilder {
        registered_command_ids: registered_command_ids.into_iter().collect(),
        ..RuntimeTreeBuilder::default()
    };
    let root_id = builder.convert_node(&root)?;
    let tree = SduiTree {
        ui_version: DEFAULT_RUNTIME_UI_VERSION,
        root_id,
        nodes: builder.nodes,
    };
    sdui::validate_runtime_tree(&tree, DEFAULT_RUNTIME_DOCUMENT_ID)
        .map_err(runtime_validation_error)?;
    Ok(tree)
}

#[derive(Debug, Default)]
struct RuntimeTreeBuilder {
    next_id: u64,
    named_ids: BTreeMap<String, SduiNodeId>,
    registered_command_ids: BTreeSet<String>,
    nodes: Vec<SduiNode>,
}

impl RuntimeTreeBuilder {
    fn convert_node(&mut self, value: &Value) -> Result<SduiNodeId, JsErrorBox> {
        let object = value.as_object().ok_or_else(|| {
            sdui_error("clay.sdui.invalid_node: each SDUI node must be an object")
        })?;
        let kind = required_str(object, "kind")?;
        let id = self.node_id(object.get("id"))?;
        let node_kind = match kind {
            "panel" => SduiNodeKind::Panel {
                title: required_str(object, "title")?.to_string(),
                children: self.convert_children(object.get("children"))?,
            },
            "label" => SduiNodeKind::Label {
                text: required_str(object, "text")?.to_string(),
            },
            "button" => SduiNodeKind::Button {
                label: required_str(object, "label")?.to_string(),
                action: self.convert_action(
                    required_object(object, "action")?,
                    SduiActionSource::Button { node_id: id },
                )?,
            },
            "list" => SduiNodeKind::List {
                items: self.convert_list_items(object.get("items"), id)?,
            },
            "editorView" => SduiNodeKind::EditorView {
                binding: SduiEditorBinding {
                    document_id: required_u64(object, "documentId")?,
                    expected_version: optional_u64(object.get("expectedVersion"))?,
                },
            },
            "flex" => SduiNodeKind::Flex {
                direction: match required_str(object, "direction")? {
                    "row" => SduiFlexDirection::Row,
                    "column" => SduiFlexDirection::Column,
                    other => {
                        return Err(sdui_error(format!(
                            "clay.sdui.invalid_node: unsupported flex direction `{other}`"
                        )));
                    }
                },
                children: self.convert_children(object.get("children"))?,
            },
            "stack" => SduiNodeKind::Stack {
                children: self.convert_children(object.get("children"))?,
            },
            other => {
                return Err(sdui_error(format!(
                    "clay.sdui.invalid_node: unsupported SDUI node kind `{other}`"
                )));
            }
        };
        self.nodes.push(SduiNode::new(id, node_kind));
        Ok(id)
    }

    fn node_id(&mut self, value: Option<&Value>) -> Result<SduiNodeId, JsErrorBox> {
        match value {
            Some(Value::Number(number)) => number
                .as_u64()
                .filter(|id| *id > 0)
                .map(SduiNodeId)
                .ok_or_else(|| {
                    sdui_error("clay.sdui.invalid_node: numeric node id must be a positive integer")
                }),
            Some(Value::String(name)) => {
                if name.trim().is_empty() {
                    return Err(sdui_error(
                        "clay.sdui.invalid_node: string node id must not be empty",
                    ));
                }
                if let Some(id) = self.named_ids.get(name) {
                    Ok(*id)
                } else {
                    let id = self.allocate_id();
                    self.named_ids.insert(name.clone(), id);
                    Ok(id)
                }
            }
            Some(_) => Err(sdui_error(
                "clay.sdui.invalid_node: node id must be a string or positive integer",
            )),
            None => Ok(self.allocate_id()),
        }
    }

    fn allocate_id(&mut self) -> SduiNodeId {
        self.next_id += 1;
        SduiNodeId(self.next_id)
    }

    fn convert_children(&mut self, value: Option<&Value>) -> Result<Vec<SduiNodeId>, JsErrorBox> {
        let Some(value) = value else {
            return Ok(Vec::new());
        };
        let children = value
            .as_array()
            .ok_or_else(|| sdui_error("clay.sdui.invalid_node: children must be an array"))?;
        children
            .iter()
            .map(|child| self.convert_node(child))
            .collect()
    }

    fn convert_list_items(
        &mut self,
        value: Option<&Value>,
        list_id: SduiNodeId,
    ) -> Result<Vec<SduiListItem>, JsErrorBox> {
        let Some(value) = value else {
            return Ok(Vec::new());
        };
        let items = value
            .as_array()
            .ok_or_else(|| sdui_error("clay.sdui.invalid_node: list items must be an array"))?;
        items
            .iter()
            .map(|item| {
                let object = item.as_object().ok_or_else(|| {
                    sdui_error("clay.sdui.invalid_node: list items must be objects")
                })?;
                let item_id = required_str(object, "id")?.to_string();
                Ok(SduiListItem {
                    id: item_id.clone(),
                    label: required_str(object, "label")?.to_string(),
                    detail: optional_string(object.get("detail"))?,
                    action: match object.get("action") {
                        Some(Value::Null) | None => None,
                        Some(Value::Object(action)) => Some(self.convert_action(
                            action,
                            SduiActionSource::ListItem {
                                node_id: list_id,
                                item_id,
                            },
                        )?),
                        Some(_) => {
                            return Err(sdui_error(
                                "clay.sdui.invalid_action: list item action must be an object",
                            ));
                        }
                    },
                })
            })
            .collect()
    }

    fn convert_action(
        &self,
        object: &Map<String, Value>,
        source: SduiActionSource,
    ) -> Result<SduiActionIntent, JsErrorBox> {
        let command_id = required_str(object, "commandId")?.to_string();
        if !is_builtin_sdui_command(&command_id)
            && !self.registered_command_ids.contains(&command_id)
        {
            return Err(sdui_error(format!(
                "clay.sdui.invalid_action: command `{command_id}` is not allowed for SDUI actions; register package commands before publishing package-owned SDUI"
            )));
        }
        let arguments = match object.get("arguments") {
            Some(Value::Null) | None => Vec::new(),
            Some(Value::Object(arguments)) => arguments
                .iter()
                .map(|(name, value)| {
                    Ok(SduiActionArgument {
                        name: name.clone(),
                        value: action_value(value)?,
                    })
                })
                .collect::<Result<Vec<_>, JsErrorBox>>()?,
            Some(_) => {
                return Err(sdui_error(
                    "clay.sdui.invalid_action: action arguments must be an object",
                ));
            }
        };
        Ok(SduiActionIntent {
            command_id,
            source,
            arguments,
        })
    }
}

fn is_builtin_sdui_command(command_id: &str) -> bool {
    matches!(
        command_id,
        "workspace.refresh" | "document.focus_active" | "document.open_recent"
    )
}

fn required_object<'a>(
    object: &'a Map<String, Value>,
    field: &str,
) -> Result<&'a Map<String, Value>, JsErrorBox> {
    object.get(field).and_then(Value::as_object).ok_or_else(|| {
        sdui_error(format!(
            "clay.sdui.invalid_node: `{field}` must be an object"
        ))
    })
}

fn required_str<'a>(object: &'a Map<String, Value>, field: &str) -> Result<&'a str, JsErrorBox> {
    object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            sdui_error(format!(
                "clay.sdui.invalid_node: `{field}` must be a non-empty string"
            ))
        })
}

fn required_u64(object: &Map<String, Value>, field: &str) -> Result<u64, JsErrorBox> {
    object.get(field).and_then(Value::as_u64).ok_or_else(|| {
        sdui_error(format!(
            "clay.sdui.invalid_node: `{field}` must be an unsigned integer"
        ))
    })
}

fn optional_u64(value: Option<&Value>) -> Result<Option<u64>, JsErrorBox> {
    match value {
        Some(Value::Null) | None => Ok(None),
        Some(value) => value.as_u64().map(Some).ok_or_else(|| {
            sdui_error("clay.sdui.invalid_node: optional version must be an unsigned integer")
        }),
    }
}

fn optional_string(value: Option<&Value>) -> Result<Option<String>, JsErrorBox> {
    match value {
        Some(Value::Null) | None => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(_) => Err(sdui_error(
            "clay.sdui.invalid_node: optional string fields must be strings",
        )),
    }
}

fn action_value(value: &Value) -> Result<SduiActionValue, JsErrorBox> {
    match value {
        Value::String(value) => Ok(SduiActionValue::String(value.clone())),
        Value::Bool(value) => Ok(SduiActionValue::Bool(*value)),
        Value::Number(value) => {
            if let Some(value) = value.as_i64() {
                Ok(SduiActionValue::I64(value))
            } else if let Some(value) = value.as_u64() {
                Ok(SduiActionValue::U64(value))
            } else {
                Err(sdui_error(
                    "clay.sdui.invalid_action: numeric action arguments must be integers",
                ))
            }
        }
        Value::Null | Value::Array(_) | Value::Object(_) => Err(sdui_error(
            "clay.sdui.invalid_action: action arguments must be primitive string, boolean, or integer values",
        )),
    }
}

fn runtime_validation_error(error: SduiValidationError) -> JsErrorBox {
    sdui_error(format!(
        "clay.sdui.invalid_tree: published tree failed validation ({error:?})"
    ))
}

fn sdui_error(message: impl Into<String>) -> JsErrorBox {
    JsErrorBox::generic(message.into())
}
