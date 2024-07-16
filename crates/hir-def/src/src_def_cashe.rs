use span::HirFileId;
use syntax::SyntaxNode;

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

    fn parse_or_expand(&self, file_id: HirFileId) -> SyntaxNode;
}
