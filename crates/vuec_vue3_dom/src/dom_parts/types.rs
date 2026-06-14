/// Options for the Vue 3 DOM compiler facade.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DomCompilerOptions {
    /// Shared Vue 3 compiler-core options.
    pub core: Vue3CompilerOptions,
    /// Tag names treated as custom elements by the DOM parser facade.
    pub is_custom_element: Vec<String>,
    /// Whether static asset URL attributes should be transformed.
    pub transform_asset_urls: bool,
    /// Asset URL transform options.
    pub asset_url_options: AssetUrlOptions,
    /// Whether basic HTML entities in text nodes should be decoded.
    pub decode_entities: bool,
}

impl Default for DomCompilerOptions {
    fn default() -> Self {
        let mut core = Vue3CompilerOptions::default();
        apply_dom_parser_defaults(&mut core);
        core.dom_namespaces = true;
        Self {
            core,
            is_custom_element: Vec::new(),
            transform_asset_urls: true,
            asset_url_options: AssetUrlOptions::default(),
            decode_entities: true,
        }
    }
}

/// Applies Vue 3 DOM parser defaults to compiler-core options.
pub fn apply_dom_parser_defaults(core: &mut Vue3CompilerOptions) {
    if core.void_tags.is_empty() {
        core.void_tags = DOM_VOID_TAGS.iter().map(|tag| (*tag).to_string()).collect();
    }
    if core.native_tags.is_none() {
        core.native_tags = Some(
            DOM_HTML_TAGS
                .iter()
                .chain(DOM_SVG_TAGS.iter())
                .chain(DOM_MATH_TAGS.iter())
                .map(|tag| (*tag).to_string())
                .collect(),
        );
    }
    if core.pre_tags.is_empty() {
        core.pre_tags = vec!["pre".into()];
    }
    if core.ignore_newline_tags.is_empty() {
        core.ignore_newline_tags = vec!["pre".into(), "textarea".into()];
    }
    core.dom_namespaces = true;
    if core.built_in_components.is_empty() {
        core.built_in_components = vec![
            "Transition".into(),
            "transition".into(),
            "TransitionGroup".into(),
            "transition-group".into(),
        ];
    }
}

const DOM_VOID_TAGS: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];

const DOM_HTML_TAGS: &[&str] = &[
    "html",
    "body",
    "base",
    "head",
    "link",
    "meta",
    "style",
    "title",
    "address",
    "article",
    "aside",
    "footer",
    "header",
    "hgroup",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "nav",
    "section",
    "div",
    "dd",
    "dl",
    "dt",
    "figcaption",
    "figure",
    "picture",
    "hr",
    "img",
    "li",
    "main",
    "ol",
    "p",
    "pre",
    "ul",
    "a",
    "b",
    "abbr",
    "bdi",
    "bdo",
    "br",
    "cite",
    "code",
    "data",
    "dfn",
    "em",
    "i",
    "kbd",
    "mark",
    "q",
    "rp",
    "rt",
    "ruby",
    "s",
    "samp",
    "small",
    "span",
    "strong",
    "sub",
    "sup",
    "time",
    "u",
    "var",
    "wbr",
    "area",
    "audio",
    "map",
    "track",
    "video",
    "embed",
    "object",
    "param",
    "source",
    "canvas",
    "script",
    "noscript",
    "del",
    "ins",
    "caption",
    "col",
    "colgroup",
    "table",
    "thead",
    "tbody",
    "td",
    "th",
    "tr",
    "button",
    "datalist",
    "fieldset",
    "form",
    "input",
    "label",
    "legend",
    "meter",
    "optgroup",
    "option",
    "output",
    "progress",
    "select",
    "textarea",
    "details",
    "dialog",
    "menu",
    "summary",
    "template",
    "blockquote",
    "iframe",
    "tfoot",
];

const DOM_SVG_TAGS: &[&str] = &[
    "svg",
    "animate",
    "animateMotion",
    "animateTransform",
    "circle",
    "clipPath",
    "color-profile",
    "defs",
    "desc",
    "discard",
    "ellipse",
    "feBlend",
    "feColorMatrix",
    "feComponentTransfer",
    "feComposite",
    "feConvolveMatrix",
    "feDiffuseLighting",
    "feDisplacementMap",
    "feDistantLight",
    "feDropShadow",
    "feFlood",
    "feFuncA",
    "feFuncB",
    "feFuncG",
    "feFuncR",
    "feGaussianBlur",
    "feImage",
    "feMerge",
    "feMergeNode",
    "feMorphology",
    "feOffset",
    "fePointLight",
    "feSpecularLighting",
    "feSpotLight",
    "feTile",
    "feTurbulence",
    "filter",
    "foreignObject",
    "g",
    "hatch",
    "hatchpath",
    "image",
    "line",
    "linearGradient",
    "marker",
    "mask",
    "mesh",
    "meshgradient",
    "meshpatch",
    "meshrow",
    "metadata",
    "mpath",
    "path",
    "pattern",
    "polygon",
    "polyline",
    "radialGradient",
    "rect",
    "set",
    "solidcolor",
    "stop",
    "switch",
    "symbol",
    "text",
    "textPath",
    "title",
    "tspan",
    "unknown",
    "use",
    "view",
];

const DOM_MATH_TAGS: &[&str] = &[
    "annotation",
    "annotation-xml",
    "maction",
    "maligngroup",
    "malignmark",
    "math",
    "menclose",
    "merror",
    "mfenced",
    "mfrac",
    "mfraction",
    "mglyph",
    "mi",
    "mlabeledtr",
    "mlongdiv",
    "mmultiscripts",
    "mn",
    "mo",
    "mover",
    "mpadded",
    "mphantom",
    "mprescripts",
    "mroot",
    "mrow",
    "ms",
    "mscarries",
    "mscarry",
    "msgroup",
    "msline",
    "mspace",
    "msqrt",
    "msrow",
    "mstack",
    "mstyle",
    "msub",
    "msubsup",
    "msup",
    "mtable",
    "mtd",
    "mtext",
    "mtr",
    "munder",
    "munderover",
    "none",
    "semantics",
];

