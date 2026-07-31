pub(crate) fn vue27_template_usage_check_string(template: &str, is_ts: bool) -> String {
    let mut code = String::new();
    let mut tokenizer = HtmlTokenizer::new(template);
    loop {
        let token = tokenizer.next_token();
        match token.kind {
            HtmlTokenKind::StartTag {
                name, attributes, ..
            } => {
                if !vue27_template_is_builtin_tag(&name) && !vue27_template_is_reserved_tag(&name) {
                    let camel = vue27_camelize(&name);
                    code.push(',');
                    code.push_str(&camel);
                    code.push(',');
                    code.push_str(&vue27_capitalize(&camel));
                }
                for attribute in attributes {
                    collect_vue27_template_attribute_usage(&mut code, &attribute, is_ts);
                }
            }
            HtmlTokenKind::Text(text) => {
                collect_vue27_template_text_usage(&mut code, &text, is_ts);
            }
            HtmlTokenKind::Eof => break,
            _ => {}
        }
    }
    code.push(';');
    code
}

pub(crate) fn collect_vue27_template_attribute_usage(
    code: &mut String,
    attr: &HtmlAttribute,
    is_ts: bool,
) {
    let name = attr.name.as_str();
    if vue27_template_is_directive_attr(name) {
        let base_name = vue27_template_directive_base_name(name);
        if !vue27_template_is_builtin_dir(&base_name) {
            code.push_str(",v");
            code.push_str(&vue27_capitalize(&vue27_camelize(&base_name)));
        }
        if let Some(value) = attr.value.as_deref() {
            code.push(',');
            code.push_str(&vue27_process_template_exp(value, is_ts, Some(&base_name)));
        }
    } else if name == "ref" {
        if let Some(value) = attr.value.as_deref() {
            code.push(',');
            code.push_str(value);
        }
    }
}

pub(crate) fn collect_vue27_template_text_usage(code: &mut String, text: &str, is_ts: bool) {
    let mut rest = text;
    while let Some(start) = rest.find("{{") {
        let after_start = &rest[start + 2..];
        let Some(end) = after_start.find("}}") else {
            break;
        };
        let expression = after_start[..end].trim();
        if !expression.is_empty() {
            code.push(',');
            code.push_str(&vue27_process_template_exp(expression, is_ts, None));
        }
        rest = &after_start[end + 2..];
    }
}

pub(crate) fn vue27_template_directive_base_name(name: &str) -> String {
    let body = if let Some(value) = name.strip_prefix("v-") {
        value
    } else if name.starts_with('@') {
        return "on".into();
    } else if name.starts_with('#') {
        return "slot".into();
    } else if name.starts_with(':') {
        return "bind".into();
    } else {
        name
    };
    body.split([':', '.', '['])
        .next()
        .filter(|value| !value.is_empty())
        .unwrap_or(body)
        .to_string()
}

pub(crate) fn vue27_template_is_directive_attr(name: &str) -> bool {
    name.starts_with("v-")
        || name.starts_with(':')
        || name.starts_with('@')
        || name.starts_with('#')
}

pub(crate) fn vue27_template_is_builtin_dir(name: &str) -> bool {
    matches!(
        name,
        "text"
            | "html"
            | "show"
            | "if"
            | "else"
            | "else-if"
            | "for"
            | "on"
            | "bind"
            | "model"
            | "slot"
            | "pre"
            | "cloak"
            | "once"
            | "memo"
    )
}

pub(crate) fn vue27_template_is_builtin_tag(name: &str) -> bool {
    matches!(name, "slot" | "component")
}

pub(crate) fn vue27_template_is_reserved_tag(name: &str) -> bool {
    const RESERVED: &str = concat!(
        "html,body,base,head,link,meta,style,title,address,article,aside,footer,header,h1,h2,h3,h4,h5,h6,",
        "nav,section,div,dd,dl,dt,figcaption,figure,picture,hr,img,li,main,ol,p,pre,ul,a,b,abbr,bdi,bdo,",
        "br,cite,code,data,dfn,em,i,kbd,mark,q,rp,rt,ruby,s,samp,small,span,strong,sub,sup,time,u,var,wbr,",
        "area,audio,map,track,video,embed,object,param,source,canvas,script,noscript,del,ins,caption,col,",
        "colgroup,table,thead,tbody,td,th,tr,button,datalist,fieldset,form,input,label,legend,meter,optgroup,",
        "option,output,progress,select,textarea,details,dialog,menu,menuitem,summary,content,element,shadow,",
        "template,blockquote,iframe,tfoot,svg,animate,circle,clippath,cursor,defs,desc,ellipse,filter,font-face,",
        "foreignObject,g,glyph,image,line,marker,mask,missing-glyph,path,pattern,polygon,polyline,rect,switch,",
        "symbol,text,textpath,tspan,use,view"
    );
    RESERVED
        .split(',')
        .any(|tag| tag.eq_ignore_ascii_case(name))
}

pub(crate) fn vue27_process_template_exp(
    exp: &str,
    is_ts: bool,
    directive: Option<&str>,
) -> String {
    if is_ts && vue27_template_exp_has_ts_syntax(exp) {
        if directive == Some("slot") {
            return vue27_extract_js_identifiers(&format!("({exp})=>{{}}"));
        }
        if directive == Some("on") {
            return vue27_extract_js_identifiers(&format!("()=>{{return {exp}}}"));
        }
        if directive == Some("for") {
            if let Some((left, right)) = vue27_split_for_expression(exp) {
                let mut value = vue27_extract_js_identifiers(&format!("({left})=>{{}}"));
                value.push_str(&vue27_extract_js_identifiers(right));
                return value;
            }
        }
        return vue27_extract_js_identifiers(exp);
    }
    let identifiers = vue27_extract_js_identifiers(exp);
    if identifiers.is_empty() {
        vue27_strip_template_expression_strings(exp)
    } else {
        identifiers
    }
}

pub(crate) fn vue27_template_exp_has_ts_syntax(exp: &str) -> bool {
    exp.contains(':') || exp.contains('<') || exp.split_whitespace().any(|part| part == "as")
}

pub(crate) fn vue27_split_for_expression(exp: &str) -> Option<(&str, &str)> {
    for keyword in [" in ", " of "] {
        if let Some(index) = exp.find(keyword) {
            let left = exp[..index].trim();
            let right = exp[index + keyword.len()..].trim();
            if !left.is_empty() && !right.is_empty() {
                return Some((left, right));
            }
        }
    }
    None
}

pub(crate) fn vue27_extract_js_identifiers(exp: &str) -> String {
    let allocator = oxc_allocator::Allocator::default();
    let parse_options = oxc_parser::ParseOptions {
        parse_regular_expression: true,
        ..oxc_parser::ParseOptions::default()
    };
    if let Ok(expression) = oxc_parser::Parser::new(
        &allocator,
        exp,
        oxc_span::SourceType::ts().with_module(false),
    )
    .with_options(parse_options)
    .parse_expression()
    {
        let mut value = String::new();
        collect_vue27_expression_identifier_usage(&expression, &mut value);
        return value;
    }
    let parsed = oxc_parser::Parser::new(
        &allocator,
        exp,
        oxc_span::SourceType::ts().with_module(false),
    )
    .with_options(parse_options)
    .parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return String::new();
    }
    let mut value = String::new();
    for statement in &parsed.program.body {
        collect_vue27_statement_identifier_usage(statement, &mut value);
    }
    value
}
