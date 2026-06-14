//! Vue 2 template compiler implementation.
//!
//! This crate owns the Rust-backed Vue 2 template parser, optimizer, render
//! code generator, public AST projection, SFC asset URL option support, and
//! official-style warning/code-frame result types used by the bridge layers.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

use oxc_span::SourceType;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use vuec_ast::{
    GeneratedReason, Hir, HirBinding, HirConstness, HirDirectiveUse, HirElement, HirEvent, HirExpr,
    HirFor, HirFragment, HirIf, HirIfBranch, HirInterpolation, HirNodeKind, HirObjectBinding,
    HirObjectListeners, HirPropSegment, HirProps, HirRef, HirSlotOutlet, HirStaticAttr, HirTag,
    HtmlNamespace, JsExprId, JsPatternId, JsStmtId, LoweringMap, MirExpr, MissingSpanReason,
    NodeId, NodeSpan, Vue2Ast, Vue2AstKind, Vue2BindWrap, Vue2ComponentModelMir, Vue2CreateElement,
    Vue2DataObject, Vue2DataProp, Vue2DirectiveRuntime, Vue2ForMir, Vue2IfMir, Vue2IfMirBranch,
    Vue2InlineTemplate, Vue2Mir, Vue2MirKind, Vue2NodeKind, Vue2NormalizationType, Vue2Once,
    Vue2RenderStatic, Vue2ScopedSlot, Vue2ScopedSlotBranch, Vue2SlotOutlet, Vue2TextCall,
    Vue2ValidationData,
};
use vuec_diagnostics::{Diagnostic, DiagnosticSink, Severity};
use vuec_html::{decode_html_text_entities, HtmlAttribute, HtmlTokenKind, HtmlTokenizer};
use vuec_js::{parse_vue2_filter_expression, rewrite_vue2_filter_expression, JsAstStore};
use vuec_source::{FileId, Span};

include!("vue2_parts/types.rs");
include!("vue2_parts/compiler_api.rs");
include!("vue2_parts/parser.rs");
include!("vue2_parts/transforms.rs");
include!("vue2_parts/asset_urls.rs");
include!("vue2_parts/conditions_optimizer.rs");
include!("vue2_parts/mir_codegen.rs");
include!("vue2_parts/validation_parse_helpers.rs");
include!("vue2_parts/projection.rs");
include!("vue2_parts/lowering.rs");
include!("vue2_parts/helpers_and_tests.rs");
