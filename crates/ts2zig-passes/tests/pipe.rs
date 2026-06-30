use ts2zig_core::{ModuleId, StringTable, SymbolTable, TypeTable};
use ts2zig_ir_hir::{HirDecl, HirEnumVariant, HirExpr, HirFunction, HirProgram, HirStmt};
use ts2zig_ir_mir::{MirDecl, MirExpr, MirGlobalDecl, MirStmt};
use ts2zig_passes::{PassContext, convert_program, lower_enums, lower_result};

fn fixture() -> (StringTable, SymbolTable, TypeTable, PassContext) {
    let strings = StringTable::new();
    let symbols = SymbolTable::new();
    let types = TypeTable::new();
    let ctx = PassContext::default();
    (strings, symbols, types, ctx)
}

fn unit_ty(types: &mut TypeTable) -> ts2zig_core::TypeId {
    types.intern(&ts2zig_core::Type::Void)
}

fn build_enum_decl(
    name: &str,
    variants: Vec<(&str, Option<i64>)>,
    strings: &mut StringTable,
    symbols: &mut SymbolTable,
) -> HirDecl {
    let variants = variants
        .into_iter()
        .map(|(n, v)| HirEnumVariant {
            name: strings.intern(n),
            value: v.map(HirExpr::Int),
        })
        .collect();
    HirDecl::Enum {
        name: symbols.intern(name),
        variants,
    }
}

#[test]
fn convert_program_preserves_global_with_int_init() {
    let (strings, mut symbols, mut types, mut ctx) = fixture();
    let name_sym = symbols.intern("ANSWER");
    let mut hir = HirProgram::new(ModuleId::from_raw(0));
    hir.declarations.push(HirDecl::Global {
        name: name_sym,
        ty: types.intern(&ts2zig_core::Type::I64),
        init: Some(HirExpr::Int(42)),
    });

    let mir = convert_program(&hir, &strings, &mut symbols, &mut ctx);

    assert_eq!(mir.declarations.len(), 1);
    let MirDecl::Global(g) = &mir.declarations[0] else {
        panic!("expected MirDecl::Global");
    };
    assert_eq!(g.name, name_sym);
    let i64_ty = types.intern(&ts2zig_core::Type::I64);
    assert_eq!(g.ty, i64_ty, "global.ty must be the i64 from HIR");
    let Some(init) = &g.init else {
        panic!("init must be preserved through HIR->MIR");
    };
    let MirExpr::Int { value, ty } = init else {
        panic!("expected Int init, got {init:?}");
    };
    assert_eq!(*value, 42);
    assert_eq!(*ty, g.ty, "init.ty must match global.ty, not TypeId(0)");
}

#[test]
fn lower_enums_then_convert_program_emits_globals_with_values() {
    let (mut strings, mut symbols, mut types, mut ctx) = fixture();
    let mut hir = HirProgram::new(ModuleId::from_raw(0));
    hir.declarations.push(build_enum_decl(
        "Color",
        vec![("Red", None), ("Green", Some(10)), ("Blue", None)],
        &mut strings,
        &mut symbols,
    ));

    lower_enums(&mut hir, &strings, &mut symbols, &mut types, &mut ctx);

    let mir = convert_program(&hir, &strings, &mut symbols, &mut ctx);

    let globals: Vec<&MirGlobalDecl> = mir.globals().collect();
    assert_eq!(
        globals.len(),
        3,
        "enum with 3 variants must produce 3 MirDecl::Global"
    );

    let mut by_name: Vec<(String, i128)> = Vec::new();
    for g in globals {
        let raw = symbols.resolve(g.name).unwrap_or("").to_owned();
        let val = match &g.init {
            Some(MirExpr::Int { value, .. }) => *value,
            other => panic!("expected Int init for {raw}, got {other:?}"),
        };
        by_name.push((raw, val));
    }
    by_name.sort_by(|a, b| a.0.cmp(&b.0));

    assert_eq!(by_name[0].0, "Color.Blue");
    assert_eq!(by_name[0].1, 11);
    assert_eq!(by_name[1].0, "Color.Green");
    assert_eq!(by_name[1].1, 10);
    assert_eq!(by_name[2].0, "Color.Red");
    assert_eq!(by_name[2].1, 0);
}

