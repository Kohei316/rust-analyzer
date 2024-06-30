//! Maps *syntax* of various definitions to their semantic ids.
//!
//! This is a very interesting module, and, in some sense, can be considered the
//! heart of the IDE parts of rust-analyzer.
//!
//! This module solves the following problem:
//!
//!     Given a piece of syntax, find the corresponding semantic definition (def).
//!
//! This problem is a part of more-or-less every IDE feature implemented. Every
//! IDE functionality (like goto to definition), conceptually starts with a
//! specific cursor position in a file. Starting with this text offset, we first
//! figure out what syntactic construct are we at: is this a pattern, an
//! expression, an item definition.
//!
//! Knowing only the syntax gives us relatively little info. For example,
//! looking at the syntax of the function we can realize that it is a part of an
//! `impl` block, but we won't be able to tell what trait function the current
//! function overrides, and whether it does that correctly. For that, we need to
//! go from [`ast::Fn`] to [`crate::Function`], and that's exactly what this
//! module does.
//!
//! As syntax trees are values and don't know their place of origin/identity,
//! this module also requires [`InFile`] wrappers to understand which specific
//! real or macro-expanded file the tree comes from.
//!
//! The actual algorithm to resolve syntax to def is curious in two aspects:
//!
//! * It is recursive
//! * It uses the inverse algorithm (what is the syntax for this def?)
//!
//! Specifically, the algorithm goes like this:
//!
//! 1. Find the syntactic container for the syntax. For example, field's
//!    container is the struct, and structs container is a module.
//! 2. Recursively get the def corresponding to container.
//! 3. Ask the container def for all child defs. These child defs contain
//!    the answer and answer's siblings.
//! 4. For each child def, ask for it's source.
//! 5. The child def whose source is the syntax node we've started with
//!    is the answer.
//!
//! It's interesting that both Roslyn and Kotlin contain very similar code
//! shape.
//!
//! Let's take a look at Roslyn:
//!
//!   <https://github.com/dotnet/roslyn/blob/36a0c338d6621cc5fe34b79d414074a95a6a489c/src/Compilers/CSharp/Portable/Compilation/SyntaxTreeSemanticModel.cs#L1403-L1429>
//!   <https://sourceroslyn.io/#Microsoft.CodeAnalysis.CSharp/Compilation/SyntaxTreeSemanticModel.cs,1403>
//!
//! The `GetDeclaredType` takes `Syntax` as input, and returns `Symbol` as
//! output. First, it retrieves a `Symbol` for parent `Syntax`:
//!
//! * <https://sourceroslyn.io/#Microsoft.CodeAnalysis.CSharp/Compilation/SyntaxTreeSemanticModel.cs,1423>
//!
//! Then, it iterates parent symbol's children, looking for one which has the
//! same text span as the original node:
//!
//!   <https://sourceroslyn.io/#Microsoft.CodeAnalysis.CSharp/Compilation/SyntaxTreeSemanticModel.cs,1786>
//!
//! Now, let's look at Kotlin:
//!
//!   <https://github.com/JetBrains/kotlin/blob/a288b8b00e4754a1872b164999c6d3f3b8c8994a/idea/idea-frontend-fir/idea-fir-low-level-api/src/org/jetbrains/kotlin/idea/fir/low/level/api/FirModuleResolveStateImpl.kt#L93-L125>
//!
//! This function starts with a syntax node (`KtExpression` is syntax, like all
//! `Kt` nodes), and returns a def. It uses
//! `getNonLocalContainingOrThisDeclaration` to get syntactic container for a
//! current node. Then, `findSourceNonLocalFirDeclaration` gets `Fir` for this
//! parent. Finally, `findElementIn` function traverses `Fir` children to find
//! one with the same source we originally started with.
//!
//! One question is left though -- where does the recursion stops? This happens
//! when we get to the file syntax node, which doesn't have a syntactic parent.
//! In that case, we loop through all the crates that might contain this file
//! and look for a module whose source is the given file.
//!
//! Note that the logic in this module is somewhat fundamentally imprecise --
//! due to conditional compilation and `#[path]` attributes, there's no
//! injective mapping from syntax nodes to defs. This is not an edge case --
//! more or less every item in a `lib.rs` is a part of two distinct crates: a
//! library with `--cfg test` and a library without.
//!
//! At the moment, we don't really handle this well and return the first answer
//! that works. Ideally, we should first let the caller to pick a specific
//! active crate for a given position, and then provide an API to resolve all
//! syntax nodes against this specific crate.

