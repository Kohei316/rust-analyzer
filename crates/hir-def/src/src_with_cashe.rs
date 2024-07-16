//! Utilities for mapping between hir IDs and the surface syntax.

use hir_expand::{InFile, Lookup};
use syntax::{ast, AstNode, AstPtr};

use crate::{
    db::DefDatabase, dyn_map::def_to_src, item_tree::ItemTreeNode,
    src_def_cashe::SrcDefCacheContext, ConstId, EnumId, EnumVariantId, FunctionId, ImplId,
    ItemTreeLoc, Macro2Id, MacroRulesId, ProcMacroId, StaticId, StructId, TraitAliasId, TraitId,
    TypeAliasId, UnionId,
};

pub trait HasSource {
    type Value: AstNode;

    fn source<CTX: SrcDefCacheContext>(
        self,
        db: &dyn DefDatabase,
        ctx: &CTX,
    ) -> InFile<Self::Value>;
}

impl HasSource for StructId {
    type Value = ast::Struct;
    fn source<CTX: SrcDefCacheContext>(
        self,
        db: &dyn DefDatabase,
        ctx: &CTX,
    ) -> InFile<Self::Value> {
        let loc = self.lookup(db);
        let file_id = loc.item_tree_id().file_id();
        let ast_ptr = ctx.get_or_inset_with(def_to_src::STRUCT, self, || ast_ptr(loc, db));
        InFile::new(file_id, ast_ptr.to_node(&ctx.parse_or_expand(file_id)))
    }
}

impl HasSource for UnionId {
    type Value = ast::Union;

    fn source<CTX: SrcDefCacheContext>(
        self,
        db: &dyn DefDatabase,
        ctx: &CTX,
    ) -> InFile<Self::Value> {
        let loc = self.lookup(db);
        let file_id = loc.item_tree_id().file_id();
        let ast_ptr = ctx.get_or_inset_with(def_to_src::UNION, self, || ast_ptr(loc, db));
        InFile::new(file_id, ast_ptr.to_node(&ctx.parse_or_expand(file_id)))
    }
}

impl HasSource for EnumId {
    type Value = ast::Enum;

    fn source<CTX: SrcDefCacheContext>(
        self,
        db: &dyn DefDatabase,
        ctx: &CTX,
    ) -> InFile<Self::Value> {
        let loc = self.lookup(db);
        let file_id = loc.item_tree_id().file_id();
        let ast_ptr = ctx.get_or_inset_with(def_to_src::ENUM, self, || ast_ptr(loc, db));
        InFile::new(file_id, ast_ptr.to_node(&ctx.parse_or_expand(file_id)))
    }
}

impl HasSource for EnumVariantId {
    type Value = ast::Variant;

    fn source<CTX: SrcDefCacheContext>(
        self,
        db: &dyn DefDatabase,
        ctx: &CTX,
    ) -> InFile<Self::Value> {
        let loc = self.lookup(db);
        let file_id = loc.item_tree_id().file_id();
        let ast_ptr = ctx.get_or_inset_with(def_to_src::ENUM_VARIANT, self, || ast_ptr(loc, db));
        InFile::new(file_id, ast_ptr.to_node(&ctx.parse_or_expand(file_id)))
    }
}

impl HasSource for FunctionId {
    type Value = ast::Fn;

    fn source<CTX: SrcDefCacheContext>(
        self,
        db: &dyn DefDatabase,
        ctx: &CTX,
    ) -> InFile<Self::Value> {
        let loc = self.lookup(db);
        let file_id = loc.item_tree_id().file_id();
        let ast_ptr = ctx.get_or_inset_with(def_to_src::FUNCTION, self, || ast_ptr(loc, db));
        InFile::new(file_id, ast_ptr.to_node(&ctx.parse_or_expand(file_id)))
    }
}

impl HasSource for ConstId {
    type Value = ast::Const;

    fn source<CTX: SrcDefCacheContext>(
        self,
        db: &dyn DefDatabase,
        ctx: &CTX,
    ) -> InFile<Self::Value> {
        let loc = self.lookup(db);
        let file_id = loc.item_tree_id().file_id();
        let ast_ptr = ctx.get_or_inset_with(def_to_src::CONST, self, || ast_ptr(loc, db));
        InFile::new(file_id, ast_ptr.to_node(&ctx.parse_or_expand(file_id)))
    }
}

impl HasSource for StaticId {
    type Value = ast::Static;

    fn source<CTX: SrcDefCacheContext>(
        self,
        db: &dyn DefDatabase,
        ctx: &CTX,
    ) -> InFile<Self::Value> {
        let loc = self.lookup(db);
        let file_id = loc.item_tree_id().file_id();
        let ast_ptr = ctx.get_or_inset_with(def_to_src::STATIC, self, || ast_ptr(loc, db));
        InFile::new(file_id, ast_ptr.to_node(&ctx.parse_or_expand(file_id)))
    }
}

