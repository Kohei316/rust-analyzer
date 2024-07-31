use std::{collections::hash_map::Entry, hash::Hash};

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

struct DefToSrcCache {
    dyn_map_cache: DynMap,
}

pub struct DefToSrcCacheContext<'cache> {
    cache: &'cache mut DefToSrcCache,
}

impl<'cache> DefToSrcCacheContext<'cache> {
    pub fn entry<Def, Ast>(
        &mut self,
        map_key: Key<Def, AstPtr<Ast>, DefIdPolicy<Def, Ast>>,
        key: Def,
    ) -> Entry<'_, Def, AstPtr<Ast>>
    where
        Def: Eq + Hash + 'static,
        Ast: AstNode + 'static,
    {
        self.cache.dyn_map_cache[map_key].entry(key)
    }
}