#[test]
fn convert_function_with_throw_sets_throws() {
    let (strings, mut symbols, mut types, mut ctx) = fixture();
    let name = symbols.intern("oops");

    let mut hir = HirProgram::new(ModuleId::from_raw(0));
    hir.declarations.push(HirDecl::Function(HirFunction {
        name,
        params: Vec::new(),
        ret: unit_ty(&mut types),
        throws: None,
        body: vec![HirStmt::Throw {
            expr: HirExpr::Int(7),
        }],
        is_async: false,
        is_generator: false,
        is_exported: false,
        type_params: Vec::new(),
        async_info: None,
    }));

    let mir = convert_program(&hir, &strings, &mut symbols, &mut ctx);
    let fns: Vec<_> = mir.functions().collect();
    assert_eq!(fns.len(), 1);
    let f = fns[0];
    assert!(
        f.throws.is_some(),
        "convert_function must populate throws when body has Throw"
    );
    assert!(f.effects.can_throw);
}

#[test]
fn convert_function_without_throw_leaves_throws_none() {
    let (strings, mut symbols, mut types, mut ctx) = fixture();
    let name = symbols.intern("ok");

    let mut hir = HirProgram::new(ModuleId::from_raw(0));
    hir.declarations.push(HirDecl::Function(HirFunction {
        name,
        params: Vec::new(),
        ret: unit_ty(&mut types),
        throws: None,
        body: vec![HirStmt::Expr {
            expr: HirExpr::Int(1),
        }],
        is_async: false,
        is_generator: false,
        is_exported: false,
        type_params: Vec::new(),
        async_info: None,
    }));

    let mir = convert_program(&hir, &strings, &mut symbols, &mut ctx);
    let f = mir.functions().next().expect("one function");
    assert!(f.throws.is_none());
    assert!(!f.effects.can_throw);
}

#[test]
fn end_to_end_lower_result_rewrites_throw_to_return_result_err() {
    let (strings, mut symbols, mut types, mut ctx) = fixture();
    let name = symbols.intern("oops");

    let mut hir = HirProgram::new(ModuleId::from_raw(0));
    hir.declarations.push(HirDecl::Function(HirFunction {
        name,
        params: Vec::new(),
        ret: unit_ty(&mut types),
        throws: None,
        body: vec![HirStmt::Throw {
            expr: HirExpr::Int(7),
        }],
        is_async: false,
        is_generator: false,
        is_exported: false,
        type_params: Vec::new(),
        async_info: None,
    }));

    let mut mir = convert_program(&hir, &strings, &mut symbols, &mut ctx);
    lower_result(&mut mir);

    let f = mir.functions().next().expect("one function");
    assert!(f.throws.is_some());
    assert_eq!(f.body.block.stmts.len(), 1);
    assert!(
        matches!(f.body.block.stmts[0], MirStmt::ReturnResultErr { .. }),
        "Throw must be rewritten to ReturnResultErr by lower_result, got {:?}",
        f.body.block.stmts[0]
    );
}