use either::Either;
use hir_def::{
    child_by_source::ChildBySource,
    dyn_map::{
        keys::src_to_def::{self, Key},
        DynMap,
    },
    hir::{BindingId, LabelId},
    src::DefToSourceContext,
    AdtId, BlockId, ConstId, ConstParamId, DefWithBodyId, EnumId, EnumVariantId, ExternCrateId,
    FieldId, FunctionId, GenericDefId, GenericParamId, ImplId, LifetimeParamId, Lookup, MacroId,
    ModuleId, StaticId, StructId, TraitAliasId, TraitId, TypeAliasId, TypeParamId, UnionId, UseId,
    VariantId,
};
use hir_expand::{
    attrs::AttrId, db::ExpandDatabase, name::AsName, ExpansionInfo, HirFileId, HirFileIdExt,
    MacroCallId, MacroFileIdExt,
};
use rustc_hash::FxHashMap;
use smallvec::SmallVec;
use span::{FileId, MacroFileId};
use stdx::impl_from;
use syntax::{
    ast::{self, HasName},
    AstNode, AstPtr, SyntaxNode,
};

use crate::{db::HirDatabase, InFile};

#[derive(Default)]
pub(super) struct SourceToDefCache {
    pub(super) dynmap_cache: FxHashMap<(ChildContainer, HirFileId), DynMap>,
    pub(super) expansion_info_cache: FxHashMap<MacroFileId, ExpansionInfo>,
    pub(super) file_to_def_cache: FxHashMap<FileId, SmallVec<[ModuleId; 1]>>,
}

pub(super) struct SourceToDefCtx<'db, 'cache> {
    pub(super) db: &'db dyn HirDatabase,
    pub(super) cache: &'cache mut SourceToDefCache,
}

impl SourceToDefCtx<'_, '_> {
    pub(super) fn file_to_def(&mut self, file: FileId) -> &SmallVec<[ModuleId; 1]> {
        let _p = tracing::info_span!("SourceToDefCtx::file_to_def").entered();
        self.cache.file_to_def_cache.entry(file).or_insert_with(|| {
            let mut mods = SmallVec::new();
            for &crate_id in self.db.relevant_crates(file).iter() {
                // Note: `mod` declarations in block modules cannot be supported here
                let crate_def_map = self.db.crate_def_map(crate_id);
                let n_mods = mods.len();
                let modules = |file| {
                    crate_def_map
                        .modules_for_file(file)
                        .map(|local_id| crate_def_map.module_id(local_id))
                };
                mods.extend(modules(file));
                if mods.len() == n_mods {
                    mods.extend(
                        self.db
                            .include_macro_invoc(crate_id)
                            .iter()
                            .filter(|&&(_, file_id)| file_id == file)
                            .flat_map(|(call, _)| {
                                modules(
                                    call.lookup(self.db.upcast())
                                        .kind
                                        .file_id()
                                        .original_file(self.db.upcast())
                                        .file_id(),
                                )
                            }),
                    );
                }
            }
            if mods.is_empty() {
                // FIXME: detached file
            }
            mods
        })
    }

    pub(super) fn module_to_def(
        &mut self,
        src: InFile<&ast::Module>,
        d2s_ctx: &mut DefToSourceContext<'_>,
    ) -> Option<ModuleId> {
        let _p = tracing::info_span!("module_to_def").entered();
        let parent_declaration = self
            .ancestors_with_macros(src.syntax_ref(), |_, ancestor| {
                ancestor.map(Either::<ast::Module, ast::BlockExpr>::cast).transpose()
            })
            .map(|it| it.transpose());

        let parent_module = match parent_declaration {
            Some(Either::Right(parent_block)) => self
                .block_to_def(parent_block.as_ref(), d2s_ctx)
                .map(|block| self.db.block_def_map(block).root_module_id()),
            Some(Either::Left(parent_declaration)) => {
                self.module_to_def(parent_declaration.as_ref(), d2s_ctx)
            }
            None => {
                let file_id = src.file_id.original_file(self.db.upcast());
                self.file_to_def(file_id.file_id()).first().copied()
            }
        }?;

        let child_name = src.value.name()?.as_name();
        let def_map = parent_module.def_map(self.db.upcast());
        let &child_id = def_map[parent_module.local_id].children.get(&child_name)?;
        Some(def_map.module_id(child_id))
    }

