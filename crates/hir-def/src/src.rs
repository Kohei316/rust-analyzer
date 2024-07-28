//! Utilities for mapping between hir IDs and the surface syntax.

use either::Either;
use hir_expand::{InFile, Lookup};
use la_arena::ArenaMap;
use profile::countme::Counts;
use span::AstIdNode;
use syntax::{ast, AstNode, AstPtr};

use crate::{
    data::adt::lower_struct,
    db::DefDatabase,
    dyn_map::{
        def_to_src::{self, DefIdPolicy},
        Key,
    },
    item_tree::ItemTreeNode,
    src_def_cashe::SrcDefCacheContext,
    trace::Trace,
    ConstId, EnumId, EnumVariantId, ExternBlockId, ExternCrateId, FunctionId, GenericDefId, ImplId,
    ItemTreeLoc, LocalFieldId, LocalLifetimeParamId, LocalTypeOrConstParamId, Macro2Id,
    MacroRulesId, ProcMacroId, StaticId, StructId, TraitAliasId, TraitId, TypeAliasId, UnionId,
    UseId, VariantId,
};

pub trait HasSource
where
    Self: Sized,
    Self: Copy,
    Self: for<'db> Lookup<Database<'db> = dyn DefDatabase + 'db>,
    <Self as Lookup>::Data: ItemTreeLoc,
    <<Self as Lookup>::Data as ItemTreeLoc>::Id: ItemTreeNode<Source = Self::Value>,
{
    type Value: AstNode + AstIdNode;

    fn source(&self, db: &dyn DefDatabase) -> InFile<Self::Value> {
        let InFile { file_id, value } = self.ast_ptr(db);
        InFile::new(file_id, value.to_node(&db.parse_or_expand(file_id)))
    }

    fn ast_ptr(&self, db: &dyn DefDatabase) -> InFile<AstPtr<Self::Value>> {
        self.ast_ptr_by(db, |this| {
            let loc = this.lookup(db);
            let id = loc.item_tree_id();
            let file_id = id.file_id();
            let tree = id.item_tree(db);
            let ast_id_map = db.ast_id_map(file_id);
            let node = &tree[id.value];

            ast_id_map.get(node.ast_id())
        })
    }

    fn source_with_ctx<Ctx: SrcDefCacheContext>(
        &self,
        db: &dyn DefDatabase,
        ctx: &Option<Ctx>,
    ) -> InFile<Self::Value> {
        let InFile { file_id, value } = self.ast_ptr_with(db, ctx);
        InFile::new(file_id, value.to_node(&db.parse_or_expand(file_id)))
    }

    fn ast_ptr_with<Ctx: SrcDefCacheContext>(
        &self,
        db: &dyn DefDatabase,
        ctx: &Option<Ctx>,
    ) -> InFile<AstPtr<Self::Value>>;

    fn ast_ptr_by<F>(&self, db: &dyn DefDatabase, f: F) -> InFile<AstPtr<Self::Value>>
    where
        F: FnOnce(&Self) -> AstPtr<Self::Value>,
    {
        let file_id = self.lookup(db).item_tree_id().file_id();
        let ast_ptr = f(&self);

        InFile::new(file_id, ast_ptr)
    }

    fn ast_ptr_by_key<Ctx: SrcDefCacheContext>(
        &self,
        db: &dyn DefDatabase,
        ctx: &Option<Ctx>,
        map_key: Key<Self, AstPtr<Self::Value>, DefIdPolicy<Self, Self::Value>>,
    ) -> InFile<AstPtr<Self::Value>> {
        let file_id = self.lookup(db).item_tree_id().file_id();
        ctx.as_ref()
            .map(|ctx| {
                let ast_ptr = ctx.get_or_inset_with(map_key, *self, || self.ast_ptr(db).value);
                InFile::new(file_id, ast_ptr)
            })
            .unwrap_or_else(|| self.ast_ptr(db))
    }
}

impl HasSource for StructId {
    type Value = ast::Struct;

    fn ast_ptr_with<Ctx: SrcDefCacheContext>(
        &self,
        db: &dyn DefDatabase,
        ctx: &Option<Ctx>,
    ) -> InFile<AstPtr<Self::Value>> {
        self.ast_ptr_by_key(db, ctx, def_to_src::STRUCT)
    }
}

impl HasSource for UnionId {
    type Value = ast::Union;

    fn ast_ptr_with<Ctx: SrcDefCacheContext>(
        &self,
        db: &dyn DefDatabase,
        ctx: &Option<Ctx>,
    ) -> InFile<AstPtr<Self::Value>> {
        self.ast_ptr_by_key(db, ctx, def_to_src::UNION)
    }
}

impl HasSource for EnumId {
    type Value = ast::Enum;

    fn ast_ptr_with<Ctx: SrcDefCacheContext>(
        &self,
        db: &dyn DefDatabase,
        ctx: &Option<Ctx>,
    ) -> InFile<AstPtr<Self::Value>> {
        self.ast_ptr_by_key(db, ctx, def_to_src::ENUM)
    }
}

