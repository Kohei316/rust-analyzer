use crate::dyn_map::Key;

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

pub trait DefToSrcCacheContext<K, V, P, F>: SrcDefCacheContext
where
    K: Copy,
    F: FnOnce() -> V,
{
    fn get(&self, map_key: Key<K, V, P>, key: K) -> Option<V>;
    fn insert_with(&self, map_key: Key<K, V, P>, key: K, f: F) -> V;
    fn get_or_inset_with(&self, map_key: Key<K, V, P>, key: K, f: F) -> V {
        DefToSrcCacheContext::get(self, map_key, key)
            .unwrap_or_else(|| DefToSrcCacheContext::insert_with(self, map_key, key, f))
    }
}
