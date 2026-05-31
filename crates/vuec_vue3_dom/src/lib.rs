//! Vue 3 DOM compiler facade and DOM-specific template normalization.
//!
//! This crate wraps `vuec_vue3_core` with browser DOM defaults, asset URL
//! handling, directive summaries, entity decoding, and an incremental parsed AST
//! cache. It does not own the canonical AST/HIR/MIR schema.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

use std::collections::hash_map::DefaultHasher;
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use vuec_ast::{
    HtmlNamespace, NodeId, RuntimeHelper, TemplateAttribute, Vue3Ast, Vue3AstKind, Vue3Element,
    Vue3ElementType, Vue3ImportItem, Vue3Prop, Vue3Root,
};
use vuec_diagnostics::{Diagnostic, Vue3ErrorCode};
use vuec_pass::TransformContext;
use vuec_vue3_asset::transform_asset_url_props;
/// Asset URL transform options re-exported for DOM compiler callers.
pub use vuec_vue3_asset::AssetUrlOptions;
use vuec_vue3_core::{CodegenResult, TemplateSource, Vue3CompilerOptions, Vue3Dialect};

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

/// Parses a Vue 3 template with DOM parser defaults and DOM normalization.
pub fn parse(source: TemplateSource, options: &DomCompilerOptions) -> Vue3Ast {
    let mut ast = Vue3Dialect::base_parse(source, &options.core);
    normalize_dom_ast(&mut ast, options);
    ast
}

/// Compiles a Vue 3 template for DOM render output.
pub fn compile(source: TemplateSource, options: DomCompilerOptions) -> CodegenResult {
    let ast = parse(source.clone(), &options);
    compile_parsed_ast(source, options, ast)
}

fn compile_parsed_ast(
    source: TemplateSource,
    options: DomCompilerOptions,
    mut ast: Vue3Ast,
) -> CodegenResult {
    let mut ctx = TransformContext::default();
    remove_side_effect_nodes(&mut ast, &mut ctx);
    report_transition_invalid_children(&ast, &mut ctx);
    transform_transition_children(&mut ast, &mut ctx);
    report_invalid_native_v_model(&ast, &options.core, &mut ctx);
    let mut asset_imports = Vec::<Vue3ImportItem>::new();
    for node_index in 0..ast.nodes.len() {
        if let Vue3AstKind::Element(element) = &mut ast.nodes[node_index].kind {
            let tag = element.tag.clone();
            if options.transform_asset_urls {
                transform_asset_url_props(
                    &tag,
                    &mut element.props,
                    &options.asset_url_options,
                    options.core.mode == "module",
                    &mut asset_imports,
                );
            }
        }
    }
    if !asset_imports.is_empty() {
        if let Some(root) = vue3_dom_root_mut(&mut ast) {
            root.imports = asset_imports;
        }
    }
    let dom_summary = dom_directive_summary(&ast);
    Vue3Dialect::transform(&mut ast, &mut ctx, &options.core);
    let mut result = Vue3Dialect::finish_compile(ast, source, options.core, ctx);
    result.ast_summary = if dom_summary.is_empty() {
        format!("dom:{}", result.ast_summary)
    } else {
        format!("dom:{};{}", result.ast_summary, dom_summary.join("|"))
    };
    result
}

fn dom_directive_summary(ast: &Vue3Ast) -> Vec<String> {
    ast.nodes
        .iter()
        .filter_map(|node| {
            let Vue3AstKind::Element(element) = &node.kind else {
                return None;
            };
            let summaries = element
                .template_attributes()
                .iter()
                .filter_map(|attr| {
                    parse_directive(attr).map(|directive| match directive.name.as_str() {
                        "html" => "v-html".to_string(),
                        "text" => "v-text".to_string(),
                        "show" => "v-show".to_string(),
                        "model" => {
                            format!("v-model:{}", model_runtime_helper(&element.tag, &directive))
                        }
                        "on" => format!("v-on:{}", directive.modifiers.join(".")),
                        "bind" => format!("v-bind:{}", directive.modifiers.join(".")),
                        _ => format!("v-{}", directive.name),
                    })
                })
                .collect::<Vec<_>>();
            (!summaries.is_empty()).then(|| summaries.join(","))
        })
        .collect()
}