impl HasSource for EnumVariantId {
    type Value = ast::Variant;

    fn ast_ptr_with<Ctx: SrcDefCacheContext>(
        &self,
        db: &dyn DefDatabase,
        ctx: &Option<Ctx>,
    ) -> InFile<AstPtr<Self::Value>> {
        self.ast_ptr_by_key(db, ctx, def_to_src::ENUM_VARIANT)
    }
}

impl HasSource for FunctionId {
    type Value = ast::Fn;

    fn ast_ptr_with<Ctx: SrcDefCacheContext>(
        &self,
        db: &dyn DefDatabase,
        ctx: &Option<Ctx>,
    ) -> InFile<AstPtr<Self::Value>> {
        self.ast_ptr_by_key(db, ctx, def_to_src::FUNCTION)
    }
}

impl HasSource for ConstId {
    type Value = ast::Const;

    fn ast_ptr_with<Ctx: SrcDefCacheContext>(
        &self,
        db: &dyn DefDatabase,
        ctx: &Option<Ctx>,
    ) -> InFile<AstPtr<Self::Value>> {
        self.ast_ptr_by_key(db, ctx, def_to_src::CONST)
    }
}

impl HasSource for StaticId {
    type Value = ast::Static;

    fn ast_ptr_with<Ctx: SrcDefCacheContext>(
        &self,
        db: &dyn DefDatabase,
        ctx: &Option<Ctx>,
    ) -> InFile<AstPtr<Self::Value>> {
        self.ast_ptr_by_key(db, ctx, def_to_src::STATIC)
    }
}

impl HasSource for TraitId {
    type Value = ast::Trait;

    fn ast_ptr_with<Ctx: SrcDefCacheContext>(
        &self,
        db: &dyn DefDatabase,
        ctx: &Option<Ctx>,
    ) -> InFile<AstPtr<Self::Value>> {
        self.ast_ptr_by_key(db, ctx, def_to_src::TRAIT)
    }
}

impl HasSource for TraitAliasId {
    type Value = ast::TraitAlias;

    fn ast_ptr_with<Ctx: SrcDefCacheContext>(
        &self,
        db: &dyn DefDatabase,
        ctx: &Option<Ctx>,
    ) -> InFile<AstPtr<Self::Value>> {
        self.ast_ptr_by_key(db, ctx, def_to_src::TRAIT_ALIAS)
    }
}

impl HasSource for TypeAliasId {
    type Value = ast::TypeAlias;

    fn ast_ptr_with<Ctx: SrcDefCacheContext>(
        &self,
        db: &dyn DefDatabase,
        ctx: &Option<Ctx>,
    ) -> InFile<AstPtr<Self::Value>> {
        self.ast_ptr_by_key(db, ctx, def_to_src::TYPE_ALIAS)
    }
}

impl HasSource for Macro2Id {
    type Value = ast::MacroDef;

    fn ast_ptr_with<Ctx: SrcDefCacheContext>(
        &self,
        db: &dyn DefDatabase,
        ctx: &Option<Ctx>,
    ) -> InFile<AstPtr<Self::Value>> {
        self.ast_ptr_by_key(db, ctx, def_to_src::MACRO2)
    }
}

impl HasSource for MacroRulesId {
    type Value = ast::MacroRules;

    fn ast_ptr_with<Ctx: SrcDefCacheContext>(
        &self,
        db: &dyn DefDatabase,
        ctx: &Option<Ctx>,
    ) -> InFile<AstPtr<Self::Value>> {
        self.ast_ptr_by_key(db, ctx, def_to_src::MACRO_RULES)
    }
}

impl HasSource for ProcMacroId {
    type Value = ast::Fn;

    fn ast_ptr_with<Ctx: SrcDefCacheContext>(
        &self,
        db: &dyn DefDatabase,
        ctx: &Option<Ctx>,
    ) -> InFile<AstPtr<Self::Value>> {
        self.ast_ptr_by_key(db, ctx, def_to_src::PROC_MACRO)
    }
}

impl HasSource for ImplId {
    type Value = ast::Impl;

    fn ast_ptr_with<Ctx: SrcDefCacheContext>(
        &self,
        db: &dyn DefDatabase,
        ctx: &Option<Ctx>,
    ) -> InFile<AstPtr<Self::Value>> {
        self.ast_ptr_by_key(db, ctx, def_to_src::IMPL)
    }
}

impl HasSource for ExternCrateId {
    type Value = ast::ExternCrate;

    fn ast_ptr_with<Ctx: SrcDefCacheContext>(
        &self,
        db: &dyn DefDatabase,
        ctx: &Option<Ctx>,
    ) -> InFile<AstPtr<Self::Value>> {
        self.ast_ptr_by_key(db, ctx, def_to_src::EXTERN_CRATE)
    }
}

impl HasSource for ExternBlockId {
    type Value = ast::ExternBlock;

    fn ast_ptr_with<Ctx: SrcDefCacheContext>(
        &self,
        db: &dyn DefDatabase,
        ctx: &Option<Ctx>,
    ) -> InFile<AstPtr<Self::Value>> {
        self.ast_ptr_by_key(db, ctx, def_to_src::EXTERN_BLOCK)
    }
}

