use std::hash::Hash;

use syntax::{AstNode, AstPtr};

use crate::dyn_map::{def_to_src::DefIdPolicy, DynMap, Key};

pub trait SrcDefCacheContext {
    fn get<K, V, P>(&self, map_key: Key<K, V, P>, key: K) -> Option<V>;
    fn insert_with<K, V, P, F: FnOnce() -> V>(&self, map_key: Key<K, V, P>, key: K, f: F) -> V;
    fn get_or_inset_with<K: Copy, V, P, F: FnOnce() -> V>(
        &self,
        map_key: Key<K, V, P>,
        key: K,
        f: F,
    ) -> V {
        self.get(map_key, key).unwrap_or_else(|| self.insert_with(map_key, key, f))
    }
}

pub struct DefToSrcCacheContext<'cache> {
    cache: &'cache mut DynMap,
}

impl<'cache> DefToSrcCacheContext<'cache> {
    fn get<Def, Ast>(
        &self,
        map_key: Key<Def, AstPtr<Ast>, DefIdPolicy<Def, Ast>>,
        key: Def,
    ) -> Option<AstPtr<Ast>>
    where
        Def: Eq + Hash + 'static,
        Ast: AstNode + Copy + 'static,
    {
        self.cache[map_key].get(&key).copied()
    }

    fn insert_with<Def, Ast, F>(
        &mut self,
        map_key: Key<Def, AstPtr<Ast>, DefIdPolicy<Def, Ast>>,
        key: Def,
        f: F,
    ) -> AstPtr<Ast>
    where
        Def: Eq + Hash + 'static + Copy,
        Ast: AstNode + Copy + 'static,
        F: FnOnce(Def) -> AstPtr<Ast>,
    {
        let ast_ptr = f(key);
        self.cache[map_key].insert(key, ast_ptr);
        ast_ptr
    }

    fn get_insert_with<Def, Ast, F>(
        &mut self,
        map_key: Key<Def, AstPtr<Ast>, DefIdPolicy<Def, Ast>>,
        key: Def,
        f: F,
    ) -> AstPtr<Ast>
    where
        Def: Eq + Hash + 'static + Copy,
        Ast: AstNode + Copy + 'static,
        F: FnOnce(Def) -> AstPtr<Ast>,
    {
        self.get(map_key, key).unwrap_or_else(|| self.insert_with(map_key, key, f))
    }
}
