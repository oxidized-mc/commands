//! Command node types forming the Brigadier command graph.

use crate::arguments::ArgumentType;
use crate::context::CommandContext;
use std::collections::BTreeMap;
use std::sync::Arc;

/// Function invoked when a command node is executed.
pub type CommandFn<S> =
    Arc<dyn Fn(&CommandContext<S>) -> Result<i32, crate::CommandError> + Send + Sync>;

/// Predicate that determines whether a source can see/use a node.
pub type RequirementFn<S> = Arc<dyn Fn(&S) -> bool + Send + Sync>;

/// A node in the command graph.
pub enum CommandNode<S> {
    /// The invisible root node that holds all top-level commands.
    Root(RootCommandNode<S>),
    /// A literal keyword node (e.g., `tp`, `set`, `day`).
    Literal(LiteralCommandNode<S>),
    /// A typed argument node (e.g., `<target>`, `<count>`).
    Argument(ArgumentCommandNode<S>),
}

impl<S> Clone for CommandNode<S> {
    fn clone(&self) -> Self {
        match self {
            Self::Root(n) => Self::Root(n.clone()),
            Self::Literal(n) => Self::Literal(n.clone()),
            Self::Argument(n) => Self::Argument(n.clone()),
        }
    }
}

impl<S> CommandNode<S> {
    /// Returns the name used for child lookups.
    pub fn name(&self) -> &str {
        match self {
            Self::Root(_) => "",
            Self::Literal(n) => &n.literal,
            Self::Argument(n) => &n.name,
        }
    }

    /// Returns a reference to the children map.
    pub fn children(&self) -> &BTreeMap<String, CommandNode<S>> {
        match self {
            Self::Root(n) => &n.children,
            Self::Literal(n) => &n.children,
            Self::Argument(n) => &n.children,
        }
    }

    /// Returns a mutable reference to the children map.
    pub fn children_mut(&mut self) -> &mut BTreeMap<String, CommandNode<S>> {
        match self {
            Self::Root(n) => &mut n.children,
            Self::Literal(n) => &mut n.children,
            Self::Argument(n) => &mut n.children,
        }
    }

    /// Returns the command function if this node is executable.
    pub fn command(&self) -> Option<&CommandFn<S>> {
        match self {
            Self::Root(_) => None,
            Self::Literal(n) => n.command.as_ref(),
            Self::Argument(n) => n.command.as_ref(),
        }
    }

    /// Returns the requirement predicate.
    pub fn requirement(&self) -> Option<&RequirementFn<S>> {
        match self {
            Self::Root(_) => None,
            Self::Literal(n) => n.requirement.as_ref(),
            Self::Argument(n) => n.requirement.as_ref(),
        }
    }

    /// Returns the redirect target if set.
    pub fn redirect(&self) -> Option<&Arc<CommandNode<S>>> {
        match self {
            Self::Root(_) => None,
            Self::Literal(n) => n.redirect.as_ref(),
            Self::Argument(n) => n.redirect.as_ref(),
        }
    }

    /// Returns the optional human-readable description.
    pub fn description(&self) -> Option<&str> {
        match self {
            Self::Root(_) => None,
            Self::Literal(n) => n.description.as_deref(),
            Self::Argument(n) => n.description.as_deref(),
        }
    }

    /// Returns `true` if this source passes the requirement check.
    pub fn can_use(&self, source: &S) -> bool {
        match self.requirement() {
            Some(req) => (req)(source),
            None => true,
        }
    }

    /// Adds a child node.
    pub fn add_child(&mut self, node: CommandNode<S>) {
        let name = node.name().to_string();
        self.children_mut().insert(name, node);
    }
}

/// The root node of the command graph.
pub struct RootCommandNode<S> {
    /// Child nodes keyed by name.
    pub children: BTreeMap<String, CommandNode<S>>,
    _phantom: std::marker::PhantomData<S>,
}

