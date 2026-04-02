//! Command dispatcher: parse input against the graph, execute, and suggest.

use crate::CommandError;
use crate::argument_parser::parse_argument;
use crate::arguments::ArgumentType;
use crate::context::{CommandContext, ParseResults, ParsedArgument, StringRange, Suggestion};
use crate::nodes::{CommandNode, LiteralBuilder, RootCommandNode};
use crate::serializer::{CommandTreeData, serialize_tree};
use crate::string_reader::StringReader;
use std::collections::HashMap;

/// The top-level command dispatcher holding the full command graph.
pub struct CommandDispatcher<S> {
    /// The root node of the command graph.
    pub root: RootCommandNode<S>,
}

impl<S: Clone + Send + Sync + 'static> CommandDispatcher<S> {
    /// Creates a new empty dispatcher.
    pub fn new() -> Self {
        Self {
            root: RootCommandNode::new(),
        }
    }

    /// Registers a top-level command.
    pub fn register(&mut self, builder: LiteralBuilder<S>) {
        let node = builder.build();
        self.root.add_child(node);
    }

    /// Parses input against the command graph, returning a ready-to-execute
    /// context with parsed arguments.
    ///
    /// # Errors
    ///
    /// Returns [`CommandError::Parse`] if the command name is unknown, the
    /// source lacks permission, or any argument fails to parse.
    pub fn parse(&self, input: &str, source: S) -> Result<ParseResults<S>, CommandError> {
        let mut reader = StringReader::new(input, 0);

        // Read the first word as the command name.
        let cmd_name = reader.read_word();
        if cmd_name.is_empty() {
            return Err(CommandError::Parse("Expected command name".to_string()));
        }

        let node = self.root.children.get(cmd_name).ok_or_else(|| {
            CommandError::Parse(format!(
                "Unknown or incomplete command, see below for error\n{cmd_name}<--[HERE]"
            ))
        })?;

        // Check requirement
        if !node.can_use(&source) {
            return Err(CommandError::Parse(format!(
                "Unknown or incomplete command, see below for error\n{cmd_name}<--[HERE]"
            )));
        }

        // Now walk deeper into the tree, parsing arguments.
        let mut arguments = HashMap::new();
        let mut current = node;
        let mut command = node.command().cloned();

        loop {
            reader.skip_whitespace();
            if !reader.can_read() {
                break;
            }

            match try_match_child(current, &source, input, &mut reader, &mut arguments) {
                ChildMatch::Matched { node: next, cmd } => {
                    current = next;
                    if let Some(c) = cmd {
                        command = Some(c);
                    }
                },
                ChildMatch::NoMatch => {
                    let pos = reader.cursor();
                    return Err(CommandError::Parse(format!(
                        "Incorrect argument for command at position {pos}"
                    )));
                },
                ChildMatch::Error(e) => return Err(e),
            }
        }

        Ok(ParseResults {
            context: CommandContext {
                source,
                input: input.to_string(),
                arguments,
                command,
            },
            cursor: reader.cursor(),
        })
    }

    /// Executes a previously-parsed command.
    ///
    /// # Errors
    ///
    /// Returns [`CommandError::Parse`] if the parse result has no resolved
    /// command. May also propagate errors from the command handler itself.
    pub fn execute(&self, parse: &ParseResults<S>) -> Result<i32, CommandError> {
        let cmd = parse
            .context
            .command
            .as_ref()
            .ok_or_else(|| CommandError::Parse("Incomplete command".to_string()))?;
        (cmd)(&parse.context)
    }

    /// Collects tab-completion suggestions for the given input.
    ///
    /// The returned [`Suggestion`]s have their `range` set relative to
    /// the full `input` string, so callers can map them directly to the
    /// protocol's `start`/`length` fields.
    pub fn get_completions(
        &self,
        input: &str,
        source: &S,
        player_names: &[String],
    ) -> Vec<Suggestion> {
        let parts: Vec<&str> = input.splitn(2, ' ').collect();
        let partial_cmd = parts[0];

        // If we're still on the first word, suggest command names.
        if parts.len() == 1 && !input.ends_with(' ') {
            return self
                .root
                .children
                .keys()
                .filter(|name| name.starts_with(partial_cmd))
                .filter(|name| {
                    self.root
                        .children
                        .get(name.as_str())
                        .is_some_and(|n| n.can_use(source))
                })
                .map(|name| Suggestion {
                    range: StringRange::new(0, partial_cmd.len()),
                    text: name.clone(),
                    tooltip: None,
                })
                .collect();
        }

        // We're past the first word — walk the tree to find the current node,
        // then suggest its children.
        if let Some(node) = self.root.children.get(partial_cmd) {
            if !node.can_use(source) {
                return Vec::new();
            }
            let remaining = if parts.len() > 1 { parts[1] } else { "" };
            // Offset = length of command name + 1 (for the space separator)
            let offset = partial_cmd.len() + 1;
            return collect_child_suggestions(node, remaining, offset, source, player_names);
        }

        Vec::new()
    }

    /// Serializes the command tree, filtered by the given source's
    /// permissions.
    pub fn serialize_tree(&self, source: &S) -> CommandTreeData {
        serialize_tree(&self.root, source)
    }
}