    pub(super) fn macro_call_to_macro_id(
        &mut self,
        macro_call_id: MacroCallId,
        d2s_ctx: &mut DefToSourceContext<'_>,
    ) -> Option<MacroId> {
        use span::HirFileIdRepr;

        let db: &dyn ExpandDatabase = self.db.upcast();
        let loc = db.lookup_intern_macro_call(macro_call_id);

        match loc.def.ast_id() {
            Either::Left(it) => {
                let node = match it.file_id.repr() {
                    HirFileIdRepr::FileId(file_id) => {
                        it.to_ptr(db).to_node(&db.parse(file_id).syntax_node())
                    }
                    HirFileIdRepr::MacroFile(macro_file) => {
                        let expansion_info = self
                            .cache
                            .expansion_info_cache
                            .entry(macro_file)
                            .or_insert_with(|| macro_file.expansion_info(self.db.upcast()));
                        it.to_ptr(db).to_node(&expansion_info.expanded().value)
                    }
                };
                self.macro_to_def(InFile::new(it.file_id, &node), d2s_ctx)
            }
            Either::Right(it) => {
                let node = match it.file_id.repr() {
                    HirFileIdRepr::FileId(file_id) => {
                        it.to_ptr(db).to_node(&db.parse(file_id).syntax_node())
                    }
                    HirFileIdRepr::MacroFile(macro_file) => {
                        let expansion_info = self
                            .cache
                            .expansion_info_cache
                            .entry(macro_file)
                            .or_insert_with(|| macro_file.expansion_info(self.db.upcast()));
                        it.to_ptr(db).to_node(&expansion_info.expanded().value)
                    }
                };
                self.proc_macro_to_def(InFile::new(it.file_id, &node), d2s_ctx)
            }
        }
    }

    pub(super) fn source_file_to_def(
        &mut self,
        src: InFile<&ast::SourceFile>,
        _d2s_ctx: &mut DefToSourceContext<'_>,
    ) -> Option<ModuleId> {
        let _p = tracing::info_span!("source_file_to_def").entered();
        let file_id = src.file_id.original_file(self.db.upcast());
        self.file_to_def(file_id.file_id()).first().copied()
    }