impl<S> Clone for RootCommandNode<S> {
    fn clone(&self) -> Self {
        Self {
            children: self.children.clone(),
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<S> RootCommandNode<S> {
    /// Creates a new empty root node.
    pub fn new() -> Self {
        Self {
            children: BTreeMap::new(),
            _phantom: std::marker::PhantomData,
        }
    }

    /// Adds a child node, returning a mutable reference to it.
    pub fn add_child(&mut self, node: CommandNode<S>) -> &mut CommandNode<S> {
        let name = node.name().to_string();
        self.children.entry(name).or_insert(node)
    }

    /// Gets a child node by name.
    pub fn get_child(&self, name: &str) -> Option<&CommandNode<S>> {
        self.children.get(name)
    }
}

impl<S> Default for RootCommandNode<S> {
    fn default() -> Self {
        Self::new()
    }
}

/// A literal command node matches an exact keyword.
pub struct LiteralCommandNode<S> {
    /// The literal keyword.
    pub literal: String,
    /// Child nodes keyed by name.
    pub children: BTreeMap<String, CommandNode<S>>,
    /// Optional redirect target (for aliases like `/teleport` → `/tp`).
    pub redirect: Option<Arc<CommandNode<S>>>,
    /// The function executed when this node is terminal.
    pub command: Option<CommandFn<S>>,
    /// Permission check.
    pub requirement: Option<RequirementFn<S>>,
    /// Optional human-readable description (e.g. for `/help` output).
    pub description: Option<String>,
}

impl<S> Clone for LiteralCommandNode<S> {
    fn clone(&self) -> Self {
        Self {
            literal: self.literal.clone(),
            children: self.children.clone(),
            redirect: self.redirect.clone(),
            command: self.command.clone(),
            requirement: self.requirement.clone(),
            description: self.description.clone(),
        }
    }
}

/// An argument command node parses a typed value.
pub struct ArgumentCommandNode<S> {
    /// The argument name (used for retrieval in context).
    pub name: String,
    /// The argument type for parsing and wire serialization.
    pub argument_type: ArgumentType,
    /// Child nodes keyed by name.
    pub children: BTreeMap<String, CommandNode<S>>,
    /// Optional redirect target.
    pub redirect: Option<Arc<CommandNode<S>>>,
    /// The function executed when this node is terminal.
    pub command: Option<CommandFn<S>>,
    /// Permission check.
    pub requirement: Option<RequirementFn<S>>,
    /// Custom suggestion provider identifier.
    pub suggestions_type: Option<String>,
    /// Optional human-readable description (e.g. for `/help` output).
    pub description: Option<String>,
}

impl<S> Clone for ArgumentCommandNode<S> {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            argument_type: self.argument_type.clone(),
            children: self.children.clone(),
            redirect: self.redirect.clone(),
            command: self.command.clone(),
            requirement: self.requirement.clone(),
            suggestions_type: self.suggestions_type.clone(),
            description: self.description.clone(),
        }
    }
}

// ── Builder DSL ───────────────────────────────────────────────────────

/// Creates a literal node builder.
pub fn literal<S: Clone + Send + Sync + 'static>(name: &str) -> LiteralBuilder<S> {
    LiteralBuilder {
        literal: name.to_string(),
        children: Vec::new(),
        command: None,
        requirement: None,
        redirect: None,
        description: None,
    }
}

/// Creates an argument node builder.
pub fn argument<S: Clone + Send + Sync + 'static>(
    name: &str,
    arg_type: ArgumentType,
) -> ArgumentBuilder<S> {
    ArgumentBuilder {
        name: name.to_string(),
        argument_type: arg_type,
        children: Vec::new(),
        command: None,
        requirement: None,
        redirect: None,
        suggestions_type: None,
        description: None,
    }
}

/// Builder for literal nodes.
pub struct LiteralBuilder<S> {
    literal: String,
    children: Vec<CommandNode<S>>,
    command: Option<CommandFn<S>>,
    requirement: Option<RequirementFn<S>>,
    redirect: Option<Arc<CommandNode<S>>>,
    description: Option<String>,
}

