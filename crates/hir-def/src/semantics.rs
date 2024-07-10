use hir_expand::InFile;
use syntax::AstNode;

use crate::dyn_map::DynMap;

pub trait Semantics {
    fn src_to_def_dyn_map<Ast: AstNode + 'static>(&mut self, src: InFile<&Ast>) -> &mut DynMap;
    fn def_to_src_dyn_map(&mut self) -> &mut DynMap;
}