#[test]
fn end_to_end_enum_through_hir_to_mir_dump_includes_values() {
    let (mut strings, mut symbols, mut types, mut ctx) = fixture();
    let mut hir = HirProgram::new(ModuleId::from_raw(0));
    hir.declarations.push(build_enum_decl(
        "E",
        vec![("A", None), ("B", None)],
        &mut strings,
        &mut symbols,
    ));

    lower_enums(&mut hir, &strings, &mut symbols, &mut types, &mut ctx);
    let mir = convert_program(&hir, &strings, &mut symbols, &mut ctx);
    let text = mir.dump_text();
    assert!(text.contains("global"), "expected global in dump:\n{text}");

    let globals: Vec<_> = mir.globals().collect();
    assert_eq!(globals.len(), 2);
    let mut by_name: Vec<(String, i128)> = globals
        .into_iter()
        .map(|g| {
            let raw = symbols.resolve(g.name).unwrap_or("").to_owned();
            let val = match &g.init {
                Some(MirExpr::Int { value, .. }) => *value,
                other => panic!("expected Int init for {raw}, got {other:?}"),
            };
            (raw, val)
        })
        .collect();
    by_name.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(by_name[0].0, "E.A");
    assert_eq!(by_name[0].1, 0);
    assert_eq!(by_name[1].0, "E.B");
    assert_eq!(by_name[1].1, 1);
    assert!(
        text.contains("= 0(:0)"),
        "dump must render init=0 explicitly for E.A:\n{text}"
    );
    assert!(
        text.contains("= 1(:0)"),
        "dump must render init=1 explicitly for E.B:\n{text}"
    );
}

fn build_enum_decl_returning_sym(
    name: &str,
    variants: Vec<(&str, Option<i64>)>,
    strings: &mut StringTable,
    symbols: &mut SymbolTable,
) -> (HirDecl, ts2zig_core::SymbolId) {
    let enum_name = symbols.intern(name);
    let variants = variants
        .into_iter()
        .map(|(n, v)| HirEnumVariant {
            name: strings.intern(n),
            value: v.map(HirExpr::Int),
        })
        .collect();
    (
        HirDecl::Enum {
            name: enum_name,
            variants,
        },
        enum_name,
    )
}

#[test]
fn enum_member_use_in_function_body_is_rewritten_to_namespaced_global() {
    let (mut strings, mut symbols, mut types, mut ctx) = fixture();
    let mut hir = HirProgram::new(ModuleId::from_raw(0));

    let (enum_decl, color_sym) = build_enum_decl_returning_sym(
        "Color",
        vec![("Red", None), ("Green", Some(10))],
        &mut strings,
        &mut symbols,
    );
    hir.declarations.push(enum_decl);

    let i64_ty = types.intern(&ts2zig_core::Type::I64);
    let green_name = symbols.intern("Green");
    let fn_name = symbols.intern("pick");

    hir.declarations.push(HirDecl::Function(HirFunction {
        name: fn_name,
        params: Vec::new(),
        ret: i64_ty,
        throws: None,
        body: vec![HirStmt::Return {
            value: Some(HirExpr::Field {
                owner: Box::new(HirExpr::Global {
                    name: color_sym,
                    ty: i64_ty,
                }),
                field: ts2zig_core::FieldId::from_raw(0),
                field_name: green_name,
                ty: i64_ty,
            }),
        }],
        is_async: false,
        is_generator: false,
        is_exported: false,
        type_params: Vec::new(),
        async_info: None,
    }));

    lower_enums(&mut hir, &strings, &mut symbols, &mut types, &mut ctx);
    let mir = convert_program(&hir, &strings, &mut symbols, &mut ctx);

    let fns: Vec<_> = mir.functions().collect();
    assert_eq!(fns.len(), 1);
    let f = fns[0];
    assert_eq!(f.name, fn_name);

    let MirStmt::Return(Some(ret_expr)) = &f.body.block.stmts[0] else {
        panic!(
            "expected Return(Some(expr)), got {:?}",
            f.body.block.stmts[0]
        );
    };
    let MirExpr::Global(resolved) = ret_expr else {
        panic!(
            "Color.Green use must be rewritten to MirExpr::Global, got {:?}",
            ret_expr
        );
    };
    let expected = symbols.intern("Color.Green");
    assert_eq!(
        *resolved, expected,
        "Field(Global(Color), Green) must rewrite to Global(Color.Green)"
    );

    let text = mir.dump_with_symbols(&symbols);
    assert!(
        text.contains("Color.Green"),
        "dump must show the namespaced global:\n{text}"
    );
}
