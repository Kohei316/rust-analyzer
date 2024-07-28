//! keys to be used with `DynMap`

use std::hash::Hash;
use std::marker::PhantomData;

use hir_expand::{attrs::AttrId, MacroCallId};
use rustc_hash::FxHashMap;
use syntax::{
    ast::{self, Variant},
    AstNode, AstPtr,
};

use crate::{
    dyn_map::{DynMap, Policy},
    BlockId, ConstId, EnumId, EnumVariantId, ExternBlockId, ExternCrateId, FieldId, FunctionId,
    ImplId, LifetimeParamId, Macro2Id, MacroRulesId, ProcMacroId, StaticId, StructId, TraitAliasId,
    TraitId, TypeAliasId, TypeOrConstParamId, UnionId, UseId,
};

pub type Key<K, V> = crate::dyn_map::Key<K, AstPtr<V>, DefIdPolicy<K, V>>;

pub const BLOCK: Key<BlockId, ast::BlockExpr> = Key::new();
pub const FUNCTION: Key<FunctionId, ast::Fn> = Key::new();
pub const Variant: Key<Variant, ast::Variant> = Key::new();
pub const CONST: Key<ConstId, ast::Const> = Key::new();
pub const STATIC: Key<StaticId, ast::Static> = Key::new();
pub const TYPE_ALIAS: Key<TypeAliasId, ast::TypeAlias> = Key::new();
pub const IMPL: Key<ImplId, ast::Impl> = Key::new();
pub const TRAIT: Key<TraitId, ast::Trait> = Key::new();
pub const TRAIT_ALIAS: Key<TraitAliasId, ast::TraitAlias> = Key::new();
pub const STRUCT: Key<StructId, ast::Struct> = Key::new();
pub const UNION: Key<UnionId, ast::Union> = Key::new();
pub const ENUM: Key<EnumId, ast::Enum> = Key::new();
pub const EXTERN_CRATE: Key<ExternCrateId, ast::ExternCrate> = Key::new();
pub const EXTERN_BLOCK: Key<ExternBlockId, ast::ExternBlock> = Key::new();
pub const USE: Key<UseId, ast::Use> = Key::new();

pub const ENUM_VARIANT: Key<EnumVariantId, ast::Variant> = Key::new();
pub const TUPLE_FIELD: Key<FieldId, ast::TupleField> = Key::new();
pub const RECORD_FIELD: Key<FieldId, ast::RecordField> = Key::new();
pub const TYPE_PARAM: Key<TypeOrConstParamId, ast::TypeParam> = Key::new();
pub const CONST_PARAM: Key<TypeOrConstParamId, ast::ConstParam> = Key::new();
pub const LIFETIME_PARAM: Key<LifetimeParamId, ast::LifetimeParam> = Key::new();

pub const MACRO_RULES: Key<MacroRulesId, ast::MacroRules> = Key::new();
pub const MACRO2: Key<Macro2Id, ast::MacroDef> = Key::new();
pub const PROC_MACRO: Key<ProcMacroId, ast::Fn> = Key::new();
pub const MACRO_CALL: Key<MacroCallId, ast::MacroCall> = Key::new();
pub const ATTR_MACRO_CALL: Key<MacroCallId, ast::Item> = Key::new();
pub const DERIVE_MACRO_CALL: Key<(AttrId, MacroCallId, Box<[Option<MacroCallId>]>), ast::Attr> =
    Key::new();

/// XXX: AST Nodes and SyntaxNodes have identity equality semantics: nodes are
/// equal if they point to exactly the same object.
///
/// In general, we do not guarantee that we have exactly one instance of a
/// syntax tree for each file. We probably should add such guarantee, but, for
/// the time being, we will use identity-less AstPtr comparison.
pub struct DefIdPolicy<ID, AST> {
    _phantom: PhantomData<(ID, AST)>,
}

impl<AST: AstNode + 'static, ID: Eq + Hash + 'static> Policy for DefIdPolicy<ID, AST> {
    type K = ID;
    type V = AstPtr<AST>;
    fn insert(map: &mut DynMap, key: ID, value: AstPtr<AST>) {
        map.map
            .entry::<FxHashMap<ID, AstPtr<AST>>>()
            .or_insert_with(Default::default)
            .insert(key, value);
    }
    fn get<'a>(map: &'a DynMap, key: &ID) -> Option<&'a AstPtr<AST>> {
        map.map.get::<FxHashMap<ID, AstPtr<AST>>>()?.get(key)
    }
    fn is_empty(map: &DynMap) -> bool {
        map.map.get::<FxHashMap<AstPtr<AST>, ID>>().map_or(true, |it| it.is_empty())
    }
}