    pub(super) fn trait_to_def(
        &mut self,
        src: InFile<&ast::Trait>,
        d2s_ctx: &mut DefToSourceContext<'_>,
    ) -> Option<TraitId> {
        self.to_def(src, src_to_def::TRAIT, d2s_ctx)
    }
    pub(super) fn trait_alias_to_def(
        &mut self,
        src: InFile<&ast::TraitAlias>,
        d2s_ctx: &mut DefToSourceContext<'_>,
    ) -> Option<TraitAliasId> {
        self.to_def(src, src_to_def::TRAIT_ALIAS, d2s_ctx)
    }
    pub(super) fn impl_to_def(
        &mut self,
        src: InFile<&ast::Impl>,
        d2s_ctx: &mut DefToSourceContext<'_>,
    ) -> Option<ImplId> {
        self.to_def(src, src_to_def::IMPL, d2s_ctx)
    }
    pub(super) fn fn_to_def(
        &mut self,
        src: InFile<&ast::Fn>,
        d2s_ctx: &mut DefToSourceContext<'_>,
    ) -> Option<FunctionId> {
        self.to_def(src, src_to_def::FUNCTION, d2s_ctx)
    }
    pub(super) fn struct_to_def(
        &mut self,
        src: InFile<&ast::Struct>,
        d2s_ctx: &mut DefToSourceContext<'_>,
    ) -> Option<StructId> {
        self.to_def(src, src_to_def::STRUCT, d2s_ctx)
    }
    pub(super) fn enum_to_def(
        &mut self,
        src: InFile<&ast::Enum>,
        d2s_ctx: &mut DefToSourceContext<'_>,
    ) -> Option<EnumId> {
        self.to_def(src, src_to_def::ENUM, d2s_ctx)
    }
    pub(super) fn union_to_def(
        &mut self,
        src: InFile<&ast::Union>,
        d2s_ctx: &mut DefToSourceContext<'_>,
    ) -> Option<UnionId> {
        self.to_def(src, src_to_def::UNION, d2s_ctx)
    }
    pub(super) fn static_to_def(
        &mut self,
        src: InFile<&ast::Static>,
        d2s_ctx: &mut DefToSourceContext<'_>,
    ) -> Option<StaticId> {
        self.to_def(src, src_to_def::STATIC, d2s_ctx)
    }
    pub(super) fn const_to_def(
        &mut self,
        src: InFile<&ast::Const>,
        d2s_ctx: &mut DefToSourceContext<'_>,
    ) -> Option<ConstId> {
        self.to_def(src, src_to_def::CONST, d2s_ctx)
    }
    pub(super) fn type_alias_to_def(
        &mut self,
        src: InFile<&ast::TypeAlias>,
        d2s_ctx: &mut DefToSourceContext<'_>,
    ) -> Option<TypeAliasId> {
        self.to_def(src, src_to_def::TYPE_ALIAS, d2s_ctx)
    }
    pub(super) fn record_field_to_def(
        &mut self,
        src: InFile<&ast::RecordField>,
        d2s_ctx: &mut DefToSourceContext<'_>,
    ) -> Option<FieldId> {
        self.to_def(src, src_to_def::RECORD_FIELD, d2s_ctx)
    }
    pub(super) fn tuple_field_to_def(
        &mut self,
        src: InFile<&ast::TupleField>,
        d2s_ctx: &mut DefToSourceContext<'_>,
    ) -> Option<FieldId> {
        self.to_def(src, src_to_def::TUPLE_FIELD, d2s_ctx)
    }
    pub(super) fn block_to_def(
        &mut self,
        src: InFile<&ast::BlockExpr>,
        d2s_ctx: &mut DefToSourceContext<'_>,
    ) -> Option<BlockId> {
        self.to_def(src, src_to_def::BLOCK, d2s_ctx)
    }
    pub(super) fn enum_variant_to_def(
        &mut self,
        src: InFile<&ast::Variant>,
        d2s_ctx: &mut DefToSourceContext<'_>,
    ) -> Option<EnumVariantId> {
        self.to_def(src, src_to_def::ENUM_VARIANT, d2s_ctx)
    }
    pub(super) fn extern_crate_to_def(
        &mut self,
        src: InFile<&ast::ExternCrate>,
        d2s_ctx: &mut DefToSourceContext<'_>,
    ) -> Option<ExternCrateId> {
        self.to_def(src, src_to_def::EXTERN_CRATE, d2s_ctx)
    }
    #[allow(dead_code)]
    pub(super) fn use_to_def(
        &mut self,
        src: InFile<&ast::Use>,
        d2s_ctx: &mut DefToSourceContext<'_>,
    ) -> Option<UseId> {
        self.to_def(src, src_to_def::USE, d2s_ctx)
    }
    pub(super) fn adt_to_def(
        &mut self,
        InFile { file_id, value }: InFile<&ast::Adt>,
        d2s_ctx: &mut DefToSourceContext<'_>,
    ) -> Option<AdtId> {
        match value {
            ast::Adt::Enum(it) => {
                self.enum_to_def(InFile::new(file_id, it), d2s_ctx).map(AdtId::EnumId)
            }
            ast::Adt::Struct(it) => {
                self.struct_to_def(InFile::new(file_id, it), d2s_ctx).map(AdtId::StructId)
            }
            ast::Adt::Union(it) => {
                self.union_to_def(InFile::new(file_id, it), d2s_ctx).map(AdtId::UnionId)
            }
        }
    }
    pub(super) fn bind_pat_to_def(
        &mut self,
        src: InFile<&ast::IdentPat>,
        d2s_ctx: &mut DefToSourceContext<'_>,
    ) -> Option<(DefWithBodyId, BindingId)> {
        let container = self.find_pat_or_label_container(src.syntax_ref(), d2s_ctx)?;
        let (body, source_map) = self.db.body_with_source_map(container);
        let src = src.cloned().map(ast::Pat::from);
        let pat_id = source_map.node_pat(src.as_ref())?;
        // the pattern could resolve to a constant, verify that that is not the case
        if let crate::Pat::Bind { id, .. } = body[pat_id] {
            Some((container, id))
        } else {
            None
        }
    }
    pub(super) fn self_param_to_def(
        &mut self,
        src: InFile<&ast::SelfParam>,
        d2s_ctx: &mut DefToSourceContext<'_>,
    ) -> Option<(DefWithBodyId, BindingId)> {
        let container = self.find_pat_or_label_container(src.syntax_ref(), d2s_ctx)?;
        let body = self.db.body(container);
        Some((container, body.self_param?))
    }
    pub(super) fn label_to_def(
        &mut self,
        src: InFile<&ast::Label>,
        d2s_ctx: &mut DefToSourceContext<'_>,
    ) -> Option<(DefWithBodyId, LabelId)> {
        let container = self.find_pat_or_label_container(src.syntax_ref(), d2s_ctx)?;
        let (_body, source_map) = self.db.body_with_source_map(container);
        let label_id = source_map.node_label(src)?;
        Some((container, label_id))
    }

