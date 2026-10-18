//! STEP import as a dead solid and the `combine` feature that puts it into the body.

use crate::{Cx, Res, Tool};
use tcad_diag::codes;
use tcad_expr::ExprKind;
use tcad_kernel::{BoolOp, Kernel, Transform};
use tcad_model::Item;
use tcad_naming::{Id, Names};

impl<K: Kernel> Cx<'_, K> {
    /// `import name = step("imports/x.step")`: reads the file; the solid's faces are named
    /// `name.face#n`, ordered by position. Nothing is added to the body until `combine`.
    pub(crate) fn import_step(&mut self, item: &Item, path: &str) -> Res {
        let Some(loader) = self.loader else {
            return self.fail(
                codes::E5004,
                item.span,
                format!("cannot import `{path}`: this build has no access to the bundle's files"),
            );
        };
        let bytes = match loader(path) {
            Ok(b) => b,
            Err(m) => {
                return self.fail(
                    codes::E5004,
                    item.span,
                    format!("cannot read `{path}`: {m}"),
                );
            }
        };
        let shape = match self.k.import_step(&bytes) {
            Ok(s) => s,
            Err(e) => {
                return self.fail(
                    codes::E5004,
                    item.span,
                    format!("`{path}` is not a usable STEP solid: {e}"),
                );
            }
        };
        let (topo, names) = match self.k.topology(&shape) {
            Ok(t) => {
                let faces = (0..t.faces.len())
                    .map(|_| {
                        vec![Id {
                            text: format!("{}.face", item.name),
                            rank: self.rank,
                        }]
                    })
                    .collect();
                (Some(t), Names { faces })
            }
            Err(_) => (None, Names::default()),
        };
        self.imports
            .insert(item.name.clone(), Tool { shape, topo, names });
        Ok(())
    }

    /// `combine name with=<import> [at=(x, y, z)] [op=add|subtract|intersect]`.
    pub(crate) fn combine_import(&mut self, item: &Item) -> Res {
        self.only(item, &["with", "at", "op"])?;
        let with = self.require(item, "with")?;
        let name = match &with.values[0].kind {
            ExprKind::Ref(r) => r.simple_name().map(str::to_string),
            _ => None,
        };
        let Some(name) = name.filter(|n| self.imports.contains_key(n)) else {
            return self.fail(
                codes::E5002,
                with.span,
                "`with` names an earlier `import` (that is not suppressed)",
            );
        };
        let op = match self.op_of(item)? {
            "add" => BoolOp::Union,
            "subtract" => BoolOp::Subtract,
            _ => BoolOp::Intersect,
        };
        let src = &self.imports[&name];
        let (shape, topo, names) = (src.shape.clone(), src.topo.clone(), src.names.clone());
        let tool = match Self::arg(item, "at") {
            Some(a) => {
                let v = self.triple(&a.values[0], false)?;
                let moved = match self.k.transform(&shape, &Transform::Translate(v)) {
                    Ok(s) => s,
                    Err(e) => return self.kernel_error(item, e),
                };
                // the faces keep their IDs (the transform maps every face to itself)
                let t = self.k.topology(&moved).ok();
                let lin = self.k.lineage(&moved);
                let mut faces = vec![Vec::new(); t.as_ref().map_or(0, |t| t.faces.len())];
                for (f, _, i) in &lin.inherited {
                    if let (Some(slot), Some(ids)) =
                        (faces.get_mut(*f as usize), names.faces.get(*i as usize))
                    {
                        *slot = ids.clone();
                    }
                }
                Tool {
                    shape: moved,
                    topo: t,
                    names: Names { faces },
                }
            }
            None => Tool { shape, topo, names },
        };
        let roles = self.roles_of(item, &tool);
        let copy = tool.shape.clone();
        self.merge_tool(item, op, tool)?;
        self.finish_feature(item)?;
        self.register_tool(item, op, vec![(copy, roles)]);
        Ok(())
    }
}
