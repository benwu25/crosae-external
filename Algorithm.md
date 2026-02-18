The below details the exact algorithm that this repository uses to perform instrumentation.

`TRACKED_PRIMS` $\leftarrow$ $\{$ "i8", "i16", "i32", "i64", "i128", "u8", "u16", "u32", "u64", "u128", "f16", "f32", "f64", "f128" $\}$

On first compilation, after macro expansion and HIR construction:
- `tracked_fn_idents`  $\leftarrow \emptyset$
- `tracked_fn_def_ids` $\leftarrow \emptyset$
- `untracked_fn_calls` $\leftarrow$ $\{$ $\}$  (map from call expr span to return type of call)
- For each `local_did`<sup>1</sup> $\in$ `Crate Body Owners`<sup>2</sup>
    - `body_owner_node` $\leftarrow$ `get_hir_node_by_def_id(local_did)`
    - If `body_owner_node.kind` $=$ `Fn`:
        - `tracked_fn_idents` $\leftarrow$ `tracked_fn_idents` $\cup$ $\{$ `body_owner_node.ident` $\}$
        - `tracked_fn_def_ids` $\leftarrow$ `tracked_fn_def_ids` $\cup$ $\{$ `local_def_id.to_def_id()` $\}$
    
- For each `hir_node` $\in$ `HIR`<sup>3</sup> $|$ `hir_node.kind` $=$ `Expr`:
    - If `hir_node.expr.kind` $=$ `Call`:
        - `called_fn_def_id` $\leftarrow$ `hir_node.expr.hir_id.owner.def_id`
        - `type_result` $\leftarrow$ `type_check(called_fn_def_id)`
        - If `type_result` succesfully resolved `called_fn_def_id`:
            - If `called_fn_def_id` $\in$ `tracked_fn_def_ids`:
                - `untracked_fn_calls` $\leftarrow$ $\{$`hir_node.expr.span` $:$ `type_result.expr_ty(hir_node.expr)` $\}$
- Return (`tracked_fn_idents`, `tracked_fn_def_ids`, `untracked_fn_calls`)

---
On second compilation, during file loading step:
- `file_ast` $\leftarrow$ `parse_into_ast(file)`.
- For each `ast_node` $\in$ `file_ast` $|$ `ast_node.kind` $=$ `Expr`:
    - If `ast_node.expr.kind` $=$ `Lit`:
        - If `ast_node.expr.lit.kind` $=$ `Integer` $\lor$ `ast_node.expr.lit.kind` $=$ `Float`:
            - `ast_node.expr` $\leftarrow$ `parse_expr("ATI.track({ast_node.expr})")`

    - If `ast_node.expr.kind` $=$ `Call`: 
        - If `ast_node.expr.call.span` $\in$ `untracked_fn_calls`:
            - For each `arg_expr` $\in$ `ast_node.expr.call.args`:
                - `arg_expr` $\leftarrow$ `parse_expr("{arg_expr}.0")`
            - `ret_ty` $\leftarrow$ `untracked_fn_calls[ast_node.expr.call.span]`
            - If `ret_ty` $\in$ `TRACKED_PRIMS`:
                - `ast_node.expr` $\leftarrow$ `parse_expr("ATI.track({ast_node.expr})")`

    - If `ast_node.expr.kind` $=$ `Index`:
        - `ast_node.expr` $\leftarrow$ `parse_expr("{ast_node.expr}.0")`

- `fn_sigs` $\leftarrow$ $\{$ $\}$ (map from fn name to fn signature information (params names and types, and return type))
- For each `ast_node` $\in$ `file_ast` $|$ `ast_node.kind` $=$ `Item`:
    - If `ast_node.expr.kind` $=$ `Fn` $\land$ `ast_node.expr.fn.ident` $\in$ `tracked_fn_idents`:
        - For each `param` $\in$ `ast_node.expr.fn.sig.decl.inputs`:
            - `param` $\leftarrow$ `recursively_tuple_type(param.type)`
        - If `ast_node.expr.fn.sig.decl.output` exists:
            `ast_node.expr.fn.sig.decl.output` $\leftarrow$ `recursively_tuple_type(ast_node.expr.fn.sig.decl.output)`
        - `fn_sigs` $\leftarrow$ `fn_sigs` $\cup$ $\{$ `ast_node.expr.fn.ident`: `(params, returns)` $\}$
        - `ast_node.expr.fn.ident` $\leftarrow$  `"{ast_node.expr.fn.ident}_unstubbed"`
    
    - If `ast_node.expr.kind` $=$ `Struct`:
        - For `field` $\in$ `ast_node.expr.struct.fields`: <sup>6</sup>
            - `field` $\leftarrow$ `recusively_tuple_type(field.ty)`

- For each `fn_sig`, create stub and insert into crate
- `unparse(file_ast)`, and execute the rest of the compiler

---
`recursively_tuple_type(ast_expr_type)`:
- `peeled_type` $\leftarrow$ `peel_refs(ast_expr_type)` (removes all outer &/&mut)
- If `peeled_type` $\in$ `TRACKED_PRIMS`:
    - `peeled_type` $\leftarrow$ `parse_type("TaggedValue<{peeled_type}>")`
- Else:
    - Depending on kind of `peeled_type`, recursively tuple relevant inner types:
        - `[T]` $\rightarrow$ `[recursively_tuple_type(T)]`
        - `[T; c]` $\rightarrow$ `[recursively_tuple_type(T); c]`
        - `*(const|mut) T` $\rightarrow$ `*(const|mut) recursively_tuple_type(T)`
        - `(&|&mut) T` $\rightarrow$ `(&|&mut) recursively_tuple_type(T)`
        - `fn(A, B, ...) -> C` $\rightarrow$ `fn(recursively_tuple_type(A), recursively_tuple_type(B), ...) -> recursively_tuple_type(C)` <sup>4</sup>
        - `(A, B, ...)` $\rightarrow$ `(recursively_tuple_type(A), recursively_tuple_type(A), ... )`
        - `Path::With::Generics<A, B, ...>` $\rightarrow$ `Path::With::Generics<recursively_tuple_type(A), recursively_tuple_type(B), ...>` <sup>5</sup>
        - TODO:
            - PinnedRef
            - Pat
            - Infer
            - TraitObject
            - Paren
            - UnsafeBinder
            - Never
            - ImplTrait
            - ImplicitSelf
            - MacCall
            - CVarArgs
            - Dummy
            - Err