    pub(super) fn item_to_macro_call(
        &mut self,
        src: InFile<&ast::Item>,
        d2s_ctx: &mut DefToSourceContext<'_>,
    ) -> Option<MacroCallId> {
        let map = self.dyn_map(src, d2s_ctx)?;
        map[src_to_def::ATTR_MACRO_CALL].get(&AstPtr::new(src.value)).copied()
    }

    pub(super) fn macro_call_to_macro_call(
        &mut self,
        src: InFile<&ast::MacroCall>,
        d2s_ctx: &mut DefToSourceContext<'_>,
    ) -> Option<MacroCallId> {
        let map = self.dyn_map(src, d2s_ctx)?;
        map[src_to_def::MACRO_CALL].get(&AstPtr::new(src.value)).copied()
    }

    /// (AttrId, derive attribute call id, derive call ids)
    pub(super) fn attr_to_derive_macro_call(
        &mut self,
        item: InFile<&ast::Adt>,
        src: InFile<ast::Attr>,
        d2s_ctx: &mut DefToSourceContext<'_>,
    ) -> Option<(AttrId, MacroCallId, &[Option<MacroCallId>])> {
        let map = self.dyn_map(item, d2s_ctx)?;
        map[src_to_def::DERIVE_MACRO_CALL]
            .get(&AstPtr::new(&src.value))
            .map(|&(attr_id, call_id, ref ids)| (attr_id, call_id, &**ids))
    }

    pub(super) fn has_derives(
        &mut self,
        adt: InFile<&ast::Adt>,
        d2s_ctx: &mut DefToSourceContext<'_>,
    ) -> bool {
        self.dyn_map(adt, d2s_ctx)
            .as_ref()
            .map_or(false, |map| !map[src_to_def::DERIVE_MACRO_CALL].is_empty())
    }

    fn to_def<Ast: AstNode + 'static, ID: Copy + 'static>(
        &mut self,
        src: InFile<&Ast>,
        key: Key<Ast, ID>,
        d2s_ctx: &mut DefToSourceContext<'_>,
    ) -> Option<ID> {
        self.dyn_map(src, d2s_ctx)?[key].get(&AstPtr::new(src.value)).copied()
    }

