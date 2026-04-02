//! Serializes the command graph into a flat node array matching the
//! `ClientboundCommandsPacket` wire format.
//!
//! Uses BFS traversal to assign node indices matching vanilla's ordering.

use crate::arguments::ArgumentType;
use crate::nodes::{CommandNode, RootCommandNode};
use std::collections::VecDeque;
use std::sync::Arc;

/// Node type flags used in the wire format.
const TYPE_ROOT: u8 = 0;
const TYPE_LITERAL: u8 = 1;
const TYPE_ARGUMENT: u8 = 2;
const FLAG_EXECUTABLE: u8 = 0x04;
const FLAG_REDIRECT: u8 = 0x08;
const FLAG_SUGGESTIONS: u8 = 0x10;

/// A flattened command tree ready for packet encoding.
#[derive(Debug, Clone)]
pub struct CommandTreeData {
    /// The flat list of nodes.
    pub nodes: Vec<CommandNodeData>,
    /// Index of the root node.
    pub root_index: i32,
}

/// A single node in the flattened command tree.
#[derive(Debug, Clone)]
pub struct CommandNodeData {
    /// Packed flags: bits 0-1 = type, bit 2 = executable, bit 3 = redirect,
    /// bit 4 = custom suggestions.
    pub flags: u8,
    /// Child node indices.
    pub children: Vec<i32>,
    /// Redirect target index (if FLAG_REDIRECT is set).
    pub redirect_node: Option<i32>,
    /// Name of the node (literal or argument name; `None` for root).
    pub name: Option<String>,
    /// Parser info for argument nodes.
    pub parser: Option<ArgumentParser>,
    /// Custom suggestions type identifier.
    pub suggestions_type: Option<String>,
}

/// Parser info attached to argument nodes.
#[derive(Debug, Clone)]
pub struct ArgumentParser {
    /// Registry ID of the argument type.
    pub parser_id: i32,
    /// Serialized parser properties.
    pub properties: Vec<u8>,
}

/// Serializes a command tree rooted at `root`, filtered by `source`'s
/// permissions. Produces a flat node array in BFS order.
pub fn serialize_tree<S>(root: &RootCommandNode<S>, source: &S) -> CommandTreeData {
    let mut result_nodes: Vec<CommandNodeData> = Vec::new();
    let mut queue: VecDeque<(&CommandNode<S>, usize)> = VecDeque::new();
    // Map from Arc pointer identity to node index for redirect resolution.
    let mut ptr_to_idx: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
    // Deferred redirect fixups: (source_node_index, target_arc_ptr).
    let mut redirect_fixups: Vec<(usize, usize)> = Vec::new();

    // Root is always index 0.
    let root_cmd_node = CommandNode::Root(root.clone());
    result_nodes.push(CommandNodeData {
        flags: TYPE_ROOT,
        children: Vec::new(),
        redirect_node: None,
        name: None,
        parser: None,
        suggestions_type: None,
    });
    queue.push_back((&root_cmd_node, 0));

    let mut next_idx = 1usize;

    while let Some((parent_node, parent_idx)) = queue.pop_front() {
        let visible_children: Vec<(&String, &CommandNode<S>)> = parent_node
            .children()
            .iter()
            .filter(|(_, child)| child.can_use(source))
            .collect();

        let mut child_indices = Vec::new();

        for (_name, child) in &visible_children {
            let child_idx = next_idx;
            next_idx += 1;
            child_indices.push(child_idx as i32);

            // Register this node's Arc pointer for redirect resolution.
            let child_ptr = std::ptr::from_ref(*child) as usize;
            ptr_to_idx.insert(child_ptr, child_idx);

            let mut flags = match child {
                CommandNode::Root(_) => TYPE_ROOT,
                CommandNode::Literal(_) => TYPE_LITERAL,
                CommandNode::Argument(_) => TYPE_ARGUMENT,
            };

            if child.command().is_some() {
                flags |= FLAG_EXECUTABLE;
            }

            // Check for redirect and record a fixup.
            let has_redirect = if let Some(redirect_target) = child.redirect() {
                let target_ptr = Arc::as_ptr(redirect_target) as usize;
                redirect_fixups.push((child_idx, target_ptr));
                true
            } else {
                false
            };
            if has_redirect {
                flags |= FLAG_REDIRECT;
            }

            let parser = if let CommandNode::Argument(arg) = child {
                // Entity and GameProfile args need server-side suggestions
                // so the client sends ServerboundCommandSuggestionPacket.
                let has_suggestions = arg.suggestions_type.is_some()
                    || matches!(
                        &arg.argument_type,
                        ArgumentType::Entity { .. } | ArgumentType::GameProfile
                    );
                if has_suggestions {
                    flags |= FLAG_SUGGESTIONS;
                }
                let mut props = Vec::new();
                arg.argument_type.write_properties(&mut props);
                Some(ArgumentParser {
                    parser_id: arg.argument_type.registry_id(),
                    properties: props,
                })
            } else {
                None
            };

            let suggestions_type = if let CommandNode::Argument(arg) = child {
                if arg.suggestions_type.is_some() {
                    arg.suggestions_type.clone()
                } else if matches!(
                    &arg.argument_type,
                    ArgumentType::Entity { .. } | ArgumentType::GameProfile
                ) {
                    Some("minecraft:ask_server".to_string())
                } else {
                    None
                }
            } else {
                None
            };

            result_nodes.push(CommandNodeData {
                flags,
                children: Vec::new(),
                redirect_node: None,
                name: Some(child.name().to_string()),
                parser,
                suggestions_type,
            });

            queue.push_back((child, child_idx));
        }

        result_nodes[parent_idx].children = child_indices;
    }

    // Resolve redirect fixups: map Arc pointer → node index.
    for (node_idx, target_ptr) in redirect_fixups {
        if let Some(&target_idx) = ptr_to_idx.get(&target_ptr) {
            result_nodes[node_idx].redirect_node = Some(target_idx as i32);
        } else {
            // Redirect target not found in the visible tree — point to root (0)
            // as a safe fallback so the client doesn't receive an invalid index.
            result_nodes[node_idx].redirect_node = Some(0);
        }
    }

    CommandTreeData {
        nodes: result_nodes,
        root_index: 0,
    }
}
