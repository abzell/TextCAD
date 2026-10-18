//! Patterns: linear, circular and mirror copies of an earlier feature's tool, including patterns of
//! patterns and instance skip lists.

use crate::{Cx, Res, Tool};
use tcad_diag::codes;
use tcad_expr::{Dim, ExprKind, Value};
use tcad_kernel::geometry::{V3, norm, scale, unit};
use tcad_kernel::{Kernel, Transform};
use tcad_model::Item;
use tcad_naming::{Id, Names};

impl<K: Kernel> Cx<'_, K> {
    /// The feature a pattern copies.
    fn pattern_source(&mut self, item: &Item) -> Res<String> {
        let a = self.require(item, "of")?;
        let name = match &a.values[0].kind {
            ExprKind::Ref(r) => r.simple_name().map(str::to_string),
            _ => None,
        };
        match name {
            Some(n) if self.tools.contains_key(&n) => Ok(n),
            Some(n) => self.fail(
                codes::E5002,
                a.span,
                format!("`{n}` is not an earlier feature that adds or removes material (or it is suppressed)"),
            ),
            None => self.fail(codes::E5002, a.span, "`of` names an earlier feature"),
        }
    }

    /// Instance numbers to leave out (1 is the original and cannot be skipped).
    fn skip_list(&mut self, item: &Item, count: u32) -> Res<Vec<u32>> {
        let Some(a) = Self::arg(item, "skip") else {
            return Ok(Vec::new());
        };
        let mut out = Vec::new();
        for e in Self::flat(a) {
            match self.value(e)? {
                Value::Num(q) if q.dim.is_none() && q.value.fract() == 0.0 && q.value >= 1.0 => {
                    let k = q.value as u32;
                    if k == 1 {
                        return self.fail(
                            codes::E5001,
                            e.span,
                            "instance 1 is the original feature and cannot be skipped",
                        );
                    }
                    if k > count {
                        return self.fail(
                            codes::E5001,
                            e.span,
                            format!("there is no instance {k}; the pattern has {count}"),
                        );
                    }
                    out.push(k);
                }
                _ => {
                    return self.fail(
                        codes::E5001,
                        e.span,
                        "`skip` lists instance numbers like [3, 5]",
                    );
                }
            }
        }
        Ok(out)
    }

    /// Copies every piece of `of` once for each `(instance label, instance number, transform)` and
    /// combines the copies with the body using the source's operation.
    fn apply_pattern(&mut self, item: &Item, of: &str, instances: Vec<(String, Transform)>) -> Res {
        let (op, pieces) = {
            let rec = &self.tools[of];
            (
                rec.op,
                rec.pieces
                    .iter()
                    .map(|p| (p.shape.clone(), p.roles.clone()))
                    .collect::<Vec<_>>(),
            )
        };
        let mut registry: Vec<(K::Shape, Vec<String>)> = pieces.clone();
        for (label, tf) in &instances {
            for (shape, roles) in &pieces {
                let moved = match self.k.transform(shape, tf) {
                    Ok(s) => s,
                    Err(e) => return self.kernel_error(item, e),
                };
                let topo = self.k.topology(&moved).ok();
                let lin = self.k.lineage(&moved);
                let mut copy_roles =
                    vec![String::new(); topo.as_ref().map_or(0, |t| t.faces.len())];
                for (f, _, i) in &lin.inherited {
                    if let (Some(slot), Some(r)) =
                        (copy_roles.get_mut(*f as usize), roles.get(*i as usize))
                    {
                        *slot = format!("{label}{r}");
                    }
                }
                let names = Names {
                    faces: copy_roles
                        .iter()
                        .map(|r| {
                            vec![Id {
                                text: format!(
                                    "{}.{}",
                                    item.name,
                                    if r.is_empty() { "extra" } else { r }
                                ),
                                rank: self.rank,
                            }]
                        })
                        .collect(),
                };
                let tool = Tool {
                    shape: moved.clone(),
                    topo,
                    names,
                };
                self.merge_tool(item, op, tool)?;
                registry.push((moved, copy_roles));
            }
        }
        self.finish_feature(item)?;
        self.register_tool(item, op, registry);
        Ok(())
    }

    pub(crate) fn linear_pattern(&mut self, item: &Item) -> Res {
        self.only(item, &["of", "dir", "count", "spacing", "skip"])?;
        let of = self.pattern_source(item)?;
        let count = {
            let a = self.require(item, "count")?;
            self.count_of(a, "`count`")?
        };
        let spacing = {
            let a = self.require(item, "spacing")?;
            self.quantity(a, Dim::LENGTH, "the spacing")?
        };
        let dir = {
            let a = self.require(item, "dir")?;
            self.direction_of(&a.values[0])?
        };
        let skip = self.skip_list(item, count)?;
        let instances = (2..=count)
            .filter(|k| !skip.contains(k))
            .map(|k| {
                (
                    format!("i{k}."),
                    Transform::Translate(scale(unit(dir), spacing * f64::from(k - 1))),
                )
            })
            .collect();
        self.apply_pattern(item, &of, instances)
    }

    pub(crate) fn circular_pattern(&mut self, item: &Item) -> Res {
        self.only(item, &["of", "axis", "count", "angle", "skip"])?;
        let of = self.pattern_source(item)?;
        let count = {
            let a = self.require(item, "count")?;
            self.count_of(a, "`count`")?
        };
        let total = match Self::arg(item, "angle") {
            Some(a) => self.quantity(a, Dim::ANGLE, "the angle")?,
            None => 2.0 * std::f64::consts::PI,
        };
        let (origin, dir): (V3, V3) = {
            let a = self.require(item, "axis")?;
            self.axis_expr(&a.values[0])?
        };
        let full = (total - 2.0 * std::f64::consts::PI).abs() < 1e-9;
        let step = if count < 2 {
            0.0
        } else if full {
            total / f64::from(count)
        } else {
            total / f64::from(count - 1)
        };
        if norm(dir) == 0.0 {
            return self.fail(codes::E5001, item.span, "the pattern axis has no direction");
        }
        let skip = self.skip_list(item, count)?;
        let instances = (2..=count)
            .filter(|k| !skip.contains(k))
            .map(|k| {
                (
                    format!("i{k}."),
                    Transform::Rotate {
                        origin,
                        dir: unit(dir),
                        angle: step * f64::from(k - 1),
                    },
                )
            })
            .collect();
        self.apply_pattern(item, &of, instances)
    }

    pub(crate) fn mirror(&mut self, item: &Item) -> Res {
        self.only(item, &["of", "plane"])?;
        let of = self.pattern_source(item)?;
        let plane = {
            let a = self.require(item, "plane")?;
            self.plane_expr(&a.values[0])?
        };
        self.apply_pattern(
            item,
            &of,
            vec![(
                String::new(),
                Transform::Mirror {
                    origin: plane.origin,
                    normal: plane.z,
                },
            )],
        )
    }
}