    fn dyn_map<Ast: AstNode + 'static>(
        &mut self,
        src: InFile<&Ast>,
        d2s_ctx: &mut DefToSourceContext<'_>,
    ) -> Option<&DynMap> {
        let container = self.find_container(src.map(|it| it.syntax()), d2s_ctx)?;
        Some(self.cache_for(container, src.file_id, d2s_ctx))
    }

    fn cache_for(
        &mut self,
        container: ChildContainer,
        file_id: HirFileId,
        d2s_ctx: &mut DefToSourceContext<'_>,
    ) -> &DynMap {
        let db = self.db;
        self.cache
            .dynmap_cache
            .entry((container, file_id))
            .or_insert_with(|| container.child_by_source(db, d2s_ctx, file_id))
    }

    pub(super) fn type_param_to_def(
        &mut self,
        src: InFile<&ast::TypeParam>,
        d2s_ctx: &mut DefToSourceContext<'_>,
    ) -> Option<TypeParamId> {
        let container: ChildContainer =
            self.find_generic_param_container(src.syntax_ref(), d2s_ctx)?.into();
        let dyn_map = self.cache_for(container, src.file_id, d2s_ctx);
        dyn_map[src_to_def::TYPE_PARAM]
            .get(&AstPtr::new(src.value))
            .copied()
            .map(TypeParamId::from_unchecked)
    }

    pub(super) fn lifetime_param_to_def(
        &mut self,
        src: InFile<&ast::LifetimeParam>,
        d2s_ctx: &mut DefToSourceContext<'_>,
    ) -> Option<LifetimeParamId> {
        let container: ChildContainer =
            self.find_generic_param_container(src.syntax_ref(), d2s_ctx)?.into();
        let dyn_map = self.cache_for(container, src.file_id, d2s_ctx);
        dyn_map[src_to_def::LIFETIME_PARAM].get(&AstPtr::new(src.value)).copied()
    }

    pub(super) fn const_param_to_def(
        &mut self,
        src: InFile<&ast::ConstParam>,
        d2s_ctx: &mut DefToSourceContext<'_>,
    ) -> Option<ConstParamId> {
        let container: ChildContainer =
            self.find_generic_param_container(src.syntax_ref(), d2s_ctx)?.into();
        let dyn_map = self.cache_for(container, src.file_id, d2s_ctx);
        dyn_map[src_to_def::CONST_PARAM]
            .get(&AstPtr::new(src.value))
            .copied()
            .map(ConstParamId::from_unchecked)
    }

    pub(super) fn generic_param_to_def(
        &mut self,
        InFile { file_id, value }: InFile<&ast::GenericParam>,
        d2s_ctx: &mut DefToSourceContext<'_>,
    ) -> Option<GenericParamId> {
        match value {
            ast::GenericParam::ConstParam(it) => self
                .const_param_to_def(InFile::new(file_id, it), d2s_ctx)
                .map(GenericParamId::ConstParamId),
            ast::GenericParam::LifetimeParam(it) => self
                .lifetime_param_to_def(InFile::new(file_id, it), d2s_ctx)
                .map(GenericParamId::LifetimeParamId),
            ast::GenericParam::TypeParam(it) => self
                .type_param_to_def(InFile::new(file_id, it), d2s_ctx)
                .map(GenericParamId::TypeParamId),
        }
    }

    pub(super) fn macro_to_def(
        &mut self,
        src: InFile<&ast::Macro>,
        d2s_ctx: &mut DefToSourceContext<'_>,
    ) -> Option<MacroId> {
        self.dyn_map(src, d2s_ctx).and_then(|it| match src.value {
            ast::Macro::MacroRules(value) => {
                it[src_to_def::MACRO_RULES].get(&AstPtr::new(value)).copied().map(MacroId::from)
            }
            ast::Macro::MacroDef(value) => {
                it[src_to_def::MACRO2].get(&AstPtr::new(value)).copied().map(MacroId::from)
            }
        })
    }

    pub(super) fn proc_macro_to_def(
        &mut self,
        src: InFile<&ast::Fn>,
        d2s_ctx: &mut DefToSourceContext<'_>,
    ) -> Option<MacroId> {
        self.dyn_map(src, d2s_ctx).and_then(|it| {
            it[src_to_def::PROC_MACRO].get(&AstPtr::new(src.value)).copied().map(MacroId::from)
        })
    }

    pub(super) fn find_container(
        &mut self,
        src: InFile<&SyntaxNode>,
        d2s_ctx: &mut DefToSourceContext<'_>,
    ) -> Option<ChildContainer> {
        let _p = tracing::info_span!("find_container").entered();
        let def = self.ancestors_with_macros(src, |this, container| {
            this.container_to_def(container, d2s_ctx)
        });
        if let Some(def) = def {
            return Some(def);
        }

        let def = self
            .file_to_def(src.file_id.original_file(self.db.upcast()).file_id())
            .first()
            .copied()?;
        Some(def.into())
    }

    /// Skips the attributed item that caused the macro invocation we are climbing up
    fn ancestors_with_macros<T>(
        &mut self,
        node: InFile<&SyntaxNode>,
        mut cb: impl FnMut(&mut Self, InFile<SyntaxNode>) -> Option<T>,
    ) -> Option<T> {
        use hir_expand::MacroFileIdExt;
        let parent = |this: &mut Self, node: InFile<&SyntaxNode>| match node.value.parent() {
            Some(parent) => Some(node.with_value(parent)),
            None => {
                let macro_file = node.file_id.macro_file()?;

                let expansion_info = this
                    .cache
                    .expansion_info_cache
                    .entry(macro_file)
                    .or_insert_with(|| macro_file.expansion_info(this.db.upcast()));

                expansion_info.arg().map(|node| node?.parent()).transpose()
            }
        };
        let mut node = node.cloned();
        while let Some(parent) = parent(self, node.as_ref()) {
            if let Some(res) = cb(self, parent.clone()) {
                return Some(res);
            }
            node = parent;
        }
        None
    }

    fn find_generic_param_container(
        &mut self,
        src: InFile<&SyntaxNode>,
        d2s_ctx: &mut DefToSourceContext<'_>,
    ) -> Option<GenericDefId> {
        self.ancestors_with_macros(src, |this, InFile { file_id, value }| {
            let item = ast::Item::cast(value)?;
            match &item {
                ast::Item::Fn(it) => {
                    this.fn_to_def(InFile::new(file_id, it), d2s_ctx).map(Into::into)
                }
                ast::Item::Struct(it) => {
                    this.struct_to_def(InFile::new(file_id, it), d2s_ctx).map(Into::into)
                }
                ast::Item::Enum(it) => {
                    this.enum_to_def(InFile::new(file_id, it), d2s_ctx).map(Into::into)
                }
                ast::Item::Trait(it) => {
                    this.trait_to_def(InFile::new(file_id, it), d2s_ctx).map(Into::into)
                }
                ast::Item::TraitAlias(it) => {
                    this.trait_alias_to_def(InFile::new(file_id, it), d2s_ctx).map(Into::into)
                }
                ast::Item::TypeAlias(it) => {
                    this.type_alias_to_def(InFile::new(file_id, it), d2s_ctx).map(Into::into)
                }
                ast::Item::Impl(it) => {
                    this.impl_to_def(InFile::new(file_id, it), d2s_ctx).map(Into::into)
                }
                _ => None,
            }
        })
    }

    fn find_pat_or_label_container(
        &mut self,
        src: InFile<&SyntaxNode>,
        d2s_ctx: &mut DefToSourceContext<'_>,
    ) -> Option<DefWithBodyId> {
        self.ancestors_with_macros(src, |this, InFile { file_id, value }| {
            let item = match ast::Item::cast(value.clone()) {
                Some(it) => it,
                None => {
                    let variant = ast::Variant::cast(value.clone())?;
                    return this
                        .enum_variant_to_def(InFile::new(file_id, &variant), d2s_ctx)
                        .map(Into::into);
                }
            };
            match &item {
                ast::Item::Fn(it) => {
                    this.fn_to_def(InFile::new(file_id, it), d2s_ctx).map(Into::into)
                }
                ast::Item::Const(it) => {
                    this.const_to_def(InFile::new(file_id, it), d2s_ctx).map(Into::into)
                }
                ast::Item::Static(it) => {
                    this.static_to_def(InFile::new(file_id, it), d2s_ctx).map(Into::into)
                }
                _ => None,
            }
        })
    }

    fn container_to_def(
        &mut self,
        container: InFile<SyntaxNode>,
        d2s_ctx: &mut DefToSourceContext<'_>,
    ) -> Option<ChildContainer> {
        let cont = if let Some(item) = ast::Item::cast(container.value.clone()) {
            match &item {
                ast::Item::Module(it) => {
                    self.module_to_def(container.with_value(it), d2s_ctx)?.into()
                }
                ast::Item::Trait(it) => {
                    self.trait_to_def(container.with_value(it), d2s_ctx)?.into()
                }
                ast::Item::TraitAlias(it) => {
                    self.trait_alias_to_def(container.with_value(it), d2s_ctx)?.into()
                }
                ast::Item::Impl(it) => self.impl_to_def(container.with_value(it), d2s_ctx)?.into(),
                ast::Item::Enum(it) => self.enum_to_def(container.with_value(it), d2s_ctx)?.into(),
                ast::Item::TypeAlias(it) => {
                    self.type_alias_to_def(container.with_value(it), d2s_ctx)?.into()
                }
                ast::Item::Struct(it) => {
                    let def = self.struct_to_def(container.with_value(it), d2s_ctx)?;
                    VariantId::from(def).into()
                }
                ast::Item::Union(it) => {
                    let def = self.union_to_def(container.with_value(it), d2s_ctx)?;
                    VariantId::from(def).into()
                }
                ast::Item::Fn(it) => {
                    let def = self.fn_to_def(container.with_value(it), d2s_ctx)?;
                    DefWithBodyId::from(def).into()
                }
                ast::Item::Static(it) => {
                    let def = self.static_to_def(container.with_value(it), d2s_ctx)?;
                    DefWithBodyId::from(def).into()
                }
                ast::Item::Const(it) => {
                    let def = self.const_to_def(container.with_value(it), d2s_ctx)?;
                    DefWithBodyId::from(def).into()
                }
                _ => return None,
            }
        } else {
            let it = ast::Variant::cast(container.value)?;
            let def = self.enum_variant_to_def(InFile::new(container.file_id, &it), d2s_ctx)?;
            DefWithBodyId::from(def).into()
        };
        Some(cont)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) enum ChildContainer {
    DefWithBodyId(DefWithBodyId),
    ModuleId(ModuleId),
    TraitId(TraitId),
    TraitAliasId(TraitAliasId),
    ImplId(ImplId),
    EnumId(EnumId),
    VariantId(VariantId),
    TypeAliasId(TypeAliasId),
    /// XXX: this might be the same def as, for example an `EnumId`. However,
    /// here the children are generic parameters, and not, eg enum variants.
    GenericDefId(GenericDefId),
}
impl_from! {
    DefWithBodyId,
    ModuleId,
    TraitId,
    TraitAliasId,
    ImplId,
    EnumId,
    VariantId,
    TypeAliasId,
    GenericDefId
    for ChildContainer
}