impl<S: Clone + Send + Sync + 'static> LiteralBuilder<S> {
    /// Adds a child node.
    pub fn then(mut self, child: impl Into<CommandNode<S>>) -> Self {
        self.children.push(child.into());
        self
    }

    /// Sets the execution function.
    pub fn executes<F>(mut self, f: F) -> Self
    where
        F: Fn(&CommandContext<S>) -> Result<i32, crate::CommandError> + Send + Sync + 'static,
    {
        self.command = Some(Arc::new(f));
        self
    }

    /// Sets the permission requirement.
    pub fn requires<F>(mut self, f: F) -> Self
    where
        F: Fn(&S) -> bool + Send + Sync + 'static,
    {
        self.requirement = Some(Arc::new(f));
        self
    }

    /// Sets a redirect target (for aliases).
    pub fn redirect(mut self, target: Arc<CommandNode<S>>) -> Self {
        self.redirect = Some(target);
        self
    }

    /// Sets an optional human-readable description.
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Builds the literal node.
    pub fn build(self) -> CommandNode<S> {
        let mut node = LiteralCommandNode {
            literal: self.literal,
            children: BTreeMap::new(),
            redirect: self.redirect,
            command: self.command,
            requirement: self.requirement,
            description: self.description,
        };
        for child in self.children {
            let name = child.name().to_string();
            node.children.insert(name, child);
        }
        CommandNode::Literal(node)
    }
}

impl<S: Clone + Send + Sync + 'static> From<LiteralBuilder<S>> for CommandNode<S> {
    fn from(builder: LiteralBuilder<S>) -> Self {
        builder.build()
    }
}

/// Builder for argument nodes.
pub struct ArgumentBuilder<S> {
    name: String,
    argument_type: ArgumentType,
    children: Vec<CommandNode<S>>,
    command: Option<CommandFn<S>>,
    requirement: Option<RequirementFn<S>>,
    redirect: Option<Arc<CommandNode<S>>>,
    suggestions_type: Option<String>,
    description: Option<String>,
}

impl<S: Clone + Send + Sync + 'static> ArgumentBuilder<S> {
    /// Adds a child node.
    pub fn then(mut self, child: impl Into<CommandNode<S>>) -> Self {
        self.children.push(child.into());
        self
    }

    /// Sets the execution function.
    pub fn executes<F>(mut self, f: F) -> Self
    where
        F: Fn(&CommandContext<S>) -> Result<i32, crate::CommandError> + Send + Sync + 'static,
    {
        self.command = Some(Arc::new(f));
        self
    }

    /// Sets the permission requirement.
    pub fn requires<F>(mut self, f: F) -> Self
    where
        F: Fn(&S) -> bool + Send + Sync + 'static,
    {
        self.requirement = Some(Arc::new(f));
        self
    }

    /// Sets a redirect target.
    pub fn redirect(mut self, target: Arc<CommandNode<S>>) -> Self {
        self.redirect = Some(target);
        self
    }

    /// Sets the custom suggestion provider identifier.
    pub fn suggests(mut self, suggestion_type: &str) -> Self {
        self.suggestions_type = Some(suggestion_type.to_string());
        self
    }

    /// Sets an optional human-readable description.
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Builds the argument node.
    pub fn build(self) -> CommandNode<S> {
        let mut node = ArgumentCommandNode {
            name: self.name,
            argument_type: self.argument_type,
            children: BTreeMap::new(),
            redirect: self.redirect,
            command: self.command,
            requirement: self.requirement,
            suggestions_type: self.suggestions_type,
            description: self.description,
        };
        for child in self.children {
            let name = child.name().to_string();
            node.children.insert(name, child);
        }
        CommandNode::Argument(node)
    }
}

impl<S: Clone + Send + Sync + 'static> From<ArgumentBuilder<S>> for CommandNode<S> {
    fn from(builder: ArgumentBuilder<S>) -> Self {
        builder.build()
    }
}