impl HasSource for UseId {
    type Value = ast::Use;

    fn ast_ptr_with<Ctx: SrcDefCacheContext>(
        &self,
        db: &dyn DefDatabase,
        ctx: &Option<Ctx>,
    ) -> InFile<AstPtr<Self::Value>> {
        self.ast_ptr_by_key(db, ctx, def_to_src::USE)
    }
}

pub trait HasChildSource<ChildId> {
    type Value;
    fn child_source(&self, db: &dyn DefDatabase) -> InFile<ArenaMap<ChildId, Self::Value>>;

    // fn _child_source(&self, db: &dyn DefDatabase, f: F) -> InFile<ArenaMap<ChildId, Self::Value>>
    // where
    //     F: FnOnce(Id) -> InFile<Id::Value>;
}

impl HasChildSource<la_arena::Idx<ast::UseTree>> for UseId {
    type Value = ast::UseTree;
    fn child_source(
        &self,
        db: &dyn DefDatabase,
    ) -> InFile<ArenaMap<la_arena::Idx<ast::UseTree>, Self::Value>> {
        let loc = &self.lookup(db);
        let use_ = &loc.id.item_tree(db)[loc.id.value];
        InFile::new(
            loc.id.file_id(),
            use_.use_tree_source_map(db, loc.id.file_id()).into_iter().collect(),
        )
    }
}

impl HasChildSource<LocalTypeOrConstParamId> for GenericDefId {
    type Value = Either<ast::TypeOrConstParam, ast::TraitOrAlias>;
    fn child_source(
        &self,
        db: &dyn DefDatabase,
    ) -> InFile<ArenaMap<LocalTypeOrConstParamId, Self::Value>> {
        let generic_params = db.generic_params(*self);
        let mut idx_iter = generic_params.iter_type_or_consts().map(|(idx, _)| idx);

        let (file_id, generic_params_list) = self.file_id_and_params_of(db);

        let mut params = ArenaMap::default();

        // For traits and trait aliases the first type index is `Self`, we need to add it before
        // the other params.
        match *self {
            GenericDefId::TraitId(id) => {
                let trait_ref = id.source(db).value;
                let idx = idx_iter.next().unwrap();
                params.insert(idx, Either::Right(ast::TraitOrAlias::Trait(trait_ref)));
            }
            GenericDefId::TraitAliasId(id) => {
                let alias = id.source(db).value;
                let idx = idx_iter.next().unwrap();
                params.insert(idx, Either::Right(ast::TraitOrAlias::TraitAlias(alias)));
            }
            _ => {}
        }

        if let Some(generic_params_list) = generic_params_list {
            for (idx, ast_param) in idx_iter.zip(generic_params_list.type_or_const_params()) {
                params.insert(idx, Either::Left(ast_param));
            }
        }

        InFile::new(file_id, params)
    }
}

impl HasChildSource<LocalLifetimeParamId> for GenericDefId {
    type Value = ast::LifetimeParam;
    fn child_source(
        &self,
        db: &dyn DefDatabase,
    ) -> InFile<ArenaMap<LocalLifetimeParamId, Self::Value>> {
        let generic_params = db.generic_params(*self);
        let idx_iter = generic_params.iter_lt().map(|(idx, _)| idx);

        let (file_id, generic_params_list) = self.file_id_and_params_of(db);

        let mut params = ArenaMap::default();

        if let Some(generic_params_list) = generic_params_list {
            for (idx, ast_param) in idx_iter.zip(generic_params_list.lifetime_params()) {
                params.insert(idx, ast_param);
            }
        }

        InFile::new(file_id, params)
    }
}

impl HasChildSource<LocalFieldId> for VariantId {
    type Value = Either<ast::TupleField, ast::RecordField>;

    fn child_source(&self, db: &dyn DefDatabase) -> InFile<ArenaMap<LocalFieldId, Self::Value>> {
        let item_tree;
        let (src, fields, container) = match *self {
            VariantId::EnumVariantId(it) => {
                let lookup = it.lookup(db);
                item_tree = it.lookup(db).id.item_tree(db);
                (
                    it.source(db).map(|it| it.kind()),
                    &item_tree[lookup.id.value].fields,
                    lookup.parent.lookup(db).container,
                )
            }
            VariantId::StructId(it) => {
                let lookup = it.lookup(db);
                item_tree = lookup.id.item_tree(db);
                (
                    it.source(db).map(|it| it.kind()),
                    &item_tree[lookup.id.value].fields,
                    lookup.container,
                )
            }
            VariantId::UnionId(it) => {
                let lookup = it.lookup(db);
                item_tree = lookup.id.item_tree(db);
                (
                    it.source(db).map(|it| it.kind()),
                    &item_tree[lookup.id.value].fields,
                    lookup.container,
                )
            }
        };
        let mut trace = Trace::new_for_map();
        lower_struct(db, &mut trace, &src, container.krate, &item_tree, fields);
        src.with_value(trace.into_map())
    }
}