/// DOM directive summary used by compatibility reports and probes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DomDirective {
    /// Directive name without `v-`.
    pub name: String,
    /// Static directive argument.
    pub argument: Option<String>,
    /// Directive modifiers.
    pub modifiers: Vec<String>,
    /// Directive expression source.
    pub expression: Option<String>,
}

/// Incremental Vue 3 DOM compiler facade.
pub struct DomCompiler {
    ast_cache: BTreeMap<DomAstCacheKey, Vue3Ast>,
    cache_stats: DomAstCacheStats,
}

/// DOM AST cache counters.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DomAstCacheStats {
    /// Number of parsed AST cache hits.
    pub ast_hits: u64,
    /// Number of parsed AST cache misses.
    pub ast_misses: u64,
    /// Number of stale cache entries invalidated.
    pub ast_invalidations: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct DomAstCacheKey {
    filename: String,
    source_hash: u64,
    file_id: u32,
    base_offset: usize,
    parse_options: DomAstCacheOptions,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct DomAstCacheOptions {
    comments: bool,
    delimiters: Option<[String; 2]>,
    void_tags: Vec<String>,
    native_tags: Option<Vec<String>>,
    custom_elements: Vec<String>,
    built_in_components: Vec<String>,
    namespaces: Vec<(String, u8)>,
    root_namespace: u8,
    dom_namespaces: bool,
    whitespace: String,
    pre_tags: Vec<String>,
    ignore_newline_tags: Vec<String>,
    sfc_parse_mode: bool,
    sfc_plain_template_langs: Vec<String>,
    decode_entities: bool,
    is_custom_element: Vec<String>,
}

impl DomCompiler {
    /// Creates an empty DOM compiler with no cached ASTs.
    pub fn new() -> Self {
        Self {
            ast_cache: BTreeMap::new(),
            cache_stats: DomAstCacheStats::default(),
        }
    }

    /// Parses a template through the incremental DOM AST cache.
    pub fn parse(&mut self, source: TemplateSource, options: &DomCompilerOptions) -> Vue3Ast {
        let key = DomAstCacheKey::new(&source, options);
        if let Some(ast) = self.ast_cache.get(&key) {
            self.cache_stats.ast_hits += 1;
            return ast.clone();
        }
        self.invalidate_stale_ast_entries(&key);
        self.cache_stats.ast_misses += 1;
        let ast = parse(source, options);
        self.ast_cache.insert(key, ast.clone());
        ast
    }

    /// Compiles a template through the incremental DOM AST cache.
    pub fn compile(
        &mut self,
        source: TemplateSource,
        options: DomCompilerOptions,
    ) -> CodegenResult {
        let ast = self.parse(source.clone(), &options);
        compile_parsed_ast(source, options, ast)
    }

    /// Returns current cache counters.
    pub fn cache_stats(&self) -> DomAstCacheStats {
        self.cache_stats.clone()
    }

    /// Returns the number of cached parsed AST entries.
    pub fn ast_cache_len(&self) -> usize {
        self.ast_cache.len()
    }

    fn invalidate_stale_ast_entries(&mut self, key: &DomAstCacheKey) {
        let before = self.ast_cache.len();
        self.ast_cache.retain(|existing, _| {
            existing.filename != key.filename
                || existing.file_id != key.file_id
                || existing.base_offset != key.base_offset
                || existing.parse_options != key.parse_options
        });
        let removed = before.saturating_sub(self.ast_cache.len());
        self.cache_stats.ast_invalidations += removed as u64;
    }
}

impl Default for DomCompiler {
    fn default() -> Self {
        Self::new()
    }
}

impl DomAstCacheKey {
    fn new(source: &TemplateSource, options: &DomCompilerOptions) -> Self {
        Self {
            filename: source.filename.clone(),
            source_hash: source_hash(&source.source),
            file_id: source.file_id.0,
            base_offset: source.base_offset,
            parse_options: DomAstCacheOptions::new(options),
        }
    }
}

impl DomAstCacheOptions {
    fn new(options: &DomCompilerOptions) -> Self {
        Self {
            comments: options.core.comments,
            delimiters: options.core.delimiters.clone(),
            void_tags: options.core.void_tags.clone(),
            native_tags: options.core.native_tags.clone(),
            custom_elements: options.core.custom_elements.clone(),
            built_in_components: options.core.built_in_components.clone(),
            namespaces: options
                .core
                .namespaces
                .iter()
                .map(|(tag, namespace)| (tag.clone(), namespace_key(*namespace)))
                .collect(),
            root_namespace: namespace_key(options.core.root_namespace),
            dom_namespaces: options.core.dom_namespaces,
            whitespace: options.core.whitespace.clone(),
            pre_tags: options.core.pre_tags.clone(),
            ignore_newline_tags: options.core.ignore_newline_tags.clone(),
            sfc_parse_mode: options.core.sfc_parse_mode,
            sfc_plain_template_langs: options.core.sfc_plain_template_langs.clone(),
            decode_entities: options.decode_entities,
            is_custom_element: options.is_custom_element.clone(),
        }
    }
}

fn source_hash(source: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    source.hash(&mut hasher);
    hasher.finish()
}

fn namespace_key(namespace: HtmlNamespace) -> u8 {
    match namespace {
        HtmlNamespace::Html => 0,
        HtmlNamespace::Svg => 1,
        HtmlNamespace::MathMl => 2,
    }
}
