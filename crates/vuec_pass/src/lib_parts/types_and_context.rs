/// Stable ordering key for transform passes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PassOrder(i32);

impl PassOrder {
    /// Early pass slot used for parser-adjacent normalization.
    pub const EARLY: Self = Self(-1000);
    /// Default pass slot.
    pub const DEFAULT: Self = Self(0);
    /// Late pass slot used for cleanup or finalization.
    pub const LATE: Self = Self(1000);

    /// Creates an explicit ordering key.
    pub const fn new(value: i32) -> Self {
        Self(value)
    }

    /// Returns the numeric ordering key.
    pub const fn value(self) -> i32 {
        self.0
    }
}

impl From<i32> for PassOrder {
    fn from(value: i32) -> Self {
        Self::new(value)
    }
}

/// A transform scope category.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ScopeKind {
    /// Root transform scope.
    Root,
    /// Template expression scope.
    Template,
    /// Function-like generated scope.
    Function,
    /// `v-for` alias scope.
    VFor,
    /// Slot parameter scope.
    Slot,
    /// Dialect-specific scope category.
    Custom(String),
}

/// A single lexical or template scope frame.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopeFrame {
    /// Scope category.
    pub kind: ScopeKind,
    /// Bindings declared in this frame.
    pub bindings: BTreeSet<String>,
}

impl ScopeFrame {
    /// Creates an empty scope frame.
    pub fn new(kind: ScopeKind) -> Self {
        Self {
            kind,
            bindings: BTreeSet::new(),
        }
    }

    /// Adds a binding to this scope and returns whether it was newly inserted.
    pub fn add_binding(&mut self, binding: impl Into<String>) -> bool {
        self.bindings.insert(binding.into())
    }
}

/// Shared mutable state available to transform passes.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransformContext {
    /// Runtime helpers requested by transform passes.
    pub helpers: BTreeSet<RuntimeHelper>,
    /// Current lexical or template scope nesting depth.
    pub scope_depth: usize,
    /// Lexical and template scope frames from outermost to innermost.
    #[serde(default)]
    pub scopes: Vec<ScopeFrame>,
    #[serde(skip)]
    /// Diagnostics collected while running transforms.
    pub diagnostics: DiagnosticSink,
}

impl TransformContext {
    /// Records a diagnostic emitted by a transform pass.
    pub fn report(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
    }

    /// Adds a runtime helper requirement and returns whether it was newly added.
    pub fn add_helper(&mut self, helper: RuntimeHelper) -> bool {
        self.helpers.insert(helper)
    }

    /// Removes a runtime helper requirement and returns whether it existed.
    pub fn remove_helper(&mut self, helper: RuntimeHelper) -> bool {
        self.helpers.remove(&helper)
    }

    /// Returns whether a runtime helper has been requested.
    pub fn has_helper(&self, helper: RuntimeHelper) -> bool {
        self.helpers.contains(&helper)
    }

    /// Pushes a scope frame and updates the compatibility depth counter.
    pub fn enter_scope(&mut self, kind: ScopeKind) {
        self.scopes.push(ScopeFrame::new(kind));
        self.sync_scope_depth();
    }

    /// Pops the innermost scope frame.
    pub fn exit_scope(&mut self) -> Option<ScopeFrame> {
        let frame = self.scopes.pop();
        self.sync_scope_depth();
        frame
    }

    /// Returns the innermost scope frame.
    pub fn current_scope(&self) -> Option<&ScopeFrame> {
        self.scopes.last()
    }

    /// Returns the innermost mutable scope frame.
    pub fn current_scope_mut(&mut self) -> Option<&mut ScopeFrame> {
        self.scopes.last_mut()
    }

    /// Adds a binding to the innermost scope.
    pub fn add_scope_binding(&mut self, binding: impl Into<String>) -> bool {
        match self.current_scope_mut() {
            Some(scope) => scope.add_binding(binding),
            None => false,
        }
    }

    /// Returns whether a binding exists in any active scope.
    pub fn is_binding_in_scope(&self, binding: &str) -> bool {
        self.scopes
            .iter()
            .rev()
            .any(|scope| scope.bindings.contains(binding))
    }

    /// Returns collected diagnostics without consuming the context.
    pub fn diagnostics(&self) -> &[Diagnostic] {
        self.diagnostics.as_slice()
    }

    fn sync_scope_depth(&mut self) {
        self.scope_depth = self.scopes.len();
    }
}