impl<S: Clone + Send + Sync + 'static> Default for CommandDispatcher<S> {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of attempting to match input against child nodes.
enum ChildMatch<'a, S> {
    /// A child node matched.
    Matched {
        /// The matched child node.
        node: &'a CommandNode<S>,
        /// The command handler, if the matched node is executable.
        cmd: Option<super::nodes::CommandFn<S>>,
    },
    /// No child matched the input.
    NoMatch,
    /// A parse error occurred (single-child propagation).
    Error(CommandError),
}

/// Tries to match the remaining input against children of `current`.
///
/// On success, advances `reader` past the matched token and inserts
/// any parsed argument into `arguments`.
fn try_match_child<'a, S: Clone + Send + Sync + 'static>(
    current: &'a CommandNode<S>,
    source: &S,
    input: &'a str,
    reader: &mut StringReader<'a>,
    arguments: &mut HashMap<String, ParsedArgument>,
) -> ChildMatch<'a, S> {
    let remaining = reader.remaining().to_string();

    for child in current.children().values() {
        match child {
            CommandNode::Literal(lit) => {
                let is_match = remaining.starts_with(&lit.literal)
                    && (remaining.len() == lit.literal.len()
                        || remaining.as_bytes().get(lit.literal.len()) == Some(&b' '));
                if !is_match || !child.can_use(source) {
                    continue;
                }
                *reader = StringReader::new(input, reader.cursor() + lit.literal.len());
                return ChildMatch::Matched {
                    node: child,
                    cmd: child.command().cloned(),
                };
            },
            CommandNode::Argument(arg) => {
                if !child.can_use(source) {
                    continue;
                }
                let start = reader.cursor();
                let result = match parse_argument(reader, &arg.argument_type) {
                    Ok(r) => r,
                    Err(e) if current.children().len() == 1 => {
                        *reader = StringReader::new(input, start);
                        return ChildMatch::Error(e);
                    },
                    Err(_) => {
                        *reader = StringReader::new(input, start);
                        continue;
                    },
                };
                let range = StringRange::new(start, reader.cursor());
                arguments.insert(arg.name.clone(), ParsedArgument { range, result });
                return ChildMatch::Matched {
                    node: child,
                    cmd: child.command().cloned(),
                };
            },
            CommandNode::Root(_) => {},
        }
    }

    ChildMatch::NoMatch
}

