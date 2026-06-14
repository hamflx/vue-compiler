#[derive(Clone, Debug)]
pub(crate) struct StaticHtmlAnalysis {
    pub(crate) html: StaticHtmlBuffer,
    pub(crate) dom_nodes: usize,
    pub(crate) node_count: usize,
    pub(crate) element_with_binding_count: usize,
}

impl StaticHtmlAnalysis {
    pub(crate) fn append(&mut self, other: StaticHtmlAnalysis) {
        self.html.append(other.html);
        self.dom_nodes += other.dom_nodes;
        self.node_count += other.node_count;
        self.element_with_binding_count += other.element_with_binding_count;
    }

    pub(crate) fn meets_threshold(&self) -> bool {
        self.node_count >= STRINGIFY_STATIC_NODE_COUNT
            || self.element_with_binding_count >= STRINGIFY_STATIC_ELEMENT_WITH_BINDING_COUNT
    }

    pub(crate) fn render_static_call(&self) -> String {
        format!(
            "_createStaticVNode({}, {})",
            self.html.to_js_expression(),
            self.dom_nodes
        )
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct StaticHtmlBuffer {
    pub(crate) parts: Vec<StaticHtmlPart>,
}

#[derive(Clone, Debug)]
pub(crate) enum StaticHtmlPart {
    Text(String),
    Expression(String),
}

impl StaticHtmlBuffer {
    pub(crate) fn from_text(value: impl Into<String>) -> Self {
        let mut buffer = Self::default();
        buffer.push_text(value);
        buffer
    }

    pub(crate) fn push_text(&mut self, value: impl Into<String>) {
        let value = value.into();
        if value.is_empty() {
            return;
        }
        match self.parts.last_mut() {
            Some(StaticHtmlPart::Text(existing)) => existing.push_str(&value),
            _ => self.parts.push(StaticHtmlPart::Text(value)),
        }
    }

    pub(crate) fn push_expression(&mut self, value: impl Into<String>) {
        let value = value.into();
        if value.trim().is_empty() {
            return;
        }
        self.parts.push(StaticHtmlPart::Expression(value));
    }

    pub(crate) fn append(&mut self, other: Self) {
        for part in other.parts {
            match part {
                StaticHtmlPart::Text(value) => self.push_text(value),
                StaticHtmlPart::Expression(value) => self.push_expression(value),
            }
        }
    }

    pub(crate) fn to_js_expression(&self) -> String {
        let parts = self
            .parts
            .iter()
            .filter_map(|part| match part {
                StaticHtmlPart::Text(value) if !value.is_empty() => Some(quote_string(value)),
                StaticHtmlPart::Text(_) => None,
                StaticHtmlPart::Expression(value) => Some(value.clone()),
            })
            .collect::<Vec<_>>();
        if parts.is_empty() {
            quote_string("")
        } else {
            parts.join(" + ")
        }
    }
}

pub(crate) fn render_static_vnode_cache(
    ast: &Vue3Ast,
    children: &[&vuec_ast::Node<Vue3NodeKind>],
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> Option<String> {
    let analysis = analyze_static_html_chunk(ast, children, options, scope)?;
    if analysis.meets_threshold() {
        Some(analysis.render_static_call())
    } else {
        None
    }
}

pub(crate) fn render_root_static_vnode_cache(
    ast: &Vue3Ast,
    children: &[vuec_ast::NodeId],
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> Option<String> {
    if visible_child_ids(ast, children).len() < 2 {
        return None;
    }
    let child_nodes = children
        .iter()
        .filter_map(|child_id| ast.node(*child_id))
        .collect::<Vec<_>>();
    let analysis = analyze_static_html_chunk(ast, &child_nodes, options, scope)?;
    (analysis.dom_nodes > 1 || analysis.meets_threshold()).then(|| analysis.render_static_call())
}

pub(crate) fn render_static_vnode_chunked_children(
    ast: &Vue3Ast,
    children: &[&vuec_ast::Node<Vue3NodeKind>],
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
    regular_mode: NodeRenderMode,
    memo_index: &mut MemoIndex,
) -> Option<Vec<String>> {
    let chunks = static_vnode_chunks(ast, children, options, scope);
    if chunks.is_empty() {
        return None;
    }

    let mut rendered = Vec::new();
    let mut cursor = 0usize;
    for chunk in chunks {
        render_static_vnode_regular_segment(
            ast,
            children,
            cursor,
            chunk.start,
            options,
            scope,
            regular_mode,
            memo_index,
            &mut rendered,
        );
        rendered.push(chunk.call);
        cursor = chunk.end;
    }
    render_static_vnode_regular_segment(
        ast,
        children,
        cursor,
        children.len(),
        options,
        scope,
        regular_mode,
        memo_index,
        &mut rendered,
    );
    Some(rendered)
}

pub(crate) fn collect_static_hoists(ast: &Vue3Ast, options: &Vue3CompilerOptions) -> StaticHoists {
    let mut hoists = StaticHoists::default();
    if !options.hoist_static {
        return hoists;
    }
    let stringified_nodes = collect_stringified_static_node_ids(ast, options);
    let do_not_hoist_root = ast
        .root_node()
        .and_then(|root| vue3_single_static_root_child(&root.children, ast));
    collect_static_hoists_for_node(
        ast,
        ast.root,
        options,
        do_not_hoist_root,
        &stringified_nodes,
        &mut hoists,
    );
    hoists
}

pub(crate) fn collect_static_hoists_for_node(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    options: &Vue3CompilerOptions,
    do_not_hoist_root: Option<vuec_ast::NodeId>,
    stringified_nodes: &BTreeSet<vuec_ast::NodeId>,
    hoists: &mut StaticHoists,
) {
    let Some(node) = ast.node(node_id) else {
        return;
    };
    if let Vue3AstKind::Element(element) = &node.kind {
        collect_static_asset_binding_hoists(node_id, element, hoists);
        if !stringified_nodes.contains(&node_id)
            && static_props_should_hoist_element(ast, node, element, options, do_not_hoist_root)
        {
            hoists.push_props_object(node_id);
        }
    }
    if stringified_nodes.contains(&node_id) {
        for child_id in &node.children {
            collect_static_asset_binding_hoists_for_subtree(ast, *child_id, hoists);
        }
        return;
    }
    for child_id in &node.children {
        collect_static_hoists_for_node(
            ast,
            *child_id,
            options,
            do_not_hoist_root,
            stringified_nodes,
            hoists,
        );
    }
}

pub(crate) fn collect_static_asset_binding_hoists_for_subtree(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    hoists: &mut StaticHoists,
) {
    let Some(node) = ast.node(node_id) else {
        return;
    };
    if let Vue3AstKind::Element(element) = &node.kind {
        collect_static_asset_binding_hoists(node_id, element, hoists);
    }
    for child_id in &node.children {
        collect_static_asset_binding_hoists_for_subtree(ast, *child_id, hoists);
    }
}

pub(crate) fn collect_stringified_static_node_ids(
    ast: &Vue3Ast,
    options: &Vue3CompilerOptions,
) -> BTreeSet<vuec_ast::NodeId> {
    let mut ids = BTreeSet::new();
    if !options.stringify_static {
        return ids;
    }
    let scope = RenderScope::default();
    if let Some(root) = ast.root_node() {
        let root_children = root
            .children
            .iter()
            .filter_map(|child_id| ast.node(*child_id))
            .collect::<Vec<_>>();
        if visible_child_ids(ast, &root.children).len() >= 2
            && analyze_static_html_chunk(ast, &root_children, options, &scope)
                .is_some_and(|analysis| analysis.dom_nodes > 1 || analysis.meets_threshold())
        {
            for child in &root_children {
                collect_static_subtree_ids(ast, child.id, &mut ids);
            }
        }
    }
    collect_stringified_static_node_ids_for_parent(ast, ast.root, options, &scope, &mut ids);
    ids
}

pub(crate) fn collect_stringified_static_node_ids_for_parent(
    ast: &Vue3Ast,
    parent_id: vuec_ast::NodeId,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
    ids: &mut BTreeSet<vuec_ast::NodeId>,
) {
    let Some(parent) = ast.node(parent_id) else {
        return;
    };
    let children = parent
        .children
        .iter()
        .filter_map(|child_id| ast.node(*child_id))
        .collect::<Vec<_>>();
    for chunk in static_vnode_chunks(ast, &children, options, scope) {
        for child in &children[chunk.start..chunk.end] {
            collect_static_subtree_ids(ast, child.id, ids);
        }
    }
    for child_id in &parent.children {
        if !ids.contains(child_id) {
            collect_stringified_static_node_ids_for_parent(ast, *child_id, options, scope, ids);
        }
    }
}

pub(crate) fn collect_static_subtree_ids(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    ids: &mut BTreeSet<vuec_ast::NodeId>,
) {
    if !ids.insert(node_id) {
        return;
    }
    if let Some(node) = ast.node(node_id) {
        for child_id in &node.children {
            collect_static_subtree_ids(ast, *child_id, ids);
        }
    }
}

pub(crate) fn collect_static_asset_binding_hoists(
    node_id: vuec_ast::NodeId,
    element: &Vue3Element,
    hoists: &mut StaticHoists,
) {
    for (prop_index, prop) in element.props.iter().enumerate() {
        let Vue3Prop::Directive(dir) = prop else {
            continue;
        };
        let Some(expression) = static_asset_binding_expression_to_hoist(dir) else {
            continue;
        };
        let key = render_static_binding_prop_key(dir);
        hoists.push_binding(node_id, prop_index, expression, key != "srcset");
    }
}

pub(crate) fn static_asset_binding_expression_to_hoist(dir: &Vue3Directive) -> Option<String> {
    if dir.name != "bind" || dir.is_dynamic_arg || !dir.modifiers.is_empty() {
        return None;
    }
    let key = render_static_binding_prop_key(dir);
    let expression = dir.exp.as_ref()?.source_string();
    let expression = expression.trim();
    if !expression_is_generated_asset_import(expression) {
        return None;
    }
    (key == "srcset" || generated_asset_import_expression_has_literal(expression))
        .then(|| expression.to_string())
}

pub(crate) fn static_props_should_hoist_element(
    ast: &Vue3Ast,
    node: &vuec_ast::Node<Vue3NodeKind>,
    element: &Vue3Element,
    _options: &Vue3CompilerOptions,
    do_not_hoist_root: Option<vuec_ast::NodeId>,
) -> bool {
    if element.props.is_empty()
        || element.tag_type != Vue3ElementType::Element
        || directive_by_name(element, "if").is_some()
        || directive_by_name(element, "else").is_some()
        || directive_by_name(element, "else-if").is_some()
        || directive_by_name(element, "for").is_some()
        || !element
            .props
            .iter()
            .all(vue3_prop_is_static_cacheable_for_hoist)
    {
        return false;
    }
    do_not_hoist_root == Some(node.id) || !is_static_element_tree_for_cache(ast, node)
}

pub(crate) fn static_hoist_declarations(
    ast: &Vue3Ast,
    options: &Vue3CompilerOptions,
    hoists: &StaticHoists,
) -> Vec<String> {
    let scope = RenderScope::default().with_static_hoists(hoists.clone());
    hoists
        .declarations
        .iter()
        .enumerate()
        .filter_map(|(declaration_index, declaration)| {
            let index = declaration_index + 1;
            match declaration {
                StaticHoistDeclaration::BindingExpression { expression } => {
                    Some(format!("const _hoisted_{index} = {expression}"))
                }
                StaticHoistDeclaration::PropsObject { node_id } => {
                    let node = ast.node(*node_id)?;
                    let Vue3AstKind::Element(element) = &node.kind else {
                        return None;
                    };
                    let props =
                        render_static_props_hoist_object(*node_id, element, options, &scope)?;
                    Some(format!("const _hoisted_{index} = {props}"))
                }
            }
        })
        .collect()
}

#[derive(Clone, Debug)]
pub(crate) struct StaticVNodeChunk {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) call: String,
}

pub(crate) fn static_vnode_chunks(
    ast: &Vue3Ast,
    children: &[&vuec_ast::Node<Vue3NodeKind>],
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> Vec<StaticVNodeChunk> {
    let mut chunks = Vec::new();
    let mut start = 0usize;
    let mut chunk_analysis = None::<StaticHtmlAnalysis>;
    let mut blocked_by_comment = false;

    for (index, child) in children.iter().enumerate() {
        if matches!(child.kind, Vue3AstKind::Comment(_)) {
            blocked_by_comment = true;
        }
        if let Some(analysis) = analyze_static_html_chunk(ast, &[*child], options, scope) {
            if chunk_analysis.is_none() {
                start = index;
            }
            match chunk_analysis.as_mut() {
                Some(existing) => existing.append(analysis),
                None => chunk_analysis = Some(analysis),
            }
            continue;
        }
        push_static_vnode_chunk(
            &mut chunks,
            start,
            index,
            &mut chunk_analysis,
            blocked_by_comment,
        );
        blocked_by_comment = false;
    }
    push_static_vnode_chunk(
        &mut chunks,
        start,
        children.len(),
        &mut chunk_analysis,
        blocked_by_comment,
    );
    chunks
}

pub(crate) fn push_static_vnode_chunk(
    chunks: &mut Vec<StaticVNodeChunk>,
    start: usize,
    end: usize,
    chunk_analysis: &mut Option<StaticHtmlAnalysis>,
    blocked_by_comment: bool,
) {
    let Some(analysis) = chunk_analysis.take() else {
        return;
    };
    if analysis.meets_threshold() && !blocked_by_comment {
        chunks.push(StaticVNodeChunk {
            start,
            end,
            call: analysis.render_static_call(),
        });
    }
}

pub(crate) fn render_static_vnode_regular_segment(
    ast: &Vue3Ast,
    children: &[&vuec_ast::Node<Vue3NodeKind>],
    start: usize,
    end: usize,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
    mode: NodeRenderMode,
    memo_index: &mut MemoIndex,
    rendered: &mut Vec<String>,
) {
    if start >= end {
        return;
    }
    let ids = children[start..end]
        .iter()
        .map(|child| child.id)
        .collect::<Vec<_>>();
    rendered.extend(render_child_sequence(
        ast, &ids, options, mode, scope, memo_index,
    ));
}

pub(crate) fn analyze_static_html_chunk(
    ast: &Vue3Ast,
    children: &[&vuec_ast::Node<Vue3NodeKind>],
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> Option<StaticHtmlAnalysis> {
    if children.is_empty() {
        return None;
    }
    let mut analysis = StaticHtmlAnalysis {
        html: StaticHtmlBuffer::default(),
        dom_nodes: children.len(),
        node_count: 0,
        element_with_binding_count: 0,
    };
    for child in children {
        analysis
            .html
            .append(static_html_for_node(ast, child, options, scope)?);
    }
    accumulate_static_html_analysis(ast, children, &mut analysis)?;
    Some(analysis)
}

pub(crate) fn static_html_for_node(
    ast: &Vue3Ast,
    node: &vuec_ast::Node<Vue3NodeKind>,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> Option<StaticHtmlBuffer> {
    match &node.kind {
        Vue3AstKind::Element(element) => {
            static_html_for_element(ast, node, element, options, scope)
        }
        Vue3AstKind::Text(text) => Some(StaticHtmlBuffer::from_text(escape_static_html_text(
            &text.value,
        ))),
        Vue3AstKind::Interpolation(interpolation) => {
            let value = static_const_eval_source(&interpolation.expression.source_string())?;
            Some(StaticHtmlBuffer::from_text(escape_static_html_text(
                &value.to_display_string()?,
            )))
        }
        _ => None,
    }
}

pub(crate) fn static_html_for_element(
    ast: &Vue3Ast,
    node: &vuec_ast::Node<Vue3NodeKind>,
    element: &Vue3Element,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> Option<StaticHtmlBuffer> {
    if element.tag == "slot"
        || element.tag_type != Vue3ElementType::Element
        || (element.ns == vuec_ast::HtmlNamespace::Html
            && static_html_non_stringifiable_tag(&element.tag))
        || (element.ns == vuec_ast::HtmlNamespace::Html
            && static_html_is_void_tag(&element.tag)
            && !node.children.is_empty())
        || static_html_has_invalid_inner_html_placement(ast, node, element)
        || directive_by_name(element, "once").is_some()
    {
        return None;
    }

    let mut html = StaticHtmlBuffer::default();
    html.push_text("<");
    html.push_text(element.tag.as_str());
    let mut inner_html = None;
    for (prop_index, prop) in element.props.iter().enumerate() {
        match prop {
            Vue3Prop::Attribute(attr) => {
                if !static_html_attr_is_stringifiable(&attr.name, element.ns) {
                    return None;
                }
                html.push_text(" ");
                html.push_text(attr.name.as_str());
                if let Some(value) = &attr.value {
                    html.push_text("=\"");
                    html.push_text(escape_static_html_attr(value));
                    html.push_text("\"");
                }
            }
            Vue3Prop::Directive(dir) if dir.name == "html" => {
                let source = dir.exp.as_ref()?.source_string();
                let value = static_const_eval_source(&source)?;
                inner_html = Some(decode_static_html_entities(&value.to_display_string()?));
            }
            Vue3Prop::Directive(dir) if dir.name == "text" => {
                let source = dir.exp.as_ref()?.source_string();
                let value = static_const_eval_source(&source)?;
                inner_html = Some(escape_static_html_text(&value.to_display_string()?));
            }
            Vue3Prop::Directive(dir) => {
                let Some(rendered) = static_html_directive_attr(
                    &element.tag,
                    element.ns,
                    node.id,
                    prop_index,
                    dir,
                    scope,
                )?
                else {
                    continue;
                };
                html.push_text(" ");
                html.push_text(rendered.name.as_str());
                html.push_text("=\"");
                html.append(rendered.value);
                html.push_text("\"");
            }
        }
    }
    if let Some(scope_id) = options
        .scope_id
        .as_deref()
        .filter(|scope_id| !scope_id.is_empty())
    {
        html.push_text(" ");
        html.push_text(scope_id);
    }
    html.push_text(">");

    if element.ns != vuec_ast::HtmlNamespace::Html || !static_html_is_void_tag(&element.tag) {
        if let Some(inner_html) = inner_html.filter(|value| !value.is_empty()) {
            html.push_text(inner_html);
        } else {
            for child_id in &node.children {
                let child = ast.node(*child_id)?;
                html.append(static_html_for_node(ast, child, options, scope)?);
            }
        }
        html.push_text("</");
        html.push_text(element.tag.as_str());
        html.push_text(">");
    }

    Some(html)
}

#[derive(Clone, Debug)]
pub(crate) struct StaticHtmlAttr {
    pub(crate) name: String,
    pub(crate) value: StaticHtmlBuffer,
}

pub(crate) fn static_html_directive_attr(
    tag: &str,
    ns: vuec_ast::HtmlNamespace,
    node_id: vuec_ast::NodeId,
    prop_index: usize,
    dir: &Vue3Directive,
    scope: &RenderScope,
) -> Option<Option<StaticHtmlAttr>> {
    match dir.name.as_str() {
        "bind" => static_html_bind_attr(tag, ns, node_id, prop_index, dir, scope),
        "html" | "text" => None,
        _ => None,
    }
}

pub(crate) fn static_html_bind_attr(
    tag: &str,
    ns: vuec_ast::HtmlNamespace,
    node_id: vuec_ast::NodeId,
    prop_index: usize,
    dir: &Vue3Directive,
    scope: &RenderScope,
) -> Option<Option<StaticHtmlAttr>> {
    if dir.is_dynamic_arg || !dir.modifiers.is_empty() {
        return None;
    }
    let name = dir.arg.as_ref()?.source_string();
    if !static_html_attr_is_stringifiable(&name, ns) {
        return None;
    }
    if ns == vuec_ast::HtmlNamespace::Html && tag == "option" && name == "value" {
        return None;
    }
    let source = dir.exp.as_ref()?.source_string();
    if is_asset_import_binding(dir) {
        if let Some(index) = scope.static_hoists.binding_index(node_id, prop_index) {
            let mut value = StaticHtmlBuffer::default();
            value.push_expression(format!("_hoisted_{index}"));
            return Some(Some(StaticHtmlAttr { name, value }));
        }
        return Some(Some(StaticHtmlAttr {
            name,
            value: static_html_asset_import_expression(&source)?,
        }));
    }
    let value = static_const_eval_source(&source)?;
    if matches!(value, StaticConstValue::Null) {
        return Some(None);
    }
    if static_html_is_boolean_attr(&name) && matches!(value, StaticConstValue::Bool(false)) {
        return Some(None);
    }
    let value = if name == "class" {
        static_const_normalize_class(&value)?
    } else if name == "style" {
        static_const_stringify_style(&value)?
    } else {
        value.to_display_string()?
    };
    Some(Some(StaticHtmlAttr {
        name,
        value: StaticHtmlBuffer::from_text(escape_static_html_attr(&value)),
    }))
}

pub(crate) fn accumulate_static_html_analysis(
    ast: &Vue3Ast,
    children: &[&vuec_ast::Node<Vue3NodeKind>],
    analysis: &mut StaticHtmlAnalysis,
) -> Option<()> {
    for child in children {
        match &child.kind {
            Vue3AstKind::Element(element) => {
                analysis.node_count += 1;
                if !element.props.is_empty() {
                    analysis.element_with_binding_count += 1;
                }
                let descendants = child
                    .children
                    .iter()
                    .filter_map(|child_id| ast.node(*child_id))
                    .collect::<Vec<_>>();
                accumulate_static_html_analysis(ast, &descendants, analysis)?;
            }
            Vue3AstKind::Text(_) | Vue3AstKind::Interpolation(_) => {
                analysis.node_count += 1;
            }
            _ => return None,
        }
    }
    Some(())
}

pub(crate) const STATIC_HTML_KNOWN_HTML_ATTRS: &str = "accept,accept-charset,accesskey,action,align,allow,alt,async,autocapitalize,autocomplete,autofocus,autoplay,background,bgcolor,border,buffered,capture,challenge,charset,checked,cite,class,code,codebase,color,cols,colspan,content,contenteditable,contextmenu,controls,coords,crossorigin,csp,data,datetime,decoding,default,defer,dir,dirname,disabled,download,draggable,dropzone,enctype,enterkeyhint,for,form,formaction,formenctype,formmethod,formnovalidate,formtarget,headers,height,hidden,high,href,hreflang,http-equiv,icon,id,importance,inert,integrity,ismap,itemprop,keytype,kind,label,lang,language,loading,list,loop,low,manifest,max,maxlength,minlength,media,min,multiple,muted,name,novalidate,open,optimum,pattern,ping,placeholder,poster,preload,radiogroup,readonly,referrerpolicy,rel,required,reversed,rows,rowspan,sandbox,scope,scoped,selected,shape,size,sizes,slot,span,spellcheck,src,srcdoc,srclang,srcset,start,step,style,summary,tabindex,target,title,translate,type,usemap,value,width,wrap";

pub(crate) const STATIC_HTML_KNOWN_SVG_ATTRS: &str = "xmlns,accent-height,accumulate,additive,alignment-baseline,alphabetic,amplitude,arabic-form,ascent,attributeName,attributeType,azimuth,baseFrequency,baseline-shift,baseProfile,bbox,begin,bias,by,calcMode,cap-height,class,clip,clipPathUnits,clip-path,clip-rule,color,color-interpolation,color-interpolation-filters,color-profile,color-rendering,contentScriptType,contentStyleType,crossorigin,cursor,cx,cy,d,decelerate,descent,diffuseConstant,direction,display,divisor,dominant-baseline,dur,dx,dy,edgeMode,elevation,enable-background,end,exponent,fill,fill-opacity,fill-rule,filter,filterRes,filterUnits,flood-color,flood-opacity,font-family,font-size,font-size-adjust,font-stretch,font-style,font-variant,font-weight,format,from,fr,fx,fy,g1,g2,glyph-name,glyph-orientation-horizontal,glyph-orientation-vertical,glyphRef,gradientTransform,gradientUnits,hanging,height,href,hreflang,horiz-adv-x,horiz-origin-x,id,ideographic,image-rendering,in,in2,intercept,k,k1,k2,k3,k4,kernelMatrix,kernelUnitLength,kerning,keyPoints,keySplines,keyTimes,lang,lengthAdjust,letter-spacing,lighting-color,limitingConeAngle,local,marker-end,marker-mid,marker-start,markerHeight,markerUnits,markerWidth,mask,maskContentUnits,maskUnits,mathematical,max,media,method,min,mode,name,numOctaves,offset,opacity,operator,order,orient,orientation,origin,overflow,overline-position,overline-thickness,panose-1,paint-order,path,pathLength,patternContentUnits,patternTransform,patternUnits,ping,pointer-events,points,pointsAtX,pointsAtY,pointsAtZ,preserveAlpha,preserveAspectRatio,primitiveUnits,r,radius,referrerPolicy,refX,refY,rel,rendering-intent,repeatCount,repeatDur,requiredExtensions,requiredFeatures,restart,result,rotate,rx,ry,scale,seed,shape-rendering,slope,spacing,specularConstant,specularExponent,speed,spreadMethod,startOffset,stdDeviation,stemh,stemv,stitchTiles,stop-color,stop-opacity,strikethrough-position,strikethrough-thickness,string,stroke,stroke-dasharray,stroke-dashoffset,stroke-linecap,stroke-linejoin,stroke-miterlimit,stroke-opacity,stroke-width,style,surfaceScale,systemLanguage,tabindex,tableValues,target,targetX,targetY,text-anchor,text-decoration,text-rendering,textLength,to,transform,transform-origin,type,u1,u2,underline-position,underline-thickness,unicode,unicode-bidi,unicode-range,units-per-em,v-alphabetic,v-hanging,v-ideographic,v-mathematical,values,vector-effect,version,vert-adv-y,vert-origin-x,vert-origin-y,viewBox,viewTarget,visibility,width,widths,word-spacing,writing-mode,x,x-height,x1,x2,xChannelSelector,xlink:actuate,xlink:arcrole,xlink:href,xlink:role,xlink:show,xlink:title,xlink:type,xmlns:xlink,xml:base,xml:lang,xml:space,y,y1,y2,yChannelSelector,z,zoomAndPan";

pub(crate) const STATIC_HTML_KNOWN_MATHML_ATTRS: &str = "accent,accentunder,actiontype,align,alignmentscope,altimg,altimg-height,altimg-valign,altimg-width,alttext,bevelled,close,columnsalign,columnlines,columnspan,denomalign,depth,dir,display,displaystyle,encoding,equalcolumns,equalrows,fence,fontstyle,fontweight,form,frame,framespacing,groupalign,height,href,id,indentalign,indentalignfirst,indentalignlast,indentshift,indentshiftfirst,indentshiftlast,indextype,justify,largetop,largeop,lquote,lspace,mathbackground,mathcolor,mathsize,mathvariant,maxsize,minlabelspacing,mode,other,overflow,position,rowalign,rowlines,rowspan,rquote,rspace,scriptlevel,scriptminsize,scriptsizemultiplier,selection,separator,separators,shift,side,src,stackalign,stretchy,subscriptshift,superscriptshift,symmetric,voffset,width,widths,xlink:href,xlink:show,xlink:type,xmlns";

pub(crate) fn static_html_attr_is_stringifiable(name: &str, ns: vuec_ast::HtmlNamespace) -> bool {
    name.starts_with("data-")
        || name.starts_with("aria-")
        || match ns {
            vuec_ast::HtmlNamespace::Html => {
                static_html_known_attr_contains(STATIC_HTML_KNOWN_HTML_ATTRS, name)
            }
            vuec_ast::HtmlNamespace::Svg => {
                static_html_known_attr_contains(STATIC_HTML_KNOWN_SVG_ATTRS, name)
            }
            vuec_ast::HtmlNamespace::MathMl => {
                static_html_known_attr_contains(STATIC_HTML_KNOWN_MATHML_ATTRS, name)
            }
        }
}

pub(crate) fn static_html_known_attr_contains(attrs: &str, name: &str) -> bool {
    attrs.split(',').any(|attr| attr == name)
}

pub(crate) fn static_html_is_boolean_attr(name: &str) -> bool {
    matches!(
        name,
        "allowfullscreen"
            | "async"
            | "autofocus"
            | "autoplay"
            | "checked"
            | "controls"
            | "default"
            | "defer"
            | "disabled"
            | "formnovalidate"
            | "hidden"
            | "inert"
            | "ismap"
            | "itemscope"
            | "loop"
            | "multiple"
            | "muted"
            | "nomodule"
            | "novalidate"
            | "open"
            | "readonly"
            | "required"
            | "reversed"
            | "selected"
    )
}

pub(crate) fn static_html_non_stringifiable_tag(tag: &str) -> bool {
    matches!(
        tag,
        "caption" | "thead" | "tr" | "th" | "tbody" | "td" | "tfoot" | "colgroup" | "col"
    )
}

pub(crate) fn static_html_has_invalid_inner_html_placement(
    ast: &Vue3Ast,
    node: &vuec_ast::Node<Vue3NodeKind>,
    element: &Vue3Element,
) -> bool {
    element.ns == vuec_ast::HtmlNamespace::Html
        && element.tag.eq_ignore_ascii_case("p")
        && node
            .children
            .iter()
            .any(|child_id| static_html_contains_invalid_p_descendant(ast, *child_id))
}

pub(crate) fn static_html_contains_invalid_p_descendant(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
) -> bool {
    let Some(node) = ast.node(node_id) else {
        return false;
    };
    let Vue3AstKind::Element(element) = &node.kind else {
        return false;
    };
    if element.ns != vuec_ast::HtmlNamespace::Html {
        return false;
    }
    static_html_is_invalid_p_child_tag(&element.tag)
        || node
            .children
            .iter()
            .any(|child_id| static_html_contains_invalid_p_descendant(ast, *child_id))
}

pub(crate) fn static_html_is_invalid_p_child_tag(tag: &str) -> bool {
    static_html_tag_list_contains(STATIC_HTML_INVALID_P_CHILD_TAGS, tag)
}

pub(crate) const STATIC_HTML_INVALID_P_CHILD_TAGS: &str = "address,article,aside,blockquote,center,details,dialog,dir,div,dl,fieldset,figure,footer,form,h1,h2,h3,h4,h5,h6,header,hgroup,hr,li,main,nav,menu,ol,p,pre,section,table,ul";

pub(crate) fn static_html_tag_list_contains(tags: &str, tag: &str) -> bool {
    tags.split(',')
        .any(|candidate| candidate.eq_ignore_ascii_case(tag))
}

pub(crate) fn static_html_is_void_tag(tag: &str) -> bool {
    matches!(
        tag,
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
    )
}

pub(crate) fn escape_static_html_text(value: &str) -> String {
    escape_static_html(value, false)
}

pub(crate) fn escape_static_html_attr(value: &str) -> String {
    escape_static_html(value, true)
}

pub(crate) fn decode_static_html_entities(value: &str) -> String {
    value
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

pub(crate) fn escape_static_html(value: &str, attr: bool) -> String {
    let mut output = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            '"' if attr => output.push_str("&quot;"),
            _ => output.push(ch),
        }
    }
    output
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum StaticConstValue {
    String(String),
    Number(String),
    Bool(bool),
    Null,
    Array(Vec<StaticConstValue>),
    Object(Vec<(String, StaticConstValue)>),
}

impl StaticConstValue {
    pub(crate) fn to_display_string(&self) -> Option<String> {
        match self {
            Self::String(value) | Self::Number(value) => Some(value.clone()),
            Self::Bool(true) => Some("true".into()),
            Self::Bool(false) => Some("false".into()),
            Self::Null => Some(String::new()),
            Self::Array(_) | Self::Object(_) => None,
        }
    }

    pub(crate) fn to_js_string(&self) -> Option<String> {
        match self {
            Self::String(value) | Self::Number(value) => Some(value.clone()),
            Self::Bool(true) => Some("true".into()),
            Self::Bool(false) => Some("false".into()),
            Self::Null => Some("null".into()),
            Self::Array(_) | Self::Object(_) => None,
        }
    }

    pub(crate) fn truthy(&self) -> bool {
        match self {
            Self::String(value) => !value.is_empty(),
            Self::Number(value) => !matches!(value.as_str(), "0" | "-0" | "NaN"),
            Self::Bool(value) => *value,
            Self::Null => false,
            Self::Array(_) | Self::Object(_) => true,
        }
    }
}

pub(crate) fn static_const_eval_source(source: &str) -> Option<StaticConstValue> {
    let store = JsAstStore::new();
    let expression = store
        .parse_expression(source.trim(), oxc_span::SourceType::ts())
        .ok()?;
    static_const_eval_expression(&expression)
}

pub(crate) fn static_const_eval_expression(
    expression: &Expression<'_>,
) -> Option<StaticConstValue> {
    match expression {
        Expression::StringLiteral(literal) => {
            Some(StaticConstValue::String(literal.value.as_str().to_string()))
        }
        Expression::NumericLiteral(literal) => Some(StaticConstValue::Number(
            static_const_number_string(literal.value),
        )),
        Expression::BooleanLiteral(literal) => Some(StaticConstValue::Bool(literal.value)),
        Expression::NullLiteral(_) => Some(StaticConstValue::Null),
        Expression::TemplateLiteral(literal) => {
            if !literal.expressions.is_empty() || literal.quasis.len() != 1 {
                return None;
            }
            let cooked = literal.quasis.first()?.value.cooked.as_ref()?;
            Some(StaticConstValue::String(cooked.as_str().to_string()))
        }
        Expression::ParenthesizedExpression(expression) => {
            static_const_eval_expression(&expression.expression)
        }
        Expression::TSAsExpression(expression) => {
            static_const_eval_expression(&expression.expression)
        }
        Expression::TSSatisfiesExpression(expression) => {
            static_const_eval_expression(&expression.expression)
        }
        Expression::TSTypeAssertion(expression) => {
            static_const_eval_expression(&expression.expression)
        }
        Expression::TSNonNullExpression(expression) => {
            static_const_eval_expression(&expression.expression)
        }
        Expression::TSInstantiationExpression(expression) => {
            static_const_eval_expression(&expression.expression)
        }
        Expression::UnaryExpression(expression) => static_const_eval_unary(expression),
        Expression::BinaryExpression(expression) => static_const_eval_binary(expression),
        Expression::ArrayExpression(expression) => {
            let mut values = Vec::new();
            for element in &expression.elements {
                values.push(static_const_eval_array_element(element)?);
            }
            Some(StaticConstValue::Array(values))
        }
        Expression::ObjectExpression(expression) => {
            let mut values = Vec::new();
            for property in &expression.properties {
                let ObjectPropertyKind::ObjectProperty(property) = property else {
                    return None;
                };
                if property.kind != PropertyKind::Init
                    || property.method
                    || property.shorthand
                    || property.computed
                {
                    return None;
                }
                let key = static_const_property_key(&property.key)?;
                let value = static_const_eval_expression(&property.value)?;
                values.push((key, value));
            }
            Some(StaticConstValue::Object(values))
        }
        _ => None,
    }
}

pub(crate) fn static_const_eval_array_element(
    element: &ArrayExpressionElement<'_>,
) -> Option<StaticConstValue> {
    if element.is_elision() || element.is_spread() {
        return None;
    }
    static_const_eval_expression(element.as_expression()?)
}

pub(crate) fn static_const_eval_unary(
    expression: &oxc_ast::ast::UnaryExpression<'_>,
) -> Option<StaticConstValue> {
    let value = static_const_eval_expression(&expression.argument)?;
    match expression.operator {
        UnaryOperator::LogicalNot => Some(StaticConstValue::Bool(!value.truthy())),
        UnaryOperator::UnaryPlus => Some(StaticConstValue::Number(static_const_number_string(
            static_const_to_number(&value)?,
        ))),
        UnaryOperator::UnaryNegation => Some(StaticConstValue::Number(static_const_number_string(
            -static_const_to_number(&value)?,
        ))),
        _ => None,
    }
}

pub(crate) fn static_const_eval_binary(
    expression: &oxc_ast::ast::BinaryExpression<'_>,
) -> Option<StaticConstValue> {
    if expression.operator != BinaryOperator::Addition {
        return None;
    }
    let left = static_const_eval_expression(&expression.left)?;
    let right = static_const_eval_expression(&expression.right)?;
    if matches!(left, StaticConstValue::String(_)) || matches!(right, StaticConstValue::String(_)) {
        Some(StaticConstValue::String(format!(
            "{}{}",
            left.to_js_string()?,
            right.to_js_string()?
        )))
    } else {
        Some(StaticConstValue::Number(static_const_number_string(
            static_const_to_number(&left)? + static_const_to_number(&right)?,
        )))
    }
}

pub(crate) fn static_const_to_number(value: &StaticConstValue) -> Option<f64> {
    match value {
        StaticConstValue::String(value) if value.trim().is_empty() => Some(0.0),
        StaticConstValue::String(value) => value.trim().parse::<f64>().ok(),
        StaticConstValue::Number(value) => value.parse::<f64>().ok(),
        StaticConstValue::Bool(true) => Some(1.0),
        StaticConstValue::Bool(false) | StaticConstValue::Null => Some(0.0),
        StaticConstValue::Array(_) | StaticConstValue::Object(_) => None,
    }
}

pub(crate) fn static_const_property_key(key: &PropertyKey<'_>) -> Option<String> {
    match key {
        PropertyKey::StaticIdentifier(identifier) => Some(identifier.name.as_str().to_string()),
        PropertyKey::StringLiteral(literal) => Some(literal.value.as_str().to_string()),
        PropertyKey::NumericLiteral(literal) => Some(static_const_number_string(literal.value)),
        _ => None,
    }
}

pub(crate) fn static_const_number_string(value: f64) -> String {
    if value.is_nan() {
        "NaN".into()
    } else if value.is_infinite() {
        if value.is_sign_negative() {
            "-Infinity".into()
        } else {
            "Infinity".into()
        }
    } else if value == 0.0 {
        "0".into()
    } else if value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        value.to_string()
    }
}

pub(crate) fn static_const_normalize_class(value: &StaticConstValue) -> Option<String> {
    match value {
        StaticConstValue::String(value) => Some(value.clone()),
        StaticConstValue::Array(items) => {
            let mut classes = Vec::new();
            for item in items {
                let normalized = static_const_normalize_class(item)?;
                if !normalized.is_empty() {
                    classes.push(normalized);
                }
            }
            Some(classes.join(" "))
        }
        StaticConstValue::Object(properties) => Some(
            properties
                .iter()
                .filter_map(|(key, value)| value.truthy().then(|| key.clone()))
                .collect::<Vec<_>>()
                .join(" "),
        ),
        StaticConstValue::Bool(_) | StaticConstValue::Number(_) | StaticConstValue::Null => {
            Some(String::new())
        }
    }
}

pub(crate) fn static_const_stringify_style(value: &StaticConstValue) -> Option<String> {
    match value {
        StaticConstValue::String(value) => {
            let style = vue3_parse_static_style(value);
            Some(static_const_stringify_style_entries(style))
        }
        StaticConstValue::Object(properties) => Some(static_const_stringify_style_entries(
            properties
                .iter()
                .filter_map(|(key, value)| {
                    value
                        .to_display_string()
                        .filter(|_| !matches!(value, StaticConstValue::Null))
                        .map(|value| (hyphenate_style_property(key), value))
                })
                .filter(|(_, value)| !value.is_empty())
                .collect(),
        )),
        StaticConstValue::Array(items) => {
            let mut entries = Vec::new();
            for item in items {
                match item {
                    StaticConstValue::String(value) => {
                        entries.extend(vue3_parse_static_style(value));
                    }
                    StaticConstValue::Object(properties) => {
                        entries.extend(properties.iter().filter_map(|(key, value)| {
                            value
                                .to_display_string()
                                .filter(|_| !matches!(value, StaticConstValue::Null))
                                .map(|value| (hyphenate_style_property(key), value))
                        }));
                    }
                    _ => return None,
                }
            }
            Some(static_const_stringify_style_entries(entries))
        }
        StaticConstValue::Bool(_) | StaticConstValue::Number(_) | StaticConstValue::Null => None,
    }
}

pub(crate) fn static_const_stringify_style_entries(entries: Vec<(String, String)>) -> String {
    entries
        .into_iter()
        .filter(|(key, value)| !key.is_empty() && !value.is_empty())
        .map(|(key, value)| format!("{key}:{value};"))
        .collect::<Vec<_>>()
        .join("")
}

pub(crate) fn hyphenate_style_property(value: &str) -> String {
    let mut output = String::new();
    for (index, ch) in value.chars().enumerate() {
        if ch.is_ascii_uppercase() {
            if index > 0 {
                output.push('-');
            }
            output.push(ch.to_ascii_lowercase());
        } else {
            output.push(ch);
        }
    }
    output
}

pub(crate) fn vue3_single_static_root_child(children: &[NodeId], ast: &Vue3Ast) -> Option<NodeId> {
    let mut element = None;
    for child_id in children {
        let Some(child) = ast.node(*child_id) else {
            continue;
        };
        if matches!(child.kind, Vue3AstKind::Comment(_)) {
            continue;
        }
        let Vue3AstKind::Element(element_kind) = &child.kind else {
            return None;
        };
        if element_kind.tag_type == Vue3ElementType::SlotOutlet {
            return None;
        }
        if element.replace(*child_id).is_some() {
            return None;
        }
    }
    element
}

pub(crate) fn vue3_dom_mir_can_hoist_static_node(ast: &Vue3Ast, node_id: NodeId) -> bool {
    let Some(node) = ast.node(node_id) else {
        return false;
    };
    let Vue3AstKind::Element(element) = &node.kind else {
        return false;
    };
    if element.tag == "slot"
        || element.tag_type != Vue3ElementType::Element
        || !element
            .props
            .iter()
            .all(vue3_prop_is_vnode_cacheable_static)
    {
        return false;
    }
    node.children.iter().all(|child_id| {
        let Some(child) = ast.node(*child_id) else {
            return false;
        };
        match &child.kind {
            Vue3AstKind::Text(_) | Vue3AstKind::Comment(_) => true,
            Vue3AstKind::Element(_) => vue3_dom_mir_can_hoist_static_node(ast, *child_id),
            _ => false,
        }
    })
}

pub(crate) fn child_sequence_is_direct_dynamic_text(
    ast: &Vue3Ast,
    children: &[vuec_ast::NodeId],
    options: &Vue3CompilerOptions,
) -> bool {
    let visible = children
        .iter()
        .filter_map(|child_id| ast.node(*child_id))
        .filter(|child| !matches!(child.kind, Vue3AstKind::Comment(_)))
        .collect::<Vec<_>>();
    !visible.is_empty()
        && visible.iter().all(|child| {
            matches!(
                child.kind,
                Vue3AstKind::Text(_) | Vue3AstKind::Interpolation(_)
            )
        })
        && !children_literal_const_only(ast, children, options)
        && visible
            .iter()
            .any(|child| matches!(child.kind, Vue3AstKind::Interpolation(_)))
}