fn transform_transition_children(ast: &mut Vue3Ast, ctx: &mut TransformContext) {
    let transition_ids = ast
        .nodes
        .iter()
        .filter_map(|node| match &node.kind {
            Vue3AstKind::Element(element)
                if element.tag_type == Vue3ElementType::Component
                    && matches!(element.tag.as_str(), "Transition" | "transition") =>
            {
                Some(node.id)
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    for node_id in transition_ids {
        let had_ignored_comments = ast.node(node_id).is_some_and(|node| {
            node.children.iter().any(|child_id| {
                ast.node(*child_id)
                    .is_some_and(|child| matches!(child.kind, Vue3AstKind::Comment(_)))
            })
        });
        let visible_children = ast
            .node(node_id)
            .map(|node| transition_visible_child_ids(ast, &node.children))
            .unwrap_or_default();
        if let Some(node) = ast.node_mut(node_id) {
            node.children = visible_children.clone();
        }
        if had_ignored_comments {
            ctx.add_helper(RuntimeHelper::Vue3CreateCommentVNode);
        }
        if transition_single_child_has_v_show(ast, &visible_children) {
            if let Some(node) = ast.node_mut(node_id) {
                if let Vue3AstKind::Element(element) = &mut node.kind {
                    if !element.props.iter().any(|prop| {
                        matches!(prop, Vue3Prop::Attribute(attr) if attr.name == "persisted")
                    }) {
                        element.props.push(Vue3Prop::from(TemplateAttribute {
                            name: "persisted".into(),
                            value: None,
                        }));
                    }
                }
            }
        }
    }
}

fn transition_single_child_has_v_show(ast: &Vue3Ast, children: &[NodeId]) -> bool {
    let [child_id] = children else {
        return false;
    };
    let Some(child) = ast.node(*child_id) else {
        return false;
    };
    let Vue3AstKind::Element(element) = &child.kind else {
        return false;
    };
    element_has_directive(element, "show")
}

/// Applies DOM-specific AST normalization in-place.
pub fn normalize_dom_ast(ast: &mut Vue3Ast, options: &DomCompilerOptions) {
    for node in &mut ast.nodes {
        match &mut node.kind {
            Vue3AstKind::Text(text) if options.decode_entities => {
                text.value = decode_basic_entities(&text.value);
            }
            Vue3AstKind::Element(element) => {
                if options
                    .is_custom_element
                    .iter()
                    .any(|custom| custom == &element.tag)
                {
                    let mut attributes = element.template_attributes();
                    attributes.push(TemplateAttribute {
                        name: "data-vuec-custom-element".into(),
                        value: None,
                    });
                    element.props = attributes.into_iter().map(Vue3Prop::from).collect();
                }
            }
            _ => {}
        }
    }
}

/// Projects static `style` attributes for the public DOM `transformStyle` helper.
pub fn transform_style_projection(payload: &Value) -> Value {
    let props = payload
        .get("node")
        .and_then(|node| node.get("props"))
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let replacements = props
        .iter()
        .enumerate()
        .filter_map(|(index, prop)| {
            let is_static_style = prop.get("type").and_then(Value::as_u64) == Some(6)
                && prop.get("name").and_then(Value::as_str) == Some("style")
                && prop.get("value").is_some_and(|value| !value.is_null());
            if !is_static_style {
                return None;
            }
            let value = prop
                .get("value")
                .and_then(|value| value.get("content"))
                .and_then(Value::as_str)
                .unwrap_or("");
            Some(json!({
                "index": index,
                "expression": style_json_string(value),
            }))
        })
        .collect::<Vec<_>>();
    json!({ "replacements": replacements })
}

/// Projects the DOM `v-html` directive transform for compatibility bridge callers.
pub fn transform_v_html_projection(payload: &Value) -> Value {
    transform_dom_content_directive_projection(
        payload,
        DomContentDirectiveProjection {
            key: "innerHTML",
            key_loc: Some("dir"),
            missing_expression_code: 54,
            with_children_code: 55,
            wrap_dynamic_text: false,
        },
    )
}

/// Projects the DOM `v-text` directive transform for compatibility bridge callers.
pub fn transform_v_text_projection(payload: &Value) -> Value {
    transform_dom_content_directive_projection(
        payload,
        DomContentDirectiveProjection {
            key: "textContent",
            key_loc: None,
            missing_expression_code: 56,
            with_children_code: 57,
            wrap_dynamic_text: true,
        },
    )
}

/// Projects the DOM `v-show` directive transform for compatibility bridge callers.
pub fn transform_show_projection(payload: &Value) -> Value {
    let dir = payload.get("dir").unwrap_or(&Value::Null);
    let exp = dir.get("exp").filter(|exp| !exp.is_null());
    let errors = if exp.is_none() {
        vec![json!({
            "code": 62,
            "loc": "dir",
        })]
    } else {
        Vec::new()
    };
    json!({
        "props": [],
        "errors": errors,
        "needRuntime": "V_SHOW",
    })
}

/// Projects the DOM `v-on` directive transform for compatibility bridge callers.
pub fn transform_on_projection(payload: &Value) -> Value {
    let dir = payload.get("dir").unwrap_or(&Value::Null);
    let mut projection = vuec_vue3_core::transform_on_projection(payload);
    let modifiers = dom_directive_modifiers(dir);
    if modifiers.is_empty() {
        return projection;
    }

    let Some(first_prop) = projection
        .get("props")
        .and_then(Value::as_array)
        .and_then(|props| props.first())
        .cloned()
    else {
        return projection;
    };

    let mut key = first_prop
        .get("key")
        .cloned()
        .unwrap_or_else(|| json!({ "kind": "undefined" }));
    let mut value = first_prop
        .get("value")
        .cloned()
        .unwrap_or_else(|| json!({ "kind": "undefined" }));
    let resolved = dom_resolve_event_modifiers(&key, &modifiers);

    if resolved
        .non_key_modifiers
        .iter()
        .any(|modifier| modifier == "right")
    {
        key = dom_transform_click_projection(key, "onContextmenu");
    }
    if resolved
        .non_key_modifiers
        .iter()
        .any(|modifier| modifier == "middle")
    {
        key = dom_transform_click_projection(key, "onMouseup");
    }

    if !resolved.non_key_modifiers.is_empty() {
        value = dom_helper_call_projection(
            "V_ON_WITH_MODIFIERS",
            vec![
                value,
                json!(dom_json_string_array(&resolved.non_key_modifiers)),
            ],
        );
    }

    if !resolved.key_modifiers.is_empty()
        && (!dom_projection_is_static_expression(&key) || dom_projection_is_keyboard_event(&key))
    {
        value = dom_helper_call_projection(
            "V_ON_WITH_KEYS",
            vec![value, json!(dom_json_string_array(&resolved.key_modifiers))],
        );
    }

    if !resolved.event_option_modifiers.is_empty() {
        let postfix = resolved
            .event_option_modifiers
            .iter()
            .map(|modifier| dom_capitalize(modifier))
            .collect::<String>();
        key = dom_event_option_key_projection(key, &postfix);
    }

    let mut prop = first_prop;
    prop["key"] = key;
    prop["value"] = value;
    projection["props"] = json!([prop]);
    projection
}

/// Projects the DOM `v-model` directive transform for compatibility bridge callers.
pub fn transform_model_projection(payload: &Value) -> Value {
    let dir = payload.get("dir").unwrap_or(&Value::Null);
    let node = payload.get("node").unwrap_or(&Value::Null);
    let context = payload.get("context").unwrap_or(&Value::Null);
    let mut projection = vuec_vue3_core::transform_model_projection(payload);
    if projection
        .get("props")
        .and_then(Value::as_array)
        .is_none_or(Vec::is_empty)
        || json_u64(node, "tagType") == Some(1)
    {
        return dom_normalize_core_model_projection(projection, dir);
    }

    let mut errors = projection
        .get("errors")
        .and_then(Value::as_array)
        .map(|errors| {
            errors
                .iter()
                .filter_map(|error| error.as_u64().map(|code| dom_core_model_error(code, dir)))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    if let Some(arg) = dir.get("arg").filter(|arg| !arg.is_null()) {
        errors.push(json!({
            "code": 59,
            "loc": arg.get("loc").cloned().unwrap_or_else(|| dir.get("loc").cloned().unwrap_or(Value::Null)),
        }));
    }

    let mut need_runtime = None::<&'static str>;
    let tag = json_str(node, "tag").unwrap_or("");
    let is_custom_element = json_bool(context, "isCustomElement");
    if matches!(tag, "input" | "textarea" | "select") || is_custom_element {
        let mut helper = "V_MODEL_TEXT";
        let mut invalid_type = false;
        if tag == "input" || is_custom_element {
            match dom_model_input_type(node) {
                DomModelInputType::Dynamic => helper = "V_MODEL_DYNAMIC",
                DomModelInputType::Static("radio") => helper = "V_MODEL_RADIO",
                DomModelInputType::Static("checkbox") => helper = "V_MODEL_CHECKBOX",
                DomModelInputType::Static("file") => {
                    invalid_type = true;
                    errors.push(json!({
                        "code": 60,
                        "loc": dir.get("loc").cloned().unwrap_or(Value::Null),
                    }));
                }
                DomModelInputType::PresentWithoutValue => {}
                DomModelInputType::Static(_) | DomModelInputType::None => {
                    if let Some(value_loc) = dom_model_dynamic_value_binding_loc(node) {
                        errors.push(json!({
                            "code": 61,
                            "loc": value_loc,
                        }));
                    }
                }
            }
        } else if tag == "select" {
            helper = "V_MODEL_SELECT";
        } else if let Some(value_loc) = dom_model_dynamic_value_binding_loc(node) {
            errors.push(json!({
                "code": 61,
                "loc": value_loc,
            }));
        }
        if !invalid_type {
            need_runtime = Some(helper);
        }
    } else {
        errors.push(json!({
            "code": 58,
            "loc": dir.get("loc").cloned().unwrap_or(Value::Null),
        }));
    }

    projection["errors"] = json!(errors);
    projection["props"] = json!(dom_filter_native_model_props(
        projection
            .get("props")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
    ));
    if let Some(helper) = need_runtime {
        projection["needRuntime"] = json!(helper);
    }
    projection
}

/// Projects the DOM `Transition` node transform for compatibility bridge callers.
pub fn transform_transition_projection(payload: &Value) -> Value {
    let node = payload.get("node").unwrap_or(&Value::Null);
    let context = payload.get("context").unwrap_or(&Value::Null);
    if !json_bool(context, "isTransition") {
        return json!({ "transform": false });
    }
    let Some(children) = node.get("children").and_then(Value::as_array) else {
        return json!({ "transform": true, "errors": [] });
    };
    if children.is_empty() {
        return json!({ "transform": true, "errors": [] });
    }

    let visible_indices = transition_json_visible_child_indices(children);
    let visible_children = visible_indices
        .iter()
        .filter_map(|index| children.get(*index))
        .collect::<Vec<_>>();
    let mut errors = Vec::new();
    if transition_json_child_sequence_is_invalid(&visible_children, false) {
        if let Some(loc) = transition_json_error_loc(&visible_children) {
            errors.push(json!({
                "code": 63,
                "loc": loc,
            }));
        }
    }

    json!({
        "transform": true,
        "keepChildren": visible_indices,
        "errors": errors,
        "injectPersisted": transition_json_single_child_has_v_show(&visible_children),
    })
}

struct DomContentDirectiveProjection {
    key: &'static str,
    key_loc: Option<&'static str>,
    missing_expression_code: u8,
    with_children_code: u8,
    wrap_dynamic_text: bool,
}

fn transform_dom_content_directive_projection(
    payload: &Value,
    projection: DomContentDirectiveProjection,
) -> Value {
    let dir = payload.get("dir").unwrap_or(&Value::Null);
    let exp = dir.get("exp").filter(|exp| !exp.is_null());
    let has_children = payload
        .get("node")
        .and_then(|node| node.get("children"))
        .and_then(Value::as_array)
        .is_some_and(|children| !children.is_empty());

    let mut errors = Vec::new();
    if exp.is_none() {
        errors.push(json!({
            "code": projection.missing_expression_code,
            "loc": "dir",
        }));
    }
    if has_children {
        errors.push(json!({
            "code": projection.with_children_code,
            "loc": "dir",
        }));
    }

    let value = match exp {
        Some(exp) if projection.wrap_dynamic_text && !dom_directive_exp_is_constant(exp) => {
            json!({
                "kind": "displayString",
                "argument": {
                    "kind": "node",
                    "path": "dir.exp",
                },
                "loc": "dir",
            })
        }
        Some(_) => json!({
            "kind": "node",
            "path": "dir.exp",
        }),
        None => json!({
            "kind": "simple",
            "content": "",
            "isStatic": true,
        }),
    };

    let mut prop = json!({
        "key": projection.key,
        "value": value,
    });
    if let Some(key_loc) = projection.key_loc {
        prop["keyLoc"] = json!(key_loc);
    }

    json!({
        "props": [prop],
        "errors": errors,
        "clearChildren": has_children,
    })
}

fn dom_directive_exp_is_constant(exp: &Value) -> bool {
    exp.get("constType")
        .and_then(Value::as_i64)
        .is_some_and(|constant_type| constant_type > 0)
}

#[derive(Default)]
struct DomEventModifiers {
    key_modifiers: Vec<String>,
    non_key_modifiers: Vec<String>,
    event_option_modifiers: Vec<String>,
}

fn dom_directive_modifiers(dir: &Value) -> Vec<String> {
    dir.get("modifiers")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|modifier| {
            modifier
                .as_str()
                .or_else(|| modifier.get("content").and_then(Value::as_str))
                .map(ToOwned::to_owned)
        })
        .collect()
}

fn dom_resolve_event_modifiers(key: &Value, raw_modifiers: &[String]) -> DomEventModifiers {
    let mut modifiers = DomEventModifiers::default();
    for modifier in raw_modifiers {
        if dom_event_option_modifier(modifier) {
            modifiers.event_option_modifiers.push(modifier.clone());
            continue;
        }

        if dom_maybe_key_modifier(modifier) {
            if dom_projection_is_static_expression(key) {
                if dom_projection_is_keyboard_event(key) {
                    modifiers.key_modifiers.push(modifier.clone());
                } else {
                    modifiers.non_key_modifiers.push(modifier.clone());
                }
            } else {
                modifiers.key_modifiers.push(modifier.clone());
                modifiers.non_key_modifiers.push(modifier.clone());
            }
            continue;
        }

        if dom_non_key_modifier(modifier) {
            modifiers.non_key_modifiers.push(modifier.clone());
        } else {
            modifiers.key_modifiers.push(modifier.clone());
        }
    }
    modifiers
}

fn dom_event_option_modifier(modifier: &str) -> bool {
    matches!(modifier, "passive" | "once" | "capture")
}

fn dom_non_key_modifier(modifier: &str) -> bool {
    matches!(
        modifier,
        "stop" | "prevent" | "self" | "ctrl" | "shift" | "alt" | "meta" | "exact" | "middle"
    )
}

fn dom_maybe_key_modifier(modifier: &str) -> bool {
    matches!(modifier, "left" | "right")
}

fn dom_projection_is_static_expression(projection: &Value) -> bool {
    match projection.get("kind").and_then(Value::as_str) {
        Some("static") => true,
        Some("simple") => projection
            .get("isStatic")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        _ => false,
    }
}

fn dom_projection_is_keyboard_event(projection: &Value) -> bool {
    dom_projection_static_content(projection)
        .map(|content| {
            matches!(
                content.to_ascii_lowercase().as_str(),
                "onkeyup" | "onkeydown" | "onkeypress"
            )
        })
        .unwrap_or(false)
}

fn dom_projection_static_content(projection: &Value) -> Option<&str> {
    if dom_projection_is_static_expression(projection) {
        projection.get("content").and_then(Value::as_str)
    } else {
        None
    }
}

fn dom_transform_click_projection(key: Value, event: &str) -> Value {
    if dom_projection_static_content(&key)
        .is_some_and(|content| content.eq_ignore_ascii_case("onClick"))
    {
        return json!({
            "kind": "simple",
            "content": event,
            "isStatic": true,
            "loc": key.get("loc").cloned().unwrap_or(Value::Null),
        });
    }

    if key.get("kind").and_then(Value::as_str) != Some("simple") {
        return json!({
            "kind": "compound",
            "children": [
                "(",
                key.clone(),
                format!(") === \"onClick\" ? \"{event}\" : ("),
                key,
                ")",
            ],
        });
    }
    key
}

fn dom_helper_call_projection(helper: &str, arguments: Vec<Value>) -> Value {
    json!({
        "kind": "call",
        "callee": helper,
        "arguments": arguments,
    })
}

fn dom_event_option_key_projection(key: Value, postfix: &str) -> Value {
    if dom_projection_is_static_expression(&key) {
        let content = dom_projection_static_content(&key)
            .unwrap_or("")
            .to_string();
        let mut next = key;
        next["kind"] = json!("simple");
        next["content"] = json!(format!("{content}{postfix}"));
        next["isStatic"] = json!(true);
        return next;
    }

    json!({
        "kind": "compound",
        "children": [
            "(",
            key,
            format!(") + \"{postfix}\""),
        ],
    })
}

fn dom_json_string_array(values: &[String]) -> String {
    serde_json::to_string(values).unwrap_or_else(|_| "[]".to_string())
}

fn dom_capitalize(value: &str) -> String {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    first.to_uppercase().collect::<String>() + chars.as_str()
}

fn json_str<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

fn json_bool(value: &Value, key: &str) -> bool {
    value.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn json_u64(value: &Value, key: &str) -> Option<u64> {
    value.get(key).and_then(Value::as_u64)
}

fn dom_normalize_core_model_projection(mut projection: Value, dir: &Value) -> Value {
    let errors = projection
        .get("errors")
        .and_then(Value::as_array)
        .map(|errors| {
            errors
                .iter()
                .filter_map(|error| error.as_u64().map(|code| dom_core_model_error(code, dir)))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    projection["errors"] = json!(errors);
    projection
}

fn dom_core_model_error(code: u64, dir: &Value) -> Value {
    let loc = if code == 41 {
        dir.get("loc").cloned().unwrap_or(Value::Null)
    } else {
        dir.get("exp")
            .and_then(|exp| exp.get("loc"))
            .cloned()
            .or_else(|| dir.get("loc").cloned())
            .unwrap_or(Value::Null)
    };
    json!({
        "code": code,
        "loc": loc,
    })
}

enum DomModelInputType<'a> {
    None,
    Dynamic,
    PresentWithoutValue,
    Static(&'a str),
}

fn dom_model_input_type(node: &Value) -> DomModelInputType<'_> {
    let Some(props) = node.get("props").and_then(Value::as_array) else {
        return DomModelInputType::None;
    };
    for prop in props {
        if json_u64(prop, "type") == Some(6) && json_str(prop, "name") == Some("type") {
            return prop
                .get("value")
                .and_then(|value| json_str(value, "content"))
                .map(DomModelInputType::Static)
                .unwrap_or(DomModelInputType::PresentWithoutValue);
        }
        if json_u64(prop, "type") == Some(7)
            && json_str(prop, "name") == Some("bind")
            && prop.get("exp").is_some_and(|exp| !exp.is_null())
            && prop
                .get("arg")
                .filter(|arg| !arg.is_null())
                .is_some_and(|arg| {
                    json_bool(arg, "isStatic") && json_str(arg, "content") == Some("type")
                })
        {
            return DomModelInputType::Dynamic;
        }
    }
    if dom_model_has_dynamic_key_bind(props) {
        return DomModelInputType::Dynamic;
    }
    DomModelInputType::None
}

fn dom_model_has_dynamic_key_bind(props: &[Value]) -> bool {
    props.iter().any(|prop| {
        json_u64(prop, "type") == Some(7)
            && json_str(prop, "name") == Some("bind")
            && (prop.get("arg").is_none_or(Value::is_null)
                || prop
                    .get("arg")
                    .is_some_and(|arg| !json_bool(arg, "isStatic")))
    })
}

fn dom_model_dynamic_value_binding_loc(node: &Value) -> Option<Value> {
    node.get("props")
        .and_then(Value::as_array)?
        .iter()
        .find(|prop| {
            json_u64(prop, "type") == Some(7)
                && json_str(prop, "name") == Some("bind")
                && prop.get("exp").is_some_and(|exp| !exp.is_null())
                && prop
                    .get("arg")
                    .filter(|arg| !arg.is_null())
                    .is_some_and(|arg| {
                        json_bool(arg, "isStatic") && json_str(arg, "content") == Some("value")
                    })
        })
        .map(|prop| prop.get("loc").cloned().unwrap_or(Value::Null))
}

fn dom_filter_native_model_props(props: Vec<Value>) -> Vec<Value> {
    props
        .into_iter()
        .filter(|prop| {
            prop.get("key")
                .is_none_or(|key| json_str(key, "content") != Some("modelValue"))
        })
        .collect()
}

fn transition_json_visible_child_indices(children: &[Value]) -> Vec<usize> {
    children
        .iter()
        .enumerate()
        .filter_map(|(index, child)| transition_json_child_is_visible(child).then_some(index))
        .collect()
}

fn transition_json_child_is_visible(child: &Value) -> bool {
    match json_u64(child, "type") {
        Some(3) => false,
        Some(2) => json_str(child, "content")
            .or_else(|| json_str(child, "value"))
            .is_none_or(|text| !text.chars().all(is_html_whitespace)),
        _ => true,
    }
}

fn transition_json_child_sequence_is_invalid(children: &[&Value], empty_is_invalid: bool) -> bool {
    if children.is_empty() {
        return empty_is_invalid;
    }
    children.len() != 1 || transition_json_child_is_invalid(children[0])
}

fn transition_json_child_is_invalid(child: &Value) -> bool {
    if json_u64(child, "type") == Some(11) {
        return true;
    }
    if json_u64(child, "type") == Some(9) {
        return child
            .get("branches")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .any(transition_json_if_branch_is_invalid);
    }
    if json_u64(child, "type") == Some(1) {
        return child
            .get("props")
            .and_then(Value::as_array)
            .is_some_and(|props| transition_json_props_have_directive(props, "for"));
    }
    false
}

fn transition_json_if_branch_is_invalid(branch: &Value) -> bool {
    let visible_indices = branch
        .get("children")
        .and_then(Value::as_array)
        .map(|children| {
            transition_json_visible_child_indices(children)
                .into_iter()
                .filter_map(|index| children.get(index))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    transition_json_child_sequence_is_invalid(&visible_indices, true)
}

fn transition_json_single_child_has_v_show(children: &[&Value]) -> bool {
    let [child] = children else {
        return false;
    };
    if json_u64(child, "type") != Some(1) {
        return false;
    }
    child
        .get("props")
        .and_then(Value::as_array)
        .is_some_and(|props| transition_json_props_have_directive(props, "show"))
}

fn transition_json_props_have_directive(props: &[Value], name: &str) -> bool {
    props
        .iter()
        .any(|prop| json_u64(prop, "type") == Some(7) && json_str(prop, "name") == Some(name))
}

fn transition_json_error_loc(children: &[&Value]) -> Option<Value> {
    let first = children.first()?.get("loc")?;
    let last = children.last()?.get("loc")?;
    Some(json!({
        "start": first.get("start").cloned().unwrap_or(Value::Null),
        "end": last.get("end").cloned().unwrap_or(Value::Null),
        "source": "",
    }))
}

/// Extracts DOM directive summaries from compatibility template attributes.
pub fn extract_directives(attributes: &[TemplateAttribute]) -> Vec<DomDirective> {
    attributes
        .iter()
        .filter_map(|attr| parse_directive(attr))
        .collect()
}

fn style_json_string(value: &str) -> String {
    let style = parse_string_style(value);
    let properties = style
        .iter()
        .map(|(name, value)| {
            format!(
                "{}:{}",
                serde_json::to_string(name).unwrap_or_else(|_| "\"\"".into()),
                serde_json::to_string(value).unwrap_or_else(|_| "\"\"".into())
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("{{{properties}}}")
}

fn parse_string_style(value: &str) -> Vec<(String, String)> {
    let mut style = Vec::new();
    let mut current = String::new();
    let mut depth = 0usize;
    for ch in strip_css_comments(value).chars() {
        match ch {
            '(' => {
                depth += 1;
                current.push(ch);
            }
            ')' => {
                depth = depth.saturating_sub(1);
                current.push(ch);
            }
            ';' if depth == 0 => {
                push_style_declaration(&mut style, &current);
                current.clear();
            }
            _ => current.push(ch),
        }
    }
    push_style_declaration(&mut style, &current);
    style
}

fn strip_css_comments(value: &str) -> String {
    let mut output = String::new();
    let mut chars = value.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '/' && chars.peek() == Some(&'*') {
            chars.next();
            let mut previous = '\0';
            for next in chars.by_ref() {
                if previous == '*' && next == '/' {
                    break;
                }
                previous = next;
            }
        } else {
            output.push(ch);
        }
    }
    output
}

fn push_style_declaration(style: &mut Vec<(String, String)>, item: &str) {
    let Some((name, value)) = item.split_once(':') else {
        return;
    };
    let name = name.trim();
    let value = value.trim();
    if !name.is_empty() && !value.is_empty() {
        if let Some((_, existing)) = style.iter_mut().find(|(existing, _)| existing == name) {
            *existing = value.to_string();
        } else {
            style.push((name.to_string(), value.to_string()));
        }
    }
}

fn parse_directive(attr: &TemplateAttribute) -> Option<DomDirective> {
    let raw = attr.name.as_str();
    let (name, tail) = if let Some(stripped) = raw.strip_prefix("v-") {
        stripped
            .split_once(':')
            .map_or((stripped, ""), |(name, tail)| (name, tail))
    } else if let Some(argument) = raw.strip_prefix('@') {
        ("on", argument)
    } else if let Some(argument) = raw.strip_prefix(':') {
        ("bind", argument)
    } else {
        return None;
    };
    let mut parts = tail.split('.').filter(|part| !part.is_empty());
    let argument = parts.next().map(str::to_string);
    let modifiers = parts.map(str::to_string).collect();
    Some(DomDirective {
        name: name.to_string(),
        argument,
        modifiers,
        expression: attr.value.clone(),
    })
}

fn model_runtime_helper(tag: &str, directive: &DomDirective) -> &'static str {
    if tag == "select" {
        "vModelSelect"
    } else if tag == "textarea" {
        "vModelText"
    } else if directive.argument.as_deref() == Some("type") {
        "vModelDynamic"
    } else {
        match directive.expression.as_deref() {
            Some(value) if value.contains("checkbox") => "vModelCheckbox",
            Some(value) if value.contains("radio") => "vModelRadio",
            _ => "vModelText",
        }
    }
}

fn vue3_dom_root_mut(ast: &mut Vue3Ast) -> Option<&mut Vue3Root> {
    let root = ast.root_node_mut()?;
    match &mut root.kind {
        Vue3AstKind::Root(root) => Some(root),
        _ => None,
    }
}

fn report_transition_invalid_children(ast: &Vue3Ast, ctx: &mut TransformContext) {
    report_transition_invalid_children_for_node(ast, ast.root, ctx);
}

fn report_invalid_native_v_model(
    ast: &Vue3Ast,
    options: &Vue3CompilerOptions,
    ctx: &mut TransformContext,
) {
    for node in &ast.nodes {
        let Vue3AstKind::Element(element) = &node.kind else {
            continue;
        };
        if element.tag_type != Vue3ElementType::Element {
            continue;
        }
        if matches!(
            element.tag.as_str(),
            "input" | "textarea" | "select" | "script" | "style"
        ) {
            continue;
        }
        let Some(model) = element.props.iter().find_map(|prop| match prop {
            Vue3Prop::Directive(dir) if dir.name == "model" => Some(dir),
            _ => None,
        }) else {
            continue;
        };
        if model_binding_error_preempts_invalid_native_model(model, options) {
            continue;
        }
        ctx.report(Diagnostic::vue3_error(
            Vue3ErrorCode::XVModelOnInvalidElement,
            "v-model can only be used on <input>, <textarea> and <select> elements.",
            model.span.or_else(|| node.span.source()),
        ));
    }
}

fn model_binding_error_preempts_invalid_native_model(
    model: &vuec_ast::Vue3Directive,
    options: &Vue3CompilerOptions,
) -> bool {
    let Some(expression) = model.exp.as_ref() else {
        return true;
    };
    let raw = expression.source_string();
    let raw = raw.trim();
    if raw.is_empty() {
        return true;
    }
    if matches!(
        options.binding_metadata.get(raw).map(String::as_str),
        Some("props" | "props-aliased" | "literal-const" | "setup-const")
    ) {
        return true;
    }
    !vuec_vue3_core::model_is_member_expression(raw)
}

fn report_transition_invalid_children_for_node(
    ast: &Vue3Ast,
    node_id: NodeId,
    ctx: &mut TransformContext,
) {
    let Some(node) = ast.node(node_id) else {
        return;
    };
    if let Vue3AstKind::Element(element) = &node.kind {
        if element.tag_type == Vue3ElementType::Component
            && matches!(element.tag.as_str(), "Transition" | "transition")
            && transition_children_are_invalid(ast, &node.children)
        {
            ctx.report(Diagnostic::vue3_error(
                Vue3ErrorCode::XTransitionInvalidChildren,
                "<Transition> expects exactly one child element or component.",
                node.span.source(),
            ));
        }
    }
    for child_id in node.children.clone() {
        report_transition_invalid_children_for_node(ast, child_id, ctx);
    }
}

fn transition_children_are_invalid(ast: &Vue3Ast, children: &[NodeId]) -> bool {
    if children.is_empty() {
        return false;
    }
    transition_child_sequence_is_invalid(ast, &transition_visible_child_ids(ast, children), false)
}

fn transition_child_sequence_is_invalid(
    ast: &Vue3Ast,
    visible_children: &[NodeId],
    empty_is_invalid: bool,
) -> bool {
    if visible_children.is_empty() {
        return empty_is_invalid;
    }
    let mut logical_children = 0usize;
    let mut invalid = false;
    let mut index = 0usize;
    while index < visible_children.len() {
        logical_children += 1;
        let child_id = visible_children[index];
        if transition_child_starts_if_chain(ast, child_id) {
            let (branches, next_index) = collect_transition_if_chain(ast, visible_children, index);
            invalid |= branches
                .iter()
                .any(|branch_id| transition_if_branch_is_invalid(ast, *branch_id));
            index = next_index;
        } else {
            invalid |= transition_single_child_is_invalid(ast, child_id);
            index += 1;
        }
    }
    logical_children != 1 || invalid
}

fn transition_single_child_is_invalid(ast: &Vue3Ast, child_id: NodeId) -> bool {
    let Some(child) = ast.node(child_id) else {
        return false;
    };
    let Vue3AstKind::Element(element) = &child.kind else {
        return false;
    };
    element_has_directive(element, "for")
}

fn transition_if_branch_is_invalid(ast: &Vue3Ast, branch_id: NodeId) -> bool {
    let Some(branch) = ast.node(branch_id) else {
        return false;
    };
    let Vue3AstKind::Element(element) = &branch.kind else {
        return false;
    };
    if element_has_directive(element, "for") {
        return true;
    }
    if element.tag == "template" {
        return transition_child_sequence_is_invalid(
            ast,
            &transition_visible_child_ids(ast, &branch.children),
            true,
        );
    }
    false
}

fn collect_transition_if_chain(
    ast: &Vue3Ast,
    visible_children: &[NodeId],
    start: usize,
) -> (Vec<NodeId>, usize) {
    let mut branches = vec![visible_children[start]];
    let mut index = start + 1;
    while index < visible_children.len() {
        let Some(node) = ast.node(visible_children[index]) else {
            index += 1;
            continue;
        };
        let Vue3AstKind::Element(element) = &node.kind else {
            break;
        };
        if element_has_directive(element, "else-if") || element_has_directive(element, "else") {
            branches.push(visible_children[index]);
            index += 1;
        } else {
            break;
        }
    }
    (branches, index)
}

fn transition_child_starts_if_chain(ast: &Vue3Ast, child_id: NodeId) -> bool {
    ast.node(child_id).is_some_and(|child| {
        matches!(
            &child.kind,
            Vue3AstKind::Element(element) if element_has_directive(element, "if")
        )
    })
}

fn transition_visible_child_ids(ast: &Vue3Ast, children: &[NodeId]) -> Vec<NodeId> {
    children
        .iter()
        .copied()
        .filter(|child_id| {
            ast.node(*child_id).is_some_and(|child| match &child.kind {
                Vue3AstKind::Comment(_) => false,
                Vue3AstKind::Text(text) => !text.value.chars().all(is_html_whitespace),
                _ => true,
            })
        })
        .collect()
}

fn element_has_directive(element: &Vue3Element, name: &str) -> bool {
    element.props.iter().any(|prop| {
        matches!(
            prop,
            Vue3Prop::Directive(directive) if directive.name == name
        )
    })
}

fn is_html_whitespace(ch: char) -> bool {
    matches!(ch, '\t' | '\n' | '\u{000C}' | '\r' | ' ')
}

fn remove_side_effect_nodes(ast: &mut Vue3Ast, ctx: &mut TransformContext) {
    remove_side_effect_children(ast, ast.root, ctx);
}

fn remove_side_effect_children(ast: &mut Vue3Ast, parent_id: NodeId, ctx: &mut TransformContext) {
    let child_ids = ast
        .node(parent_id)
        .map(|node| node.children.clone())
        .unwrap_or_default();
    let mut retained = Vec::new();
    for child_id in child_ids {
        let remove = ast.node(child_id).is_some_and(|child| {
            matches!(
                child.kind,
                Vue3AstKind::Element(ref element) if element.tag == "script" || element.tag == "style"
            )
        });
        if remove {
            if let Some(span) = ast.node(child_id).and_then(|node| node.span.source()) {
                ctx.report(Diagnostic::vue3_error(
                    Vue3ErrorCode::XIgnoredSideEffectTag,
                    "Tags with side effect (<script> and <style>) are ignored in client component templates.",
                    Some(span),
                ));
            }
        } else {
            remove_side_effect_children(ast, child_id, ctx);
            retained.push(child_id);
        }
    }
    if let Some(parent) = ast.node_mut(parent_id) {
        parent.children = retained;
    }
}

fn decode_basic_entities(value: &str) -> String {
    value
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use vuec_source::FileId;

    fn template_source(name: &str, source: &str) -> TemplateSource {
        TemplateSource {
            filename: name.into(),
            source: source.into(),
            file_id: FileId(0),
            base_offset: 0,
        }
    }

    fn model_dir() -> Value {
        json!({
            "name": "model",
            "exp": {
                "type": 4,
                "content": "model",
                "loc": { "source": "model" }
            },
            "modifiers": [],
            "loc": { "source": "v-model=\"model\"" }
        })
    }

    fn model_node(tag: &str, props: Vec<Value>) -> Value {
        json!({
            "type": 1,
            "tag": tag,
            "tagType": 0,
            "props": props,
        })
    }

    fn model_projection(tag: &str, props: Vec<Value>) -> Value {
        transform_model_projection(&json!({
            "dir": model_dir(),
            "node": model_node(tag, props),
            "context": {},
        }))
    }

    fn transition_element_child(props: Vec<Value>) -> Value {
        json!({
            "type": 1,
            "tag": "div",
            "tagType": 0,
            "props": props,
            "children": [],
            "loc": {
                "start": { "line": 1, "column": 13, "offset": 12 },
                "end": { "line": 1, "column": 24, "offset": 23 },
                "source": "<div></div>"
            }
        })
    }

    fn transition_projection(children: Vec<Value>) -> Value {
        transform_transition_projection(&json!({
            "node": {
                "type": 1,
                "tag": "transition",
                "tagType": 1,
                "children": children,
            },
            "context": { "isTransition": true },
        }))
    }

    #[test]
    fn dom_compiler_ast_cache_hits_for_same_parse_input() {
        let mut compiler = DomCompiler::new();
        let mut options = DomCompilerOptions::default();
        options.core.prefix_identifiers = true;
        options.core.mode = "module".into();
        let source = template_source("cached.vue", "<div>{{ msg }}</div>");

        let first = compiler.compile(source.clone(), options.clone());
        let second = compiler.compile(source, options);

        assert_eq!(first.code, second.code);
        assert_eq!(
            compiler.cache_stats(),
            DomAstCacheStats {
                ast_hits: 1,
                ast_misses: 1,
                ast_invalidations: 0,
            }
        );
        assert_eq!(compiler.ast_cache_len(), 1);
    }

    #[test]
    fn dom_compiler_ast_cache_invalidates_changed_same_file_source() {
        let mut compiler = DomCompiler::new();
        let options = DomCompilerOptions::default();
        let first = template_source("cached.vue", "<div>{{ one }}</div>");
        let second = template_source("cached.vue", "<section>{{ two }}</section>");

        let first_result = compiler.compile(first, options.clone());
        let second_result = compiler.compile(second, options);

        assert_ne!(first_result.code, second_result.code);
        assert!(second_result.code.contains("section"));
        assert_eq!(
            compiler.cache_stats(),
            DomAstCacheStats {
                ast_hits: 0,
                ast_misses: 2,
                ast_invalidations: 1,
            }
        );
        assert_eq!(compiler.ast_cache_len(), 1);
    }

    #[test]
    fn dom_compiler_ast_cache_key_separates_parse_options() {
        let mut compiler = DomCompiler::new();
        let source = template_source("cached.vue", "<div><!--x-->{{ msg }}</div>");
        let with_comments = DomCompilerOptions::default();
        let mut without_comments = DomCompilerOptions::default();
        without_comments.core.comments = false;

        let with_comments_result = compiler.compile(source.clone(), with_comments);
        let without_comments_result = compiler.compile(source, without_comments);

        assert_ne!(
            with_comments_result.ast_summary,
            without_comments_result.ast_summary
        );
        assert_eq!(
            compiler.cache_stats(),
            DomAstCacheStats {
                ast_hits: 0,
                ast_misses: 2,
                ast_invalidations: 0,
            }
        );
        assert_eq!(compiler.ast_cache_len(), 2);
    }

    #[test]
    fn extracts_dom_directives() {
        let attrs = vec![
            TemplateAttribute {
                name: "@click.stop".into(),
                value: Some("save".into()),
            },
            TemplateAttribute {
                name: "v-model".into(),
                value: Some("checked".into()),
            },
        ];
        let directives = extract_directives(&attrs);
        assert_eq!(directives.len(), 2);
        assert_eq!(directives[0].name, "on");
        assert_eq!(directives[0].modifiers, vec!["stop"]);
    }

    #[test]
    fn compile_records_dom_summary() {
        let result = compile(
            TemplateSource {
                filename: "x.vue".into(),
                source: r#"<img src="./a.png"><input v-model="value">"#.into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            DomCompilerOptions::default(),
        );
        assert!(result.ast_summary.starts_with("dom:"));
        assert!(result.ast_summary.contains("v-model:vModelText"));
        assert!(!result.code.contains("data-vuec-dom"));
    }

    #[test]
    fn transform_model_projection_selects_text_runtime_and_filters_model_value() {
        let projection = model_projection("input", vec![]);

        assert_eq!(projection["errors"], json!([]));
        assert_eq!(projection["needRuntime"], json!("V_MODEL_TEXT"));
        assert_eq!(projection["props"].as_array().unwrap().len(), 1);
        assert_eq!(
            projection["props"][0]["key"],
            json!({ "kind": "static", "content": "onUpdate:modelValue" })
        );
    }

    #[test]
    fn transform_model_projection_selects_native_input_helpers() {
        let radio = model_projection(
            "input",
            vec![json!({
                "type": 6,
                "name": "type",
                "value": { "content": "radio" },
            })],
        );
        assert_eq!(radio["needRuntime"], json!("V_MODEL_RADIO"));

        let checkbox = model_projection(
            "input",
            vec![json!({
                "type": 6,
                "name": "type",
                "value": { "content": "checkbox" },
            })],
        );
        assert_eq!(checkbox["needRuntime"], json!("V_MODEL_CHECKBOX"));

        let dynamic = model_projection(
            "input",
            vec![json!({
                "type": 7,
                "name": "bind",
                "arg": { "type": 4, "content": "type", "isStatic": true },
                "exp": { "type": 4, "content": "kind" },
            })],
        );
        assert_eq!(dynamic["needRuntime"], json!("V_MODEL_DYNAMIC"));

        let static_type_wins_over_dynamic_bind = model_projection(
            "input",
            vec![
                json!({
                    "type": 6,
                    "name": "type",
                    "value": { "content": "radio" },
                }),
                json!({
                    "type": 7,
                    "name": "bind",
                    "arg": null,
                    "exp": { "type": 4, "content": "attrs" },
                }),
            ],
        );
        assert_eq!(
            static_type_wins_over_dynamic_bind["needRuntime"],
            json!("V_MODEL_RADIO")
        );
    }

    #[test]
    fn transform_model_projection_selects_select_textarea_and_custom_helpers() {
        let select = model_projection("select", vec![]);
        assert_eq!(select["needRuntime"], json!("V_MODEL_SELECT"));

        let textarea = model_projection("textarea", vec![]);
        assert_eq!(textarea["needRuntime"], json!("V_MODEL_TEXT"));

        let custom = transform_model_projection(&json!({
            "dir": model_dir(),
            "node": model_node("my-input", vec![]),
            "context": { "isCustomElement": true },
        }));
        assert_eq!(custom["errors"], json!([]));
        assert_eq!(custom["needRuntime"], json!("V_MODEL_TEXT"));
    }

    #[test]
    fn transform_model_projection_reports_dom_model_errors() {
        let file = model_projection(
            "input",
            vec![json!({
                "type": 6,
                "name": "type",
                "value": { "content": "file" },
            })],
        );
        assert_eq!(file["errors"][0]["code"], json!(60));
        assert!(file.get("needRuntime").is_none());

        let invalid = model_projection("span", vec![]);
        assert_eq!(invalid["errors"][0]["code"], json!(58));

        let with_arg = transform_model_projection(&json!({
            "dir": {
                "name": "model",
                "exp": {
                    "type": 4,
                    "content": "model",
                    "loc": { "source": "model" }
                },
                "arg": {
                    "type": 4,
                    "content": "value",
                    "isStatic": true,
                    "loc": { "source": "value" }
                },
                "modifiers": [],
                "loc": { "source": "v-model:value=\"model\"" }
            },
            "node": model_node("input", vec![]),
            "context": {},
        }));
        assert_eq!(with_arg["errors"][0]["code"], json!(59));
        assert_eq!(with_arg["errors"][0]["loc"]["source"], json!("value"));
        assert_eq!(with_arg["props"].as_array().unwrap().len(), 2);

        let dynamic_value = model_projection(
            "input",
            vec![json!({
                "type": 7,
                "name": "bind",
                "arg": { "type": 4, "content": "value", "isStatic": true },
                "exp": { "type": 4, "content": "model" },
                "loc": { "source": ":value=\"model\"" },
            })],
        );
        assert_eq!(dynamic_value["errors"][0]["code"], json!(61));
        assert_eq!(
            dynamic_value["errors"][0]["loc"]["source"],
            json!(":value=\"model\"")
        );

        let static_value = model_projection(
            "input",
            vec![json!({
                "type": 6,
                "name": "value",
                "value": { "content": "model" },
            })],
        );
        assert_eq!(static_value["errors"], json!([]));
    }

    #[test]
    fn transform_transition_projection_filters_comments_and_whitespace() {
        let projection = transition_projection(vec![
            json!({ "type": 3, "content": "ignored" }),
            json!({ "type": 2, "content": "\n  " }),
            transition_element_child(vec![]),
        ]);

        assert_eq!(projection["keepChildren"], json!([2]));
        assert_eq!(projection["errors"], json!([]));
        assert_eq!(projection["injectPersisted"], json!(false));
    }

    #[test]
    fn transform_transition_projection_reports_invalid_children() {
        let projection = transition_projection(vec![
            transition_element_child(vec![]),
            json!({
                "type": 1,
                "tag": "span",
                "tagType": 0,
                "props": [],
                "children": [],
                "loc": {
                    "start": { "line": 1, "column": 25, "offset": 24 },
                    "end": { "line": 1, "column": 38, "offset": 37 },
                    "source": "<span></span>"
                }
            }),
        ]);

        assert_eq!(projection["errors"][0]["code"], json!(63));
        assert_eq!(projection["errors"][0]["loc"]["start"]["offset"], json!(12));
        assert_eq!(projection["errors"][0]["loc"]["end"]["offset"], json!(37));

        let for_child = transition_projection(vec![json!({
            "type": 11,
            "loc": {
                "start": { "line": 1, "column": 13, "offset": 12 },
                "end": { "line": 1, "column": 40, "offset": 39 },
                "source": "<div v-for=\"i in items\"/>"
            }
        })]);
        assert_eq!(for_child["errors"][0]["code"], json!(63));
    }

    #[test]
    fn transform_transition_projection_handles_if_branch_shape() {
        let valid_if = transition_projection(vec![json!({
            "type": 9,
            "branches": [
                { "children": [transition_element_child(vec![])] },
                { "children": [transition_element_child(vec![])] }
            ],
            "loc": {
                "start": { "line": 1, "column": 13, "offset": 12 },
                "end": { "line": 1, "column": 80, "offset": 79 },
                "source": ""
            }
        })]);
        assert_eq!(valid_if["errors"], json!([]));

        let invalid_template_if = transition_projection(vec![json!({
            "type": 9,
            "branches": [
                { "children": [] }
            ],
            "loc": {
                "start": { "line": 1, "column": 13, "offset": 12 },
                "end": { "line": 1, "column": 40, "offset": 39 },
                "source": ""
            }
        })]);
        assert_eq!(invalid_template_if["errors"][0]["code"], json!(63));
    }

    #[test]
    fn transform_transition_projection_injects_persisted_for_v_show_child() {
        let projection = transition_projection(vec![transition_element_child(vec![json!({
            "type": 7,
            "name": "show",
        })])]);

        assert_eq!(projection["errors"], json!([]));
        assert_eq!(projection["injectPersisted"], json!(true));
    }

    #[test]
    fn parse_uses_dom_parser_defaults() {
        let ast = parse(
            TemplateSource {
                filename: "x.vue".into(),
                source: "<input><hello/>".into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            &DomCompilerOptions::default(),
        );
        let root = ast.node(ast.root).expect("root");
        let input = ast.node(root.children[0]).expect("input");
        let hello = ast.node(root.children[1]).expect("hello");

        assert!(input.children.is_empty());
        assert!(matches!(
            &hello.kind,
            Vue3AstKind::Element(element)
                if element.tag == "hello" && element.tag_type == Vue3ElementType::Component
        ));
    }

    #[test]
    fn compile_rewrites_explicit_base_assets_in_module_mode() {
        let mut options = DomCompilerOptions {
            asset_url_options: AssetUrlOptions {
                base: Some("/foo".into()),
                ..AssetUrlOptions::default()
            },
            ..DomCompilerOptions::default()
        };
        options.core.mode = "module".into();
        options.core.prefix_identifiers = true;
        let result = compile(
            TemplateSource {
                filename: "x.vue".into(),
                source: r#"<img src="./bar.png"><img src="bar.png"><img src="~bar.png"><img src="@theme/bar.png"><img src="/bar.png"><img src="data:image/png;base64,i">"#.into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            options,
        );

        assert!(result.code.contains(r#"src: "/foo/bar.png""#));
        assert!(result.code.contains(r#"src: "bar.png""#));
        assert!(result.code.contains("import _imports_0 from 'bar.png'"));
        assert!(result
            .code
            .contains("import _imports_1 from '@theme/bar.png'"));
        assert!(result.code.contains("src: _imports_0"));
        assert!(result.code.contains("src: _imports_1"));
        assert!(result.code.contains(r#"src: "/bar.png""#));
        assert!(result.code.contains(r#"src: "data:image/png;base64,i""#));
        assert!(!result.code.contains(r#"src: "~bar.png""#));
        assert!(!result.code.contains(r#"src: "@theme/bar.png""#));
    }

    #[test]
    fn compile_transforms_asset_urls_to_imports_in_module_mode() {
        let mut options = DomCompilerOptions::default();
        options.core.mode = "module".into();
        options.core.prefix_identifiers = true;
        let result = compile(
            TemplateSource {
                filename: "x.vue".into(),
                source: r##"<img src="./bar.png"><img src="~fixtures/logo.png"><img src="@theme/bar.png"><img src="./icons.svg#heart"><use href="#local"></use>"##.into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            options,
        );

        assert!(result.code.contains("import _imports_0 from './bar.png'"));
        assert!(result
            .code
            .contains("import _imports_1 from 'fixtures/logo.png'"));
        assert!(result
            .code
            .contains("import _imports_2 from '@theme/bar.png'"));
        assert!(result.code.contains("import _imports_3 from './icons.svg'"));
        assert!(result.code.contains("src: _imports_0"));
        assert!(result.code.contains("src: _imports_1"));
        assert!(result.code.contains("src: _imports_2"));
        assert!(result.code.contains(r#"src: _imports_3 + '#heart'"#));
        assert!(result.code.contains(r##"href: "#local""##));
        assert!(!result.code.contains("_ctx._imports_"));
    }

    #[test]
    fn compile_caches_static_children_with_asset_url_imports() {
        let mut options = DomCompilerOptions::default();
        options.core.mode = "module".into();
        options.core.prefix_identifiers = true;
        options.core.hoist_static = true;
        let result = compile(
            TemplateSource {
                filename: "x.vue".into(),
                source: r#"<div><img src="./bar.png"><span title="static">ok</span></div>"#.into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            options,
        );

        assert!(result.code.contains("import _imports_0 from './bar.png'"));
        assert!(result.code.contains("_cache[0] || (_cache[0] = ["));
        assert!(result.code.contains("src: _imports_0"));
        assert!(result.code.contains("-1"));
        assert!(!result.code.contains("_ctx._imports_0"));
        assert!(!result.code.contains("8 /* PROPS */"));
        assert!(!result.code.contains("[\"src\"]"));
    }

    #[test]
    fn compile_stringifies_static_children_with_asset_url_imports() {
        let mut options = DomCompilerOptions::default();
        options.core.mode = "module".into();
        options.core.prefix_identifiers = true;
        options.core.hoist_static = true;
        options.core.stringify_static = true;
        let result = compile(
            TemplateSource {
                filename: "x.vue".into(),
                source: format!(
                    r#"<div><img src="./bar.png" srcset="./bar.png, ./icons.svg#heart 2x" />{}</div>"#,
                    r#"<span title="static">ok</span>"#.repeat(5)
                ),
                file_id: FileId(0),
                base_offset: 0,
            },
            options,
        );

        assert!(result.code.contains("import _imports_0 from './bar.png'"));
        assert!(result.code.contains("import _imports_1 from './icons.svg'"));
        assert!(
            result.code.contains("_createStaticVNode"),
            "{}",
            result.code
        );
        assert!(result.code.contains(r##"_createStaticVNode("<img src=\"" + _imports_0 + "\" srcset=\"" + _imports_0 + ", " + _imports_1 + "#heart 2x\"><span title=\"static\">ok</span>"##));
        assert!(!result.code.contains("src: _imports_0"));
        assert!(!result.code.contains("_ctx._imports_0"));
        assert!(!result.code.contains("_ctx._imports_1"));
    }

    #[test]
    fn compile_stringifies_multiple_static_chunks_around_dynamic_child() {
        let mut options = DomCompilerOptions::default();
        options.core.prefix_identifiers = true;
        options.core.hoist_static = true;
        options.core.stringify_static = true;
        let result = compile(
            TemplateSource {
                filename: "x.vue".into(),
                source: format!(
                    "<div>{}{{{{ msg }}}}{}</div>",
                    r#"<span class="foo"></span>"#.repeat(5),
                    r#"<span class="bar"></span>"#.repeat(5)
                ),
                file_id: FileId(0),
                base_offset: 0,
            },
            options,
        );

        assert_eq!(result.code.matches("_createStaticVNode(").count(), 2);
        assert!(result.code.contains("_createStaticVNode(\"<span class=\\\"foo\\\"></span><span class=\\\"foo\\\"></span><span class=\\\"foo\\\"></span><span class=\\\"foo\\\"></span><span class=\\\"foo\\\"></span>\", 5)"));
        assert!(result
            .code
            .contains("_createTextVNode(_toDisplayString(_ctx.msg), 1 /* TEXT */)"));
        assert!(result.code.contains("_createStaticVNode(\"<span class=\\\"bar\\\"></span><span class=\\\"bar\\\"></span><span class=\\\"bar\\\"></span><span class=\\\"bar\\\"></span><span class=\\\"bar\\\"></span>\", 5)"));
    }

    #[test]
    fn compile_bails_stringify_static_invalid_p_child_placement() {
        let mut options = DomCompilerOptions::default();
        options.core.prefix_identifiers = true;
        options.core.hoist_static = true;
        options.core.stringify_static = true;
        let result = compile(
            TemplateSource {
                filename: "x.vue".into(),
                source: format!(
                    "<div><p>{}</p></div>",
                    r#"<span class="inline"></span>"#.repeat(5)
                        + "<span><div class=\"block\"></div></span>"
                ),
                file_id: FileId(0),
                base_offset: 0,
            },
            options,
        );

        assert!(!result.code.contains("_createStaticVNode"));
        assert!(result.code.contains("_cache[0] || (_cache[0] = ["));
        assert!(result.code.contains("_createElementVNode(\"p\""));
        assert!(result
            .code
            .contains("_createElementVNode(\"div\", { class: \"block\" })"));
    }

    #[test]
    fn compile_stringifies_static_children_when_transform_hoist_requested() {
        let mut options = DomCompilerOptions::default();
        options.core.prefix_identifiers = true;
        options.core.hoist_static = true;
        options.core.stringify_static = true;
        let result = compile(
            TemplateSource {
                filename: "x.vue".into(),
                source: format!("<div>{}</div>", r#"<span class="foo"/>"#.repeat(5)),
                file_id: FileId(0),
                base_offset: 0,
            },
            options,
        );

        assert!(result.code.contains("createStaticVNode"));
        assert!(result.code.contains("_createStaticVNode(\"<span class=\\\"foo\\\"></span><span class=\\\"foo\\\"></span><span class=\\\"foo\\\"></span><span class=\\\"foo\\\"></span><span class=\\\"foo\\\"></span>\", 5)"));
    }

    #[test]
    fn compile_stringifies_static_constant_bindings_when_transform_hoist_requested() {
        let mut options = DomCompilerOptions::default();
        options.core.prefix_identifiers = true;
        options.core.hoist_static = true;
        options.core.stringify_static = true;
        let result = compile(
            TemplateSource {
                filename: "x.vue".into(),
                source: format!(
                    r#"<div><div :style="`color:red;`">{}</div></div>"#,
                    r#"<span :class="[{ foo: true }, { bar: true }]">{{ 1 }} + {{ false }}</span>"#
                        .repeat(5)
                ),
                file_id: FileId(0),
                base_offset: 0,
            },
            options,
        );

        assert!(result.code.contains("createStaticVNode"));
        assert!(result
            .code
            .contains(r#"<div style=\"color:red;\"><span class=\"foo bar\">1 + false</span>"#));
    }

    #[test]
    fn compile_stringifies_static_children_with_scope_id() {
        let mut options = DomCompilerOptions::default();
        options.core.prefix_identifiers = true;
        options.core.mode = "module".into();
        options.core.hoist_static = true;
        options.core.stringify_static = true;
        options.core.scope_id = Some("data-v-test".into());
        let result = compile(
            TemplateSource {
                filename: "x.vue".into(),
                source: format!(
                    r#"<div><div :style="`color:red;`">{}</div></div>"#,
                    r#"<span class="foo">ok</span>"#.repeat(5)
                ),
                file_id: FileId(0),
                base_offset: 0,
            },
            options,
        );

        assert!(result.code.contains("_createStaticVNode"));
        assert!(result.code.contains(
            r#"<div style=\"color:red;\" data-v-test><span class=\"foo\" data-v-test>ok</span>"#
        ));
    }

    #[test]
    fn compile_stringifies_static_svg_namespace_children_by_default() {
        let mut options = DomCompilerOptions::default();
        options.core.prefix_identifiers = true;
        options.core.hoist_static = true;
        options.core.stringify_static = true;
        let result = compile(
            TemplateSource {
                filename: "x.vue".into(),
                source: format!(
                    r#"<div><svg width="50" height="50" viewBox="0 0 50 50" fill="none" xmlns="http://www.w3.org/2000/svg">{}</svg></div>"#,
                    r##"<rect width="50" height="50" fill="#C4C4C4"></rect>"##.repeat(5)
                ),
                file_id: FileId(0),
                base_offset: 0,
            },
            options,
        );

        assert!(result.code.contains("_createStaticVNode"));
        assert!(result.code.contains(r#"<svg width=\"50\" height=\"50\" viewBox=\"0 0 50 50\" fill=\"none\" xmlns=\"http://www.w3.org/2000/svg\">"#));
        assert!(result
            .code
            .contains(r##"<rect width=\"50\" height=\"50\" fill=\"#C4C4C4\"></rect>"##));
    }

    #[test]
    fn compile_transforms_srcset_imports_in_module_mode() {
        let mut options = DomCompilerOptions::default();
        options.core.mode = "module".into();
        options.core.prefix_identifiers = true;
        let result = compile(
            TemplateSource {
                filename: "x.vue".into(),
                source: r#"<img src="./logo.png" srcset="./logo.png, ./icons.svg#heart 2x, /absolute.png 3x, data:image/png;base64,i 4x">"#.into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            options,
        );

        assert!(result.code.contains("import _imports_0 from './logo.png'"));
        assert!(result.code.contains("import _imports_1 from './icons.svg'"));
        assert!(result.code.matches("import _imports_0").count() == 1);
        assert!(result.code.contains(
            r#"srcset: _imports_0 + ', ' + _imports_1 + '#heart' + ' 2x, ' + "/absolute.png" + ' 3x, ' + "data:image/png;base64,i" + ' 4x'"#
        ));
    }

    #[test]
    fn compile_rewrites_asset_url_base_with_hosts_and_hashes() {
        let cases = [
            (
                "http://localhost:3000/src/",
                "./logo.png",
                r#"src: "http://localhost:3000/src/logo.png""#,
            ),
            (
                "http://localhost:3000",
                "./logo.png",
                r#"src: "http://localhost:3000/logo.png""#,
            ),
            (
                "http://localhost",
                "./logo.png",
                r#"src: "http://localhost/logo.png""#,
            ),
            (
                "//localhost",
                "./logo.png",
                r#"src: "//localhost/logo.png""#,
            ),
            (
                "/foo",
                "./icons.svg#heart",
                r#"src: "/foo/icons.svg#heart""#,
            ),
        ];

        for (index, (base, url, expected)) in cases.iter().enumerate() {
            let result = compile(
                TemplateSource {
                    filename: format!("asset-base-{index}.vue"),
                    source: format!(r#"<img src="{url}">"#),
                    file_id: FileId(index as u32),
                    base_offset: 0,
                },
                DomCompilerOptions {
                    asset_url_options: AssetUrlOptions {
                        base: Some((*base).into()),
                        ..AssetUrlOptions::default()
                    },
                    ..DomCompilerOptions::default()
                },
            );

            assert!(
                result.code.contains(expected),
                "base {base} url {url} generated:\n{}",
                result.code
            );
        }
    }

    #[test]
    fn compile_rewrites_srcset_base_when_all_processable_urls_are_dot_relative() {
        let result = compile(
            TemplateSource {
                filename: "x.vue".into(),
                source: r#"<img srcset="./logo.png, ./logo.png 2x, /logo.png 3x, data:image/png;base64,i 4x">"#.into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            DomCompilerOptions {
                asset_url_options: AssetUrlOptions {
                    base: Some("/foo".into()),
                    ..AssetUrlOptions::default()
                },
                ..DomCompilerOptions::default()
            },
        );

        assert!(result.code.contains(
            r#"srcset: "/foo/logo.png, /foo/logo.png 2x, /logo.png 3x, data:image/png;base64,i 4x""#
        ));
    }

    #[test]
    fn compile_rewrites_srcset_base_independently_of_asset_tag_options() {
        let result = compile(
            TemplateSource {
                filename: "x.vue".into(),
                source: r#"<img src="./logo.png" srcset="./logo.png 2x">"#.into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            DomCompilerOptions {
                asset_url_options: AssetUrlOptions {
                    base: Some("/foo".into()),
                    tags: BTreeMap::new(),
                    ..AssetUrlOptions::default()
                },
                ..DomCompilerOptions::default()
            },
        );

        assert!(result.code.contains(r#"src: "./logo.png""#));
        assert!(result.code.contains(r#"srcset: "/foo/logo.png 2x""#));
    }

    #[test]
    fn compile_rewrites_mixed_srcset_base_candidates_and_imports_alias_candidates() {
        let mut options = DomCompilerOptions {
            asset_url_options: AssetUrlOptions {
                base: Some("/foo".into()),
                ..AssetUrlOptions::default()
            },
            ..DomCompilerOptions::default()
        };
        options.core.mode = "module".into();
        options.core.prefix_identifiers = true;
        let result = compile(
            TemplateSource {
                filename: "x.vue".into(),
                source: r#"<img srcset="@/logo.png 1x, ./logo.png 2x">"#.into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            options,
        );

        assert!(result.code.contains("import _imports_0 from '@/logo.png'"));
        assert!(result
            .code
            .contains(r#"srcset: _imports_0 + ' 1x, ' + "/foo/logo.png" + ' 2x'"#));
    }

    #[test]
    fn compile_transforms_asset_url_options_for_custom_tags() {
        let mut tags = BTreeMap::new();
        tags.insert("foo".into(), vec!["bar".into()]);
        let mut options = DomCompilerOptions {
            asset_url_options: AssetUrlOptions {
                tags,
                ..AssetUrlOptions::default()
            },
            ..DomCompilerOptions::default()
        };
        options.core.mode = "module".into();
        options.core.prefix_identifiers = true;
        let result = compile(
            TemplateSource {
                filename: "x.vue".into(),
                source: r#"<foo bar="~baz"></foo>"#.into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            options,
        );

        assert!(result.code.contains("import _imports_0 from 'baz'"));
        assert!(result.code.contains("bar: _imports_0"));
    }

    #[test]
    fn compile_respects_disabled_asset_url_transform() {
        let result = compile(
            TemplateSource {
                filename: "x.vue".into(),
                source: r#"<img src="./bar.png">"#.into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            DomCompilerOptions {
                transform_asset_urls: false,
                asset_url_options: AssetUrlOptions {
                    base: Some("/foo".into()),
                    ..AssetUrlOptions::default()
                },
                ..DomCompilerOptions::default()
            },
        );

        assert!(result.code.contains(r#"src: "./bar.png""#));
        assert!(!result.code.contains("/foo/bar.png"));
    }

    #[test]
    fn parse_marks_dom_transition_builtins() {
        let ast = parse(
            TemplateSource {
                filename: "x.vue".into(),
                source: "<transition/><transition-group/>".into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            &DomCompilerOptions::default(),
        );
        let tags = ast
            .nodes
            .iter()
            .filter_map(|node| match &node.kind {
                Vue3AstKind::Element(element) => Some((
                    element.tag.as_str(),
                    element.tag_type == vuec_ast::Vue3ElementType::Component,
                )),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(tags, vec![("transition", true), ("transition-group", true)]);
    }

    #[test]
    fn compile_reports_transition_invalid_children_diagnostics() {
        let cases = [
            ("<transition><div>hey</div><div>hey</div></transition>", true),
            ("<transition><div v-for=\"i in items\">hey</div></transition>", true),
            (
                "<transition><div v-if=\"a\" v-for=\"i in items\">hey</div><div v-else v-for=\"i in items\">hey</div></transition>",
                true,
            ),
            ("<transition><template v-if=\"ok\"></template></transition>", true),
            (
                "<transition><template v-if=\"a\"></template><template v-else></template></transition>",
                true,
            ),
            (
                "<transition><div v-if=\"one\">hey</div><div v-if=\"other\">hey</div></transition>",
                true,
            ),
            ("<transition><div>hey</div></transition>", false),
            ("<transition><div v-if=\"a\">hey</div></transition>", false),
            (
                "<transition><div v-if=\"a\">hey</div><div v-else-if=\"b\">hey</div><div v-else>hey</div></transition>",
                false,
            ),
            (
                "<transition><div v-if=\"a\">hey</div><div v-else>hey</div></transition>",
                false,
            ),
            ("<transition>\u{00a0}<div>foo</div></transition>", true),
            (
                "<transition><!-- foo --> <!-- bar --><div>foo bar</div></transition>",
                false,
            ),
        ];
        for (index, (source, should_warn)) in cases.iter().enumerate() {
            let result = compile(
                TemplateSource {
                    filename: format!("case-{index}.vue"),
                    source: (*source).into(),
                    file_id: FileId(index as u32),
                    base_offset: 0,
                },
                DomCompilerOptions::default(),
            );

            let has_warning = result.diagnostics.iter().any(|diagnostic| {
                diagnostic.message == "<Transition> expects exactly one child element or component."
            });
            assert_eq!(has_warning, *should_warn, "case {index}: {source}");
        }
    }

    #[test]
    fn compile_reports_invalid_native_v_model_diagnostics() {
        let result = compile(
            TemplateSource {
                filename: "foo.vue".into(),
                source: r#"<div v-model="baz"/>"#.into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            DomCompilerOptions::default(),
        );

        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].code, "58");
        assert_eq!(
            result.diagnostics[0].message,
            "v-model can only be used on <input>, <textarea> and <select> elements."
        );
        assert_eq!(
            result.diagnostics[0].span,
            Some(vuec_source::Span::new(FileId(0), 5, 18))
        );
    }

    #[test]
    fn compile_suppresses_invalid_native_v_model_after_binding_errors() {
        let mut options = DomCompilerOptions::default();
        options
            .core
            .binding_metadata
            .insert("foo".into(), "literal-const".into());
        let result = compile(
            TemplateSource {
                filename: "foo.vue".into(),
                source: r#"<div v-model="foo"/><div v-model="foo + bar"/>"#.into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            options,
        );

        assert_eq!(
            result
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.code.as_str())
                .collect::<Vec<_>>(),
            ["45", "42"]
        );
    }

    #[test]
    fn transform_style_projection_rewrites_static_style() {
        let projection = transform_style_projection(&json!({
            "node": {
                "props": [
                    {
                        "type": 6,
                        "name": "style",
                        "value": {
                            "content": "color: green; background: url(a;b); /* x */ margin: 0"
                        }
                    }
                ]
            }
        }));

        assert_eq!(projection["replacements"][0]["index"], json!(0));
        assert_eq!(
            projection["replacements"][0]["expression"],
            json!("{\"color\":\"green\",\"background\":\"url(a;b)\",\"margin\":\"0\"}")
        );
    }

    #[test]
    fn transform_v_html_projection_reports_children_and_clears_them() {
        let projection = transform_v_html_projection(&json!({
            "dir": {
                "exp": {
                    "type": 4,
                    "content": "raw",
                    "isStatic": false,
                    "constType": 0
                },
                "loc": { "source": "v-html=\"raw\"" }
            },
            "node": {
                "children": [
                    { "type": 2, "content": "old" }
                ]
            }
        }));

        assert_eq!(projection["clearChildren"], json!(true));
        assert_eq!(projection["errors"][0]["code"], json!(55));
        assert_eq!(projection["errors"][0]["loc"], json!("dir"));
        assert_eq!(projection["props"][0]["key"], json!("innerHTML"));
        assert_eq!(projection["props"][0]["keyLoc"], json!("dir"));
        assert_eq!(projection["props"][0]["value"]["kind"], json!("node"));
        assert_eq!(projection["props"][0]["value"]["path"], json!("dir.exp"));
    }

    #[test]
    fn transform_v_html_projection_reports_missing_expression() {
        let projection = transform_v_html_projection(&json!({
            "dir": {
                "loc": { "source": "v-html" }
            },
            "node": {
                "children": []
            }
        }));

        assert_eq!(projection["clearChildren"], json!(false));
        assert_eq!(projection["errors"][0]["code"], json!(54));
        assert_eq!(projection["props"][0]["value"]["kind"], json!("simple"));
        assert_eq!(projection["props"][0]["value"]["content"], json!(""));
        assert_eq!(projection["props"][0]["value"]["isStatic"], json!(true));
    }

    #[test]
    fn transform_v_text_projection_wraps_dynamic_expression() {
        let projection = transform_v_text_projection(&json!({
            "dir": {
                "exp": {
                    "type": 4,
                    "content": "msg",
                    "isStatic": false,
                    "constType": 0
                },
                "loc": { "source": "v-text=\"msg\"" }
            },
            "node": {
                "children": []
            }
        }));

        assert_eq!(projection["errors"].as_array().unwrap().len(), 0);
        assert_eq!(projection["props"][0]["key"], json!("textContent"));
        assert!(projection["props"][0]["keyLoc"].is_null());
        assert_eq!(
            projection["props"][0]["value"]["kind"],
            json!("displayString")
        );
        assert_eq!(
            projection["props"][0]["value"]["argument"]["path"],
            json!("dir.exp")
        );
    }

    #[test]
    fn transform_v_text_projection_keeps_constant_expression() {
        let projection = transform_v_text_projection(&json!({
            "dir": {
                "exp": {
                    "type": 4,
                    "content": "'hi'",
                    "isStatic": false,
                    "constType": 3
                },
                "loc": { "source": "v-text=\"'hi'\"" }
            },
            "node": {
                "children": [
                    { "type": 2, "content": "old" }
                ]
            }
        }));

        assert_eq!(projection["clearChildren"], json!(true));
        assert_eq!(projection["errors"][0]["code"], json!(57));
        assert_eq!(projection["props"][0]["value"]["kind"], json!("node"));
        assert_eq!(projection["props"][0]["value"]["path"], json!("dir.exp"));
    }

    #[test]
    fn transform_show_projection_returns_runtime_helper() {
        let projection = transform_show_projection(&json!({
            "dir": {
                "exp": {
                    "type": 4,
                    "content": "ok",
                    "isStatic": false,
                    "constType": 0
                },
                "loc": { "source": "v-show=\"ok\"" }
            }
        }));

        assert_eq!(projection["props"].as_array().unwrap().len(), 0);
        assert_eq!(projection["errors"].as_array().unwrap().len(), 0);
        assert_eq!(projection["needRuntime"], json!("V_SHOW"));
    }

    #[test]
    fn transform_show_projection_reports_missing_expression() {
        let projection = transform_show_projection(&json!({
            "dir": {
                "loc": { "source": "v-show" }
            }
        }));

        assert_eq!(projection["props"].as_array().unwrap().len(), 0);
        assert_eq!(projection["errors"][0]["code"], json!(62));
        assert_eq!(projection["errors"][0]["loc"], json!("dir"));
        assert_eq!(projection["needRuntime"], json!("V_SHOW"));
    }

    #[test]
    fn transform_on_projection_wraps_non_key_modifiers() {
        let projection = transform_on_projection(&json!({
            "dir": {
                "name": "on",
                "arg": {
                    "type": 4,
                    "content": "click",
                    "isStatic": true,
                    "loc": { "source": "click" }
                },
                "exp": {
                    "type": 4,
                    "content": "test",
                    "isStatic": false,
                    "constType": 0,
                    "loc": { "source": "test" }
                },
                "modifiers": [{ "content": "stop" }, { "content": "prevent" }],
                "loc": { "source": "@click.stop.prevent=\"test\"" }
            },
            "node": { "type": 1, "tag": "div", "tagType": 0 },
            "context": { "prefixIdentifiers": true }
        }));

        assert_eq!(projection["props"][0]["key"]["content"], json!("onClick"));
        assert_eq!(projection["props"][0]["value"]["kind"], json!("call"));
        assert_eq!(
            projection["props"][0]["value"]["callee"],
            json!("V_ON_WITH_MODIFIERS")
        );
        assert_eq!(
            projection["props"][0]["value"]["arguments"][1],
            json!("[\"stop\",\"prevent\"]")
        );
    }

    #[test]
    fn transform_on_projection_wraps_key_and_option_modifiers() {
        let projection = transform_on_projection(&json!({
            "dir": {
                "name": "on",
                "arg": {
                    "type": 4,
                    "content": "keydown",
                    "isStatic": true,
                    "loc": { "source": "keydown" }
                },
                "exp": {
                    "type": 4,
                    "content": "test",
                    "isStatic": false,
                    "constType": 0,
                    "loc": { "source": "test" }
                },
                "modifiers": [
                    { "content": "stop" },
                    { "content": "capture" },
                    { "content": "ctrl" },
                    { "content": "a" }
                ],
                "loc": { "source": "@keydown.stop.capture.ctrl.a=\"test\"" }
            },
            "node": { "type": 1, "tag": "div", "tagType": 0 },
            "context": { "prefixIdentifiers": true }
        }));

        assert_eq!(
            projection["props"][0]["key"]["content"],
            json!("onKeydownCapture")
        );
        let value = &projection["props"][0]["value"];
        assert_eq!(value["callee"], json!("V_ON_WITH_KEYS"));
        assert_eq!(value["arguments"][1], json!("[\"a\"]"));
        assert_eq!(
            value["arguments"][0]["callee"],
            json!("V_ON_WITH_MODIFIERS")
        );
        assert_eq!(
            value["arguments"][0]["arguments"][1],
            json!("[\"stop\",\"ctrl\"]")
        );
    }

    #[test]
    fn transform_on_projection_rewrites_click_right() {
        let projection = transform_on_projection(&json!({
            "dir": {
                "name": "on",
                "arg": {
                    "type": 4,
                    "content": "click",
                    "isStatic": true,
                    "loc": { "source": "click" }
                },
                "exp": {
                    "type": 4,
                    "content": "test",
                    "isStatic": false,
                    "constType": 0,
                    "loc": { "source": "test" }
                },
                "modifiers": [{ "content": "right" }],
                "loc": { "source": "@click.right=\"test\"" }
            },
            "node": { "type": 1, "tag": "div", "tagType": 0 },
            "context": {}
        }));

        assert_eq!(
            projection["props"][0]["key"]["content"],
            json!("onContextmenu")
        );
        assert_eq!(
            projection["props"][0]["value"]["callee"],
            json!("V_ON_WITH_MODIFIERS")
        );
        assert_eq!(
            projection["props"][0]["value"]["arguments"][1],
            json!("[\"right\"]")
        );
    }

    #[test]
    fn transform_on_projection_preserves_constant_handler_metadata() {
        let projection = transform_on_projection(&json!({
            "dir": {
                "name": "on",
                "arg": {
                    "type": 4,
                    "content": "keydown",
                    "isStatic": true,
                    "loc": { "source": "keydown" }
                },
                "exp": {
                    "type": 4,
                    "content": "foo",
                    "isStatic": false,
                    "constType": 0,
                    "loc": { "source": "foo" }
                },
                "modifiers": [{ "content": "up" }],
                "loc": { "source": "@keydown.up=\"foo\"" }
            },
            "node": { "type": 1, "tag": "div", "tagType": 0 },
            "context": {
                "prefixIdentifiers": true,
                "bindingMetadata": { "foo": "setup-const" }
            }
        }));

        assert_eq!(
            projection["props"][0]["value"]["callee"],
            json!("V_ON_WITH_KEYS")
        );
        assert_eq!(projection["props"][0]["valueConstant"], json!(true));
    }
}