impl ChildContainer {
    fn child_by_source(
        self,
        db: &dyn HirDatabase,
        d2s_ctx: &mut DefToSourceContext<'_>,
        file_id: HirFileId,
    ) -> DynMap {
        let _p = tracing::info_span!("ChildContainer::child_by_source").entered();
        let db = db.upcast();
        match self {
            ChildContainer::DefWithBodyId(it) => {
                it.child_by_source(db, &mut Some(d2s_ctx), file_id)
            }
            ChildContainer::ModuleId(it) => it.child_by_source(db, &mut Some(d2s_ctx), file_id),
            ChildContainer::TraitId(it) => it.child_by_source(db, &mut Some(d2s_ctx), file_id),
            ChildContainer::TraitAliasId(_) => DynMap::default(),
            ChildContainer::ImplId(it) => it.child_by_source(db, &mut Some(d2s_ctx), file_id),
            ChildContainer::EnumId(it) => it.child_by_source(db, &mut Some(d2s_ctx), file_id),
            ChildContainer::VariantId(it) => it.child_by_source(db, &mut Some(d2s_ctx), file_id),
            ChildContainer::TypeAliasId(_) => DynMap::default(),
            ChildContainer::GenericDefId(it) => it.child_by_source(db, &mut Some(d2s_ctx), file_id),
        }
    }
}