impl HasSource for TraitId {
    type Value = ast::Trait;

    fn source<CTX: SrcDefCacheContext>(
        self,
        db: &dyn DefDatabase,
        ctx: &CTX,
    ) -> InFile<Self::Value> {
        let loc = self.lookup(db);
        let file_id = loc.item_tree_id().file_id();
        let ast_ptr = ctx.get_or_inset_with(def_to_src::TRAIT, self, || ast_ptr(loc, db));
        InFile::new(file_id, ast_ptr.to_node(&ctx.parse_or_expand(file_id)))
    }
}

impl HasSource for TraitAliasId {
    type Value = ast::TraitAlias;

    fn source<CTX: SrcDefCacheContext>(
        self,
        db: &dyn DefDatabase,
        ctx: &CTX,
    ) -> InFile<Self::Value> {
        let loc = self.lookup(db);
        let file_id = loc.item_tree_id().file_id();
        let ast_ptr = ctx.get_or_inset_with(def_to_src::TRAIT_ALIAS, self, || ast_ptr(loc, db));
        InFile::new(file_id, ast_ptr.to_node(&ctx.parse_or_expand(file_id)))
    }
}

impl HasSource for TypeAliasId {
    type Value = ast::TypeAlias;

    fn source<CTX: SrcDefCacheContext>(
        self,
        db: &dyn DefDatabase,
        ctx: &CTX,
    ) -> InFile<Self::Value> {
        let loc = self.lookup(db);
        let file_id = loc.item_tree_id().file_id();
        let ast_ptr = ctx.get_or_inset_with(def_to_src::TYPE_ALIAS, self, || ast_ptr(loc, db));
        InFile::new(file_id, ast_ptr.to_node(&ctx.parse_or_expand(file_id)))
    }
}

impl HasSource for Macro2Id {
    type Value = ast::MacroDef;

    fn source<CTX: SrcDefCacheContext>(
        self,
        db: &dyn DefDatabase,
        ctx: &CTX,
    ) -> InFile<Self::Value> {
        let loc = self.lookup(db);
        let file_id = loc.item_tree_id().file_id();
        let ast_ptr = ctx.get_or_inset_with(def_to_src::MACRO2, self, || ast_ptr(loc, db));
        InFile::new(file_id, ast_ptr.to_node(&ctx.parse_or_expand(file_id)))
    }
}

impl HasSource for MacroRulesId {
    type Value = ast::MacroRules;

    fn source<CTX: SrcDefCacheContext>(
        self,
        db: &dyn DefDatabase,
        ctx: &CTX,
    ) -> InFile<Self::Value> {
        let loc = self.lookup(db);
        let file_id = loc.item_tree_id().file_id();
        let ast_ptr = ctx.get_or_inset_with(def_to_src::MACRO_RULES, self, || ast_ptr(loc, db));
        InFile::new(file_id, ast_ptr.to_node(&ctx.parse_or_expand(file_id)))
    }
}

impl HasSource for ProcMacroId {
    type Value = ast::Fn;

    fn source<CTX: SrcDefCacheContext>(
        self,
        db: &dyn DefDatabase,
        ctx: &CTX,
    ) -> InFile<Self::Value> {
        let loc = self.lookup(db);
        let file_id = loc.item_tree_id().file_id();
        let ast_ptr = ctx.get_or_inset_with(def_to_src::PROC_MACRO, self, || ast_ptr(loc, db));
        InFile::new(file_id, ast_ptr.to_node(&ctx.parse_or_expand(file_id)))
    }
}

impl HasSource for ImplId {
    type Value = ast::Impl;

    fn source<CTX: SrcDefCacheContext>(
        self,
        db: &dyn DefDatabase,
        ctx: &CTX,
    ) -> InFile<Self::Value> {
        let loc = self.lookup(db);
        let file_id = loc.item_tree_id().file_id();
        let ast_ptr = ctx.get_or_inset_with(def_to_src::IMPL, self, || ast_ptr(loc, db));
        InFile::new(file_id, ast_ptr.to_node(&ctx.parse_or_expand(file_id)))
    }
}

fn ast_ptr<Loc>(
    loc: Loc,
    db: &dyn DefDatabase,
) -> AstPtr<<<Loc as ItemTreeLoc>::Id as ItemTreeNode>::Source>
where
    Loc: ItemTreeLoc,
    Loc::Id: ItemTreeNode,
{
    let id = loc.item_tree_id();
    let file_id = id.file_id();
    let tree = id.item_tree(db);
    let ast_id_map = db.ast_id_map(file_id);
    let node = &tree[id.value];

    ast_id_map.get(node.ast_id())
}