/// Recursively collects suggestions from child nodes.
///
/// `offset` is the character position in the original input where
/// `remaining` starts. This lets us build correct [`StringRange`]s.
fn collect_child_suggestions<S>(
    node: &CommandNode<S>,
    remaining: &str,
    offset: usize,
    source: &S,
    player_names: &[String],
) -> Vec<Suggestion> {
    let parts: Vec<&str> = remaining.splitn(2, ' ').collect();
    let current_word = parts[0];

    // If there's more input after a space, try to walk deeper.
    if parts.len() > 1 {
        let next_offset = offset + current_word.len() + 1;
        // Try to match the current word to a child.
        for child in node.children().values() {
            match child {
                CommandNode::Literal(lit) if lit.literal == current_word => {
                    if !child.can_use(source) {
                        continue;
                    }
                    return collect_child_suggestions(
                        child,
                        parts[1],
                        next_offset,
                        source,
                        player_names,
                    );
                },
                CommandNode::Argument(_) => {
                    if !child.can_use(source) {
                        continue;
                    }
                    return collect_child_suggestions(
                        child,
                        parts[1],
                        next_offset,
                        source,
                        player_names,
                    );
                },
                _ => {},
            }
        }
        return Vec::new();
    }

    // We're at the last word — suggest matching children.
    let range = StringRange::new(offset, offset + current_word.len());
    let mut suggestions = Vec::new();
    for child in node.children().values() {
        if !child.can_use(source) {
            continue;
        }
        match child {
            CommandNode::Literal(lit) => {
                if lit.literal.starts_with(current_word) {
                    suggestions.push(Suggestion {
                        range,
                        text: lit.literal.clone(),
                        tooltip: None,
                    });
                }
            },
            CommandNode::Argument(arg) => {
                suggest_for_argument(
                    &arg.argument_type,
                    &arg.name,
                    current_word,
                    range,
                    player_names,
                    &mut suggestions,
                );
            },
            _ => {},
        }
    }
    suggestions
}

