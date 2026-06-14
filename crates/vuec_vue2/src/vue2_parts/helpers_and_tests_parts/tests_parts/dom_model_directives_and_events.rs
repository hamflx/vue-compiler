    #[test]
    fn generates_ref_data_and_single_dom_model_directive() {
        let plain_ref = compile(r#"<p ref="component1"></p>"#, options());
        assert_eq!(
            plain_ref.render,
            r#"with(this){return _c('p',{ref:"component1"})}"#
        );

        let for_ref = compile(
            r#"<ul><li v-for="item in items" ref="component1"></li></ul>"#,
            options(),
        );
        assert_eq!(
            for_ref.render,
            r#"with(this){return _c('ul',_l((items),function(item){return _c('li',{ref:"component1",refInFor:true})}),0)}"#
        );

        let nested_static_ref = compile(
            r#"<ul><li v-for="item in items"><span ref="component1"></span></li></ul>"#,
            options(),
        );
        assert_eq!(
            nested_static_ref.render,
            r#"with(this){return _c('ul',_l((items),function(item){return _c('li',[_c('span',{ref:"component1",refInFor:true})])}),0)}"#
        );

        let nested_dynamic_ref = compile(
            r#"<ul><li v-for="item in items"><CnCell :ref="'cell-' + item.id"/></li></ul>"#,
            options(),
        );
        assert_eq!(
            nested_dynamic_ref.render,
            r#"with(this){return _c('ul',_l((items),function(item){return _c('li',[_c('CnCell',{ref:'cell-' + item.id,refInFor:true})],1)}),0)}"#
        );

        let model = compile(r#"<input v-model="test">"#, options());
        assert_eq!(model.render.matches(r#"name:"model""#).count(), 1);
        assert!(model
            .render
            .contains(r#"domProps:{"value":(test)},on:{"input":function($event){if($event.target.composing)return;test=$event.target.value}}"#));

        let multiline_model = compile("<input v-model=\"\n test \n\">", options());
        let expected_value_prop = concat!("domProps:{\"value\":(\n", " test \n", ")}");
        assert!(multiline_model.render.contains(expected_value_prop));
        assert!(multiline_model
            .render
            .contains("if($event.target.composing)return;\n test \n=$event.target.value"));

        let component_model = compile("<my-component v-model=\"\n test \n\" />", options());
        assert!(component_model
            .render
            .contains("callback:function ($$v) {\n test \n=$$v}"));

        let model_with_input = compile(
            r#"<input @input="updateValue($event.target.value)" @change="emitChange" v-model="val" ref="input">"#,
            options(),
        );
        assert_eq!(
            model_with_input.render,
            r#"with(this){return _c('input',{directives:[{name:"model",rawName:"v-model",value:(val),expression:"val"}],ref:"input",domProps:{"value":(val)},on:{"input":[function($event){if($event.target.composing)return;val=$event.target.value},function($event){return updateValue($event.target.value)}],"change":emitChange}})}"#
        );

        let model_with_later_keyup =
            compile(r#"<input v-model="val" @keyup.enter="submit">"#, options());
        assert_eq!(
            model_with_later_keyup.render,
            r#"with(this){return _c('input',{directives:[{name:"model",rawName:"v-model",value:(val),expression:"val"}],domProps:{"value":(val)},on:{"keyup":function($event){if(!$event.type.indexOf('key')&&_k($event.keyCode,"enter",13,$event.key,"Enter"))return null;return submit.apply(null, arguments)},"input":function($event){if($event.target.composing)return;val=$event.target.value}}})}"#
        );

        let lazy_model_with_change = compile(
            r#"<input @change="emitChange" v-model.lazy="val">"#,
            options(),
        );
        assert_eq!(
            lazy_model_with_change.render,
            r#"with(this){return _c('input',{directives:[{name:"model",rawName:"v-model.lazy",value:(val),expression:"val",modifiers:{"lazy":true}}],domProps:{"value":(val)},on:{"change":[function($event){val=$event.target.value},emitChange]}})}"#
        );
    }

    #[test]
    fn generates_vue2_dom_model_platform_branches_like_official_codegen() {
        let checkbox = compile(r#"<input type="checkbox" v-model="checked">"#, options());
        assert_eq!(
            checkbox.render,
            r#"with(this){return _c('input',{directives:[{name:"model",rawName:"v-model",value:(checked),expression:"checked"}],attrs:{"type":"checkbox"},domProps:{"checked":Array.isArray(checked)?_i(checked,null)>-1:(checked)},on:{"change":function($event){var $$a=checked,$$el=$event.target,$$c=$$el.checked?(true):(false);if(Array.isArray($$a)){var $$v=null,$$i=_i($$a,$$v);if($$el.checked){$$i<0&&(checked=$$a.concat([$$v]))}else{$$i>-1&&(checked=$$a.slice(0,$$i).concat($$a.slice($$i+1)))}}else{checked=$$c}}}})}"#
        );

        let checkbox_value = compile(
            r#"<input type="checkbox" :value="item.value" v-model="checked">"#,
            options(),
        );
        assert_eq!(
            checkbox_value.render,
            r#"with(this){return _c('input',{directives:[{name:"model",rawName:"v-model",value:(checked),expression:"checked"}],attrs:{"type":"checkbox"},domProps:{"value":item.value,"checked":Array.isArray(checked)?_i(checked,item.value)>-1:(checked)},on:{"change":function($event){var $$a=checked,$$el=$event.target,$$c=$$el.checked?(true):(false);if(Array.isArray($$a)){var $$v=item.value,$$i=_i($$a,$$v);if($$el.checked){$$i<0&&(checked=$$a.concat([$$v]))}else{$$i>-1&&(checked=$$a.slice(0,$$i).concat($$a.slice($$i+1)))}}else{checked=$$c}}}})}"#
        );

        let checkbox_custom_values = compile(
            r#"<input type="checkbox" :value="item.value" :true-value="yes" :false-value="no" v-model.number="checked">"#,
            options(),
        );
        assert_eq!(
            checkbox_custom_values.render,
            r#"with(this){return _c('input',{directives:[{name:"model",rawName:"v-model.number",value:(checked),expression:"checked",modifiers:{"number":true}}],attrs:{"type":"checkbox","true-value":yes,"false-value":no},domProps:{"value":item.value,"checked":Array.isArray(checked)?_i(checked,item.value)>-1:_q(checked,yes)},on:{"change":function($event){var $$a=checked,$$el=$event.target,$$c=$$el.checked?(yes):(no);if(Array.isArray($$a)){var $$v=_n(item.value),$$i=_i($$a,$$v);if($$el.checked){$$i<0&&(checked=$$a.concat([$$v]))}else{$$i>-1&&(checked=$$a.slice(0,$$i).concat($$a.slice($$i+1)))}}else{checked=$$c}}}})}"#
        );

        let radio = compile(
            r#"<input type="radio" :value="nativeValue" v-model="picked">"#,
            options(),
        );
        assert_eq!(
            radio.render,
            r#"with(this){return _c('input',{directives:[{name:"model",rawName:"v-model",value:(picked),expression:"picked"}],attrs:{"type":"radio"},domProps:{"value":nativeValue,"checked":_q(picked,nativeValue)},on:{"change":function($event){picked=nativeValue}}})}"#
        );

        let select = compile(
            r#"<select v-model="selected"><option :value="item"></option></select>"#,
            options(),
        );
        assert_eq!(
            select.render,
            r#"with(this){return _c('select',{directives:[{name:"model",rawName:"v-model",value:(selected),expression:"selected"}],on:{"change":function($event){var $$selectedVal = Array.prototype.filter.call($event.target.options,function(o){return o.selected}).map(function(o){var val = "_value" in o ? o._value : o.value;return val}); selected=$event.target.multiple ? $$selectedVal : $$selectedVal[0]}}},[_c('option',{domProps:{"value":item}})])}"#
        );

        let range = compile(r#"<input type="range" v-model="val">"#, options());
        assert_eq!(
            range.render,
            r#"with(this){return _c('input',{directives:[{name:"model",rawName:"v-model",value:(val),expression:"val"}],attrs:{"type":"range"},domProps:{"value":(val)},on:{"__r":function($event){val=$event.target.value}}})}"#
        );

        let number = compile(r#"<input v-model.number="val">"#, options());
        assert_eq!(
            number.render,
            r#"with(this){return _c('input',{directives:[{name:"model",rawName:"v-model.number",value:(val),expression:"val",modifiers:{"number":true}}],domProps:{"value":(val)},on:{"input":function($event){if($event.target.composing)return;val=_n($event.target.value)},"blur":function($event){return $forceUpdate()}}})}"#
        );

        let dynamic_type = compile(
            r#"<input v-model="computedValue" :type="option.as" :value="option.nativeValue">"#,
            options(),
        );
        assert_eq!(
            dynamic_type.render,
            r#"with(this){return ((option.as)==='checkbox')?_c('input',{directives:[{name:"model",rawName:"v-model",value:(computedValue),expression:"computedValue"}],attrs:{"type":"checkbox"},domProps:{"value":option.nativeValue,"checked":Array.isArray(computedValue)?_i(computedValue,option.nativeValue)>-1:(computedValue)},on:{"change":function($event){var $$a=computedValue,$$el=$event.target,$$c=$$el.checked?(true):(false);if(Array.isArray($$a)){var $$v=option.nativeValue,$$i=_i($$a,$$v);if($$el.checked){$$i<0&&(computedValue=$$a.concat([$$v]))}else{$$i>-1&&(computedValue=$$a.slice(0,$$i).concat($$a.slice($$i+1)))}}else{computedValue=$$c}}}}):((option.as)==='radio')?_c('input',{directives:[{name:"model",rawName:"v-model",value:(computedValue),expression:"computedValue"}],attrs:{"type":"radio"},domProps:{"value":option.nativeValue,"checked":_q(computedValue,option.nativeValue)},on:{"change":function($event){computedValue=option.nativeValue}}}):_c('input',{directives:[{name:"model",rawName:"v-model",value:(computedValue),expression:"computedValue"}],attrs:{"type":option.as},domProps:{"value":option.nativeValue,"value":(computedValue)},on:{"input":function($event){if($event.target.composing)return;computedValue=$event.target.value}}})}"#
        );
    }

    #[test]
    fn generates_vue2_platform_must_use_props_like_official_codegen() {
        let option = compile(
            r#"<option :value="item.value" :selected="item.isRefined"></option>"#,
            options(),
        );
        assert_eq!(
            option.render,
            r#"with(this){return _c('option',{domProps:{"value":item.value,"selected":item.isRefined}})}"#
        );

        let checkbox = compile(
            r#"<input type="checkbox" :value="item.value" :checked="item.isRefined">"#,
            options(),
        );
        assert_eq!(
            checkbox.render,
            r#"with(this){return _c('input',{attrs:{"type":"checkbox"},domProps:{"value":item.value,"checked":item.isRefined}})}"#
        );

        let select = compile(r#"<select :value="selected"></select>"#, options());
        assert_eq!(
            select.render,
            r#"with(this){return _c('select',{domProps:{"value":selected}})}"#
        );

        let progress = compile(r#"<progress :value="n"></progress>"#, options());
        assert_eq!(
            progress.render,
            r#"with(this){return _c('progress',{domProps:{"value":n}})}"#
        );

        let button_value = compile(r#"<input type="button" :value="label">"#, options());
        assert_eq!(
            button_value.render,
            r#"with(this){return _c('input',{attrs:{"type":"button","value":label}})}"#
        );

        let dynamic_component = compile(
            r#"<component :is="tag" :value="label"></component>"#,
            options(),
        );
        assert_eq!(
            dynamic_component.render,
            r#"with(this){return _c(tag,{tag:"component",attrs:{"value":label}})}"#
        );
    }

    #[test]
    fn preserves_vue2_standard_validation_attrs_like_official_codegen() {
        let component_attr = compile(r#"<el-input maxlength="50"></el-input>"#, options());
        assert_eq!(
            component_attr.render,
            r#"with(this){return _c('el-input',{attrs:{"maxlength":"50"}})}"#
        );

        let input_attrs = compile(r#"<input maxlength="50" required>"#, options());
        assert_eq!(
            input_attrs.render,
            r#"with(this){return _c('input',{attrs:{"maxlength":"50","required":""}})}"#
        );

        let custom_validate = compile(
            r#"<input v-validate:field.required maxlength="50">"#,
            options(),
        );
        assert_eq!(
            custom_validate.render,
            r#"with(this){return _c('input',{directives:[{name:"validate",rawName:"v-validate:field.required",arg:"field",modifiers:{"required":true}}],attrs:{"maxlength":"50"}})}"#
        );
        assert!(!custom_validate.render.contains("_c('validate'"));

        let ordered_directive_modifiers = compile(
            r#"<button v-b-popover.hover.bottom="'I am Bottom'"></button>"#,
            options(),
        );
        assert_eq!(
            ordered_directive_modifiers.render,
            r#"with(this){return _c('button',{directives:[{name:"b-popover",rawName:"v-b-popover.hover.bottom",value:('I am Bottom'),expression:"'I am Bottom'",modifiers:{"hover":true,"bottom":true}}]})}"#
        );

        let numeric_directive_modifier = compile(
            r#"<div v-b-visible.once.1000="visibleHandler"></div>"#,
            options(),
        );
        assert_eq!(
            numeric_directive_modifier.render,
            r#"with(this){return _c('div',{directives:[{name:"b-visible",rawName:"v-b-visible.once.1000",value:(visibleHandler),expression:"visibleHandler",modifiers:{"1000":true,"once":true}}]})}"#
        );
    }

    #[test]
    fn generates_vue2_html_and_text_directives_like_official_codegen() {
        let compiled = compile(
            r#"<div><p v-html="content"></p><span v-text="label"></span></div>"#,
            options(),
        );

        assert_eq!(
            compiled.render,
            r#"with(this){return _c('div',[_c('p',{domProps:{"innerHTML":_s(content)}}),_c('span',{domProps:{"textContent":_s(label)}})])}"#
        );
        assert!(!compiled.render.contains("directives:"));
    }

    #[test]
    fn generates_vue2_event_handlers_like_official_codegen() {
        let method_call = compile(r#"<input @input="functionName()">"#, options());
        assert_eq!(
            method_call.render,
            r#"with(this){return _c('input',{on:{"input":function($event){return functionName()}}})}"#
        );

        let computed_ref_call = compile(
            r#"<button @click="$refs[scope.row.id].doClose()">x</button>"#,
            options(),
        );
        assert_eq!(
            computed_ref_call.render,
            r#"with(this){return _c('button',{on:{"click":function($event){$refs[scope.row.id].doClose()}}},[_v("x")])}"#
        );

        let tricky_call = compile(r#"<input @input="onInput(');[\'());');">"#, options());
        assert_eq!(
            tricky_call.render,
            r#"with(this){return _c('input',{on:{"input":function($event){onInput(');[\'());');}}})}"#
        );

        let multiple_statements = compile(r#"<input @input="onInput1();onInput2()">"#, options());
        assert_eq!(
            multiple_statements.render,
            r#"with(this){return _c('input',{on:{"input":function($event){onInput1();onInput2()}}})}"#
        );

        let ordered_keys = compile(r#"<input @keydown.enter.delete="onInput">"#, options());
        assert_eq!(
            ordered_keys.render,
            r#"with(this){return _c('input',{on:{"keydown":function($event){if(!$event.type.indexOf('key')&&_k($event.keyCode,"enter",13,$event.key,"Enter")&&_k($event.keyCode,"delete",[8,46],$event.key,["Backspace","Delete","Del"]))return null;return onInput.apply(null, arguments)}}})}"#
        );

        let ordered_modifiers = compile(r#"<input @input.stop.prevent.self="onInput">"#, options());
        assert_eq!(
            ordered_modifiers.render,
            r#"with(this){return _c('input',{on:{"input":function($event){$event.stopPropagation();$event.preventDefault();if($event.target !== $event.currentTarget)return null;return onInput.apply(null, arguments)}}})}"#
        );

        let left_exact = compile(
            r#"<button @click.exact.left.prevent="go"></button>"#,
            options(),
        );
        assert_eq!(
            left_exact.render,
            r#"with(this){return _c('button',{on:{"click":function($event){if(!$event.type.indexOf('key')&&_k($event.keyCode,"left",37,$event.key,["Left","ArrowLeft"]))return null;if($event.ctrlKey||$event.shiftKey||$event.altKey||$event.metaKey)return null;if('button' in $event && $event.button !== 0)return null;$event.preventDefault();return go.apply(null, arguments)}}})}"#
        );

        let capture_once = compile(r#"<input @input.capture.once="onInput">"#, options());
        assert_eq!(
            capture_once.render,
            r#"with(this){return _c('input',{on:{"~!input":function($event){return onInput.apply(null, arguments)}}})}"#
        );
    }

    #[test]
    fn generates_vue2_dynamic_event_handlers_like_official_codegen() {
        let dynamic = compile(r#"<Comp @[event]="change"/>"#, options());
        assert_eq!(
            dynamic.render,
            r#"with(this){return _c('Comp',{on:_d({},[event,change])})}"#
        );

        let mixed = compile(r#"<Comp @click="click" @[event]="change"/>"#, options());
        assert_eq!(
            mixed.render,
            r#"with(this){return _c('Comp',{on:_d({"click":click},[event,change])})}"#
        );

        let native_prevent = compile(
            r#"<Comp @[event].native.prevent="change(item, checked)"/>"#,
            options(),
        );
        assert_eq!(
            native_prevent.render,
            r#"with(this){return _c('Comp',{nativeOn:_d({},[event,function($event){$event.preventDefault();return change(item, checked)}])})}"#
        );

        let prefixed = compile(
            r#"<Comp @[event].capture.once.passive="change"/>"#,
            options(),
        );
        assert_eq!(
            prefixed.render,
            r#"with(this){return _c('Comp',{on:_d({},[_p(_p(_p(event,"!"),"~"),"&"),function($event){return change.apply(null, arguments)}])})}"#
        );

        let right_click = compile(r#"<Comp @[event].right="change"/>"#, options());
        assert_eq!(
            right_click.render,
            r#"with(this){return _c('Comp',{on:_d({},[(event)==='click'?'contextmenu':(event),function($event){if(!$event.type.indexOf('key')&&_k($event.keyCode,"right",39,$event.key,["Right","ArrowRight"]))return null;if('button' in $event && $event.button !== 2)return null;return change.apply(null, arguments)}])})}"#
        );

        let merged_same_name = compile(r#"<Comp @event="foo" @[event]="bar"/>"#, options());
        assert_eq!(
            merged_same_name.render,
            r#"with(this){return _c('Comp',{on:{"event":[foo,bar]}})}"#
        );
    }

    #[test]
    fn generates_vue2_native_event_handlers_like_official_codegen() {
        let native_prevent = compile(
            r#"<my-button @click.native.prevent="submit"></my-button>"#,
            options(),
        );
        assert_eq!(
            native_prevent.render,
            r#"with(this){return _c('my-button',{nativeOn:{"click":function($event){$event.preventDefault();return submit.apply(null, arguments)}}})}"#
        );
        assert!(!native_prevent.render.contains(r#""native""#));

        let native_path = compile(
            r#"<my-button @click.native="submit"></my-button>"#,
            options(),
        );
        assert_eq!(
            native_path.render,
            r#"with(this){return _c('my-button',{nativeOn:{"click":function($event){return submit.apply(null, arguments)}}})}"#
        );

        let native_key = compile(
            r#"<my-button @keyup.native.enter="submit"></my-button>"#,
            options(),
        );
        assert_eq!(
            native_key.render,
            r#"with(this){return _c('my-button',{nativeOn:{"keyup":function($event){if(!$event.type.indexOf('key')&&_k($event.keyCode,"enter",13,$event.key,"Enter"))return null;return submit.apply(null, arguments)}}})}"#
        );

        let empty_native_prevent =
            compile(r#"<el-form @submit.native.prevent></el-form>"#, options());
        assert_eq!(
            empty_native_prevent.render,
            r#"with(this){return _c('el-form',{nativeOn:{"submit":function($event){$event.preventDefault();}}})}"#
        );
    }

    #[test]
    fn generates_vue2_static_style_sync_and_event_order_like_official_codegen() {
        let no_optimize_options = Vue2CompileOptions {
            optimize: false,
            ..options()
        };
        let class_and_empty_style = compile(
            r#"<section><div class="el-button el-button--primary " style=""></div><span class=" "></span><i class=""></i></section>"#,
            no_optimize_options.clone(),
        );
        assert_eq!(
            class_and_empty_style.render,
            r#"with(this){return _c('section',[_c('div',{staticClass:"el-button el-button--primary"}),_c('span',{staticClass:""}),_c('i',{})])}"#
        );

        let empty_class_and_style_data = compile(
            r#"<section><div class=""><span>{{ message }}</span></div><div style=""><span>{{ message }}</span></div></section>"#,
            no_optimize_options,
        );
        assert_eq!(
            empty_class_and_style_data.render,
            r#"with(this){return _c('section',[_c('div',{},[_c('span',[_v(_s(message))])]),_c('div',{},[_c('span',[_v(_s(message))])])])}"#
        );

        let pagination = compile(
            r#"<el-pagination
  :page-size.sync="page.size"
  :total="page.total"
  :current-page.sync="page.page"
  style="margin-top: 8px;"
  layout="total, prev, pager, next, sizes"
  @size-change="crud.sizeChangeHandler($event)"
  @current-change="crud.pageChangeHandler"
/>"#,
            options(),
        );
        assert_eq!(
            pagination.render,
            r#"with(this){return _c('el-pagination',{staticStyle:{"margin-top":"8px"},attrs:{"page-size":page.size,"total":page.total,"current-page":page.page,"layout":"total, prev, pager, next, sizes"},on:{"update:pageSize":function($event){return $set(page, "size", $event)},"update:page-size":function($event){return $set(page, "size", $event)},"update:currentPage":function($event){return $set(page, "page", $event)},"update:current-page":function($event){return $set(page, "page", $event)},"size-change":function($event){return crud.sizeChangeHandler($event)},"current-change":crud.pageChangeHandler}})}"#
        );

        let popover = compile(
            r#"<div style="text-align: right; margin: 0" @show="onPopoverShow" @hide="onPopoverHide"></div>"#,
            options(),
        );
        assert_eq!(
            popover.render,
            r#"with(this){return _c('div',{staticStyle:{"text-align":"right","margin":"0"},on:{"show":onPopoverShow,"hide":onPopoverHide}})}"#
        );
    }

    #[test]
    fn generates_vue2_empty_event_handler_like_official_codegen() {
        let parsed = compile(r#"<input @input="current++">"#, options());
        let mut element = parsed.element_ast.unwrap();
        element.events.insert("input".into(), Vec::new());
        let generated = generate(Some(&element), &options());
        assert_eq!(
            generated.render,
            r#"with(this){return _c('input',{on:{"input":function(){}}})}"#
        );
        assert!(generated.static_render_fns.is_empty());
    }