/// Builds suggestions for an argument node based on its type.
fn suggest_for_argument(
    arg_type: &ArgumentType,
    arg_name: &str,
    current_word: &str,
    range: StringRange,
    player_names: &[String],
    suggestions: &mut Vec<Suggestion>,
) {
    let is_entity = matches!(
        arg_type,
        ArgumentType::Entity { .. } | ArgumentType::GameProfile
    );
    if !is_entity {
        suggestions.push(Suggestion {
            range,
            text: format!("<{arg_name}>"),
            tooltip: None,
        });
        return;
    }

    // Entity/game-profile args: suggest selector prefixes and player names.
    if current_word.starts_with('@') || current_word.is_empty() {
        for sel in &["@a", "@e", "@p", "@r", "@s", "@n"] {
            if sel.starts_with(current_word) {
                suggestions.push(Suggestion {
                    range,
                    text: (*sel).to_string(),
                    tooltip: None,
                });
            }
        }
    }
    let lower = current_word.to_lowercase();
    for name in player_names {
        if name.to_lowercase().starts_with(&lower) {
            suggestions.push(Suggestion {
                range,
                text: name.clone(),
                tooltip: None,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use crate::argument_access::get_integer;
    use crate::arguments::{ArgumentType, StringKind};
    use crate::nodes::{CommandNode, argument, literal};

    /// Minimal test source with a permission level.
    #[derive(Clone)]
    struct TestSource {
        permission_level: u32,
    }

    impl TestSource {
        fn has_permission(&self, level: u32) -> bool {
            self.permission_level >= level
        }
    }

    fn make_source(permission_level: u32) -> TestSource {
        TestSource { permission_level }
    }

    // ── Dispatcher: parse & execute ─────────────────────────────────────

    #[test]
    fn dispatcher_executes_literal_command() {
        let mut d = CommandDispatcher::new();
        d.register(literal("ping").executes(|_| Ok(42)));
        let src = make_source(4);
        let parse = d.parse("ping", src).unwrap();
        assert_eq!(d.execute(&parse).unwrap(), 42);
    }

    #[test]
    fn dispatcher_returns_error_for_unknown_command() {
        let d = CommandDispatcher::<TestSource>::new();
        let src = make_source(4);
        assert!(d.parse("unknowncommand", src).is_err());
    }

    #[test]
    fn dispatcher_parses_integer_argument() {
        let mut d = CommandDispatcher::new();
        d.register(
            literal("test").then(
                argument(
                    "n",
                    ArgumentType::Integer {
                        min: Some(0),
                        max: None,
                    },
                )
                .executes(|ctx| {
                    let n = get_integer(ctx, "n")?;
                    Ok(n)
                }),
            ),
        );
        let src = make_source(4);
        let parse = d.parse("test 7", src).unwrap();
        assert_eq!(d.execute(&parse).unwrap(), 7);
    }

    #[test]
    fn dispatcher_integer_argument_rejects_out_of_range() {
        let mut d = CommandDispatcher::new();
        d.register(
            literal("setval").then(
                argument(
                    "n",
                    ArgumentType::Integer {
                        min: Some(1),
                        max: Some(10),
                    },
                )
                .executes(|_| Ok(1)),
            ),
        );
        let src = make_source(4);
        assert!(d.parse("setval 99", src).is_err());
    }

    #[test]
    fn permission_requirement_blocks_low_permission_source() {
        let mut d = CommandDispatcher::new();
        d.register(
            literal("stop")
                .requires(|s: &TestSource| s.has_permission(4))
                .executes(|_| Ok(1)),
        );
        let src = make_source(0);
        assert!(d.parse("stop", src).is_err());
    }

    #[test]
    fn permission_requirement_allows_high_permission_source() {
        let mut d = CommandDispatcher::new();
        d.register(
            literal("stop")
                .requires(|s: &TestSource| s.has_permission(4))
                .executes(|_| Ok(1)),
        );
        let src = make_source(4);
        let parse = d.parse("stop", src).unwrap();
        assert_eq!(d.execute(&parse).unwrap(), 1);
    }

    #[test]
    fn dispatcher_handles_nested_literals() {
        let mut d = CommandDispatcher::new();
        d.register(
            literal("time").then(
                literal("set").then(
                    argument(
                        "value",
                        ArgumentType::Integer {
                            min: None,
                            max: None,
                        },
                    )
                    .executes(|ctx| get_integer(ctx, "value")),
                ),
            ),
        );
        let src = make_source(4);
        let parse = d.parse("time set 1000", src).unwrap();
        assert_eq!(d.execute(&parse).unwrap(), 1000);
    }

    // ── Serialization ───────────────────────────────────────────────────

    #[test]
    fn serialize_tree_root_node_has_zero_flags() {
        let d = CommandDispatcher::<TestSource>::new();
        let src = make_source(4);
        let tree = d.serialize_tree(&src);
        assert_eq!(tree.nodes[0].flags & 0b11, 0b00);
    }

    #[test]
    fn serialize_tree_literal_node_has_correct_flags() {
        let mut d = CommandDispatcher::new();
        d.register(literal("help").executes(|_| Ok(1)));
        let src = make_source(4);
        let tree = d.serialize_tree(&src);
        let help_node = tree
            .nodes
            .iter()
            .find(|n| n.name.as_deref() == Some("help"))
            .unwrap();
        assert_eq!(help_node.flags & 0b11, 0b01, "should be literal type");
        assert!(help_node.flags & 0b100 != 0, "should be executable");
    }

    #[test]
    fn serialize_tree_argument_node_has_correct_flags() {
        let mut d = CommandDispatcher::new();
        d.register(
            literal("test").then(
                argument(
                    "n",
                    ArgumentType::Integer {
                        min: None,
                        max: None,
                    },
                )
                .executes(|_| Ok(1)),
            ),
        );
        let src = make_source(4);
        let tree = d.serialize_tree(&src);
        let arg_node = tree
            .nodes
            .iter()
            .find(|n| n.name.as_deref() == Some("n"))
            .unwrap();
        assert_eq!(arg_node.flags & 0b11, 0b10, "should be argument type");
        assert!(arg_node.parser.is_some(), "should have parser info");
    }

    #[test]
    fn serialize_tree_filters_by_permission() {
        let mut d = CommandDispatcher::new();
        d.register(literal("help").executes(|_| Ok(1)));
        d.register(
            literal("stop")
                .requires(|s: &TestSource| s.has_permission(4))
                .executes(|_| Ok(1)),
        );
        let src = make_source(0);
        let tree = d.serialize_tree(&src);
        assert!(tree.nodes.iter().any(|n| n.name.as_deref() == Some("help")));
        assert!(!tree.nodes.iter().any(|n| n.name.as_deref() == Some("stop")));
    }

    // ── Completions ─────────────────────────────────────────────────────

    #[test]
    fn completions_returns_registered_command_names_at_root() {
        let mut d = CommandDispatcher::new();
        d.register(literal("help").executes(|_| Ok(1)));
        d.register(literal("stop").executes(|_| Ok(1)));
        let src = make_source(4);
        let completions = d.get_completions("", &src, &[]);
        let texts: Vec<_> = completions.iter().map(|s| s.text.as_str()).collect();
        assert!(texts.contains(&"help"));
        assert!(texts.contains(&"stop"));
    }

    #[test]
    fn completions_filters_by_prefix() {
        let mut d = CommandDispatcher::new();
        d.register(literal("give").executes(|_| Ok(1)));
        d.register(literal("gamemode").executes(|_| Ok(1)));
        d.register(literal("kill").executes(|_| Ok(1)));
        let src = make_source(4);
        let completions = d.get_completions("g", &src, &[]);
        let texts: Vec<_> = completions.iter().map(|s| s.text.as_str()).collect();
        assert!(texts.contains(&"give"), "should include give");
        assert!(texts.contains(&"gamemode"), "should include gamemode");
        assert!(!texts.contains(&"kill"), "should not include kill");
    }

    #[test]
    fn completions_respects_permissions() {
        let mut d = CommandDispatcher::new();
        d.register(literal("help").executes(|_| Ok(1)));
        d.register(
            literal("stop")
                .requires(|s: &TestSource| s.has_permission(4))
                .executes(|_| Ok(1)),
        );
        let src = make_source(0);
        let completions = d.get_completions("", &src, &[]);
        let texts: Vec<_> = completions.iter().map(|s| s.text.as_str()).collect();
        assert!(texts.contains(&"help"), "should include help");
        assert!(!texts.contains(&"stop"), "should not include stop");
    }

    // ── Description field ───────────────────────────────────────────────

    #[test]
    fn literal_node_stores_description() {
        let mut d: CommandDispatcher<TestSource> = CommandDispatcher::new();
        d.register(
            literal("help")
                .description("Shows the help menu")
                .executes(|_| Ok(1)),
        );
        let node = CommandNode::Root(d.root);
        let desc = node.children().get("help").unwrap().description();
        assert_eq!(desc, Some("Shows the help menu"));
    }

    #[test]
    fn argument_node_stores_description() {
        let mut d: CommandDispatcher<TestSource> = CommandDispatcher::new();
        d.register(
            literal("test").then(
                argument("name", ArgumentType::String(StringKind::SingleWord))
                    .description("Player name")
                    .executes(|_| Ok(1)),
            ),
        );
        let node = CommandNode::Root(d.root);
        let test_node = node.children().get("test").unwrap();
        let name_node = test_node.children().get("name").unwrap();
        let desc = name_node.description();
        assert_eq!(desc, Some("Player name"));
    }

    #[test]
    fn node_without_description_returns_none() {
        let mut d: CommandDispatcher<TestSource> = CommandDispatcher::new();
        d.register(literal("ping").executes(|_| Ok(1)));
        let node = CommandNode::Root(d.root);
        let desc = node.children().get("ping").unwrap().description();
        assert_eq!(desc, None);
    }

    // ── Username autocomplete ───────────────────────────────────────────

    #[test]
    fn completions_suggest_player_names_for_entity_arg() {
        let mut d = CommandDispatcher::new();
        d.register(
            literal("kick").then(
                argument(
                    "target",
                    ArgumentType::Entity {
                        single: true,
                        player_only: true,
                    },
                )
                .executes(|_| Ok(1)),
            ),
        );
        let src = make_source(4);
        let names = vec!["Alice".to_string(), "Bob".to_string()];
        let completions = d.get_completions("kick ", &src, &names);
        let texts: Vec<_> = completions.iter().map(|s| s.text.as_str()).collect();
        assert!(texts.contains(&"Alice"), "should suggest Alice");
        assert!(texts.contains(&"Bob"), "should suggest Bob");
    }

    #[test]
    fn completions_filter_player_names_by_prefix() {
        let mut d = CommandDispatcher::new();
        d.register(
            literal("kick").then(
                argument(
                    "target",
                    ArgumentType::Entity {
                        single: true,
                        player_only: true,
                    },
                )
                .executes(|_| Ok(1)),
            ),
        );
        let src = make_source(4);
        let names = vec!["Alice".to_string(), "Bob".to_string()];
        let completions = d.get_completions("kick A", &src, &names);
        let texts: Vec<_> = completions.iter().map(|s| s.text.as_str()).collect();
        assert!(texts.contains(&"Alice"), "should suggest Alice");
        assert!(!texts.contains(&"Bob"), "should not suggest Bob");
    }

    // ── Suggestion range correctness ────────────────────────────────────

    #[test]
    fn suggestion_range_for_command_name() {
        let mut d = CommandDispatcher::new();
        d.register(literal("help").executes(|_| Ok(1)));
        let src = make_source(4);
        let completions = d.get_completions("he", &src, &[]);
        assert_eq!(completions.len(), 1);
        assert_eq!(completions[0].text, "help");
        assert_eq!(completions[0].range.start, 0);
        assert_eq!(completions[0].range.end, 2);
    }

    #[test]
    fn suggestion_range_for_first_argument() {
        let mut d = CommandDispatcher::new();
        d.register(
            literal("kick").then(
                argument(
                    "target",
                    ArgumentType::Entity {
                        single: true,
                        player_only: true,
                    },
                )
                .executes(|_| Ok(1)),
            ),
        );
        let src = make_source(4);
        let names = vec!["Alice".to_string()];
        let completions = d.get_completions("kick Al", &src, &names);
        assert_eq!(completions.len(), 1);
        assert_eq!(completions[0].text, "Alice");
        assert_eq!(completions[0].range.start, 5);
        assert_eq!(completions[0].range.end, 7);
    }

    #[test]
    fn suggestion_range_for_second_argument() {
        let mut d = CommandDispatcher::new();
        d.register(
            literal("give").then(
                argument(
                    "target",
                    ArgumentType::Entity {
                        single: true,
                        player_only: true,
                    },
                )
                .then(argument("item", ArgumentType::ItemStack).executes(|_| Ok(1))),
            ),
        );
        let src = make_source(4);
        let completions = d.get_completions("give Alice sto", &src, &[]);
        assert!(!completions.is_empty());
        assert_eq!(completions[0].range.start, 11);
        assert_eq!(completions[0].range.end, 14);
    }

    #[test]
    fn suggestion_range_for_empty_argument() {
        let mut d = CommandDispatcher::new();
        d.register(
            literal("kick").then(
                argument(
                    "target",
                    ArgumentType::Entity {
                        single: true,
                        player_only: true,
                    },
                )
                .executes(|_| Ok(1)),
            ),
        );
        let src = make_source(4);
        let names = vec!["Alice".to_string()];
        let completions = d.get_completions("kick ", &src, &names);
        assert_eq!(completions.len(), 7);
        let texts: Vec<&str> = completions.iter().map(|c| c.text.as_str()).collect();
        assert!(texts.contains(&"@a"));
        assert!(texts.contains(&"@e"));
        assert!(texts.contains(&"@p"));
        assert!(texts.contains(&"@r"));
        assert!(texts.contains(&"@s"));
        assert!(texts.contains(&"@n"));
        assert!(texts.contains(&"Alice"));
        assert_eq!(completions[0].range.start, 5);
        assert_eq!(completions[0].range.end, 5);
    }

    #[test]
    fn suggestion_range_for_subcommand_literal() {
        let mut d = CommandDispatcher::new();
        d.register(
            literal("time")
                .then(literal("set").executes(|_| Ok(1)))
                .then(literal("query").executes(|_| Ok(2))),
        );
        let src = make_source(4);
        let completions = d.get_completions("time s", &src, &[]);
        assert_eq!(completions.len(), 1);
        assert_eq!(completions[0].text, "set");
        assert_eq!(completions[0].range.start, 5);
        assert_eq!(completions[0].range.end, 6);
    }

    // ── Serializer: Entity args get ask_server suggestions ──────────────

    #[test]
    fn serialize_entity_arg_has_ask_server_suggestion() {
        let mut d = CommandDispatcher::new();
        d.register(
            literal("kick").then(
                argument(
                    "target",
                    ArgumentType::Entity {
                        single: true,
                        player_only: true,
                    },
                )
                .executes(|_| Ok(1)),
            ),
        );
        let src = make_source(4);
        let tree = d.serialize_tree(&src);
        let target_node = &tree.nodes[2];
        assert_eq!(
            target_node.suggestions_type.as_deref(),
            Some("minecraft:ask_server"),
        );
        assert_ne!(target_node.flags & 0x10, 0);
    }
}
