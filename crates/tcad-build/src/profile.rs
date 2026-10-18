//! Profiles: turning solved sketch geometry into closed, correctly oriented loops.
//!
//! Closed entities (rectangles, circles, ellipses, polygons, slots, closed splines) are loops by
//! themselves. Lines, arcs and open splines are chained end to end. Every loop is made
//! counterclockwise; loops are then nested by containment (even depth: outer boundary, odd depth:
//! a hole of the loop that contains it).

use tcad_kernel::geometry::{Frame, Loop, P2, Path, Profile, Segment};
use tcad_model::{Geometry, SolvedEntity, SolvedSketch};

fn dist(a: P2, b: P2) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

fn arc_mid(center: P2, start: P2, end: P2) -> P2 {
    let a0 = libm::atan2(start[1] - center[1], start[0] - center[0]);
    let a1 = libm::atan2(end[1] - center[1], end[0] - center[0]);
    let mut sweep = (a1 - a0).rem_euclid(2.0 * std::f64::consts::PI);
    if sweep == 0.0 {
        sweep = 2.0 * std::f64::consts::PI;
    }
    let r = dist(start, center);
    let m = a0 + sweep / 2.0;
    [center[0] + r * libm::cos(m), center[1] + r * libm::sin(m)]
}

fn line(a: P2, b: P2) -> Segment {
    Segment::Line { a, b }
}

/// The segments of an entity and whether they form a closed loop by themselves.
fn entity_segments(e: &SolvedEntity) -> Option<(Vec<Segment>, bool)> {
    Some(match &e.geometry {
        Geometry::Rect {
            bottom_left,
            bottom_right,
            top_right,
            top_left,
        } => (
            vec![
                line(*bottom_left, *bottom_right),
                line(*bottom_right, *top_right),
                line(*top_right, *top_left),
                line(*top_left, *bottom_left),
            ],
            true,
        ),
        Geometry::Circle { center, r } => (
            vec![Segment::Circle {
                center: *center,
                r: *r,
            }],
            true,
        ),
        Geometry::Ellipse {
            center,
            a,
            b,
            theta,
        } => (
            vec![Segment::Ellipse {
                center: *center,
                a: *a,
                b: *b,
                theta: *theta,
            }],
            true,
        ),
        Geometry::Polygon { vertices, .. } => {
            let n = vertices.len();
            (
                (0..n)
                    .map(|i| line(vertices[i], vertices[(i + 1) % n]))
                    .collect(),
                true,
            )
        }
        Geometry::Slot { start, end, width } => {
            let (dx, dy) = (end[0] - start[0], end[1] - start[1]);
            let len = dx.hypot(dy);
            if len == 0.0 {
                return None;
            }
            let h = width / 2.0;
            let n = [-dy / len * h, dx / len * h];
            let (a_lo, a_hi) = (
                [start[0] - n[0], start[1] - n[1]],
                [start[0] + n[0], start[1] + n[1]],
            );
            let (b_lo, b_hi) = (
                [end[0] - n[0], end[1] - n[1]],
                [end[0] + n[0], end[1] + n[1]],
            );
            let cap_b = [end[0] + dx / len * h, end[1] + dy / len * h];
            let cap_a = [start[0] - dx / len * h, start[1] - dy / len * h];
            (
                vec![
                    line(a_lo, b_lo),
                    Segment::Arc {
                        center: *end,
                        start: b_lo,
                        mid: cap_b,
                        end: b_hi,
                    },
                    line(b_hi, a_hi),
                    Segment::Arc {
                        center: *start,
                        start: a_hi,
                        mid: cap_a,
                        end: a_lo,
                    },
                ],
                true,
            )
        }
        Geometry::Spline {
            points,
            interpolated,
            closed,
        } => (
            vec![Segment::Spline {
                points: points.clone(),
                interpolated: *interpolated,
                closed: *closed,
            }],
            *closed,
        ),
        Geometry::Line { start, end } => (vec![line(*start, *end)], false),
        Geometry::Arc {
            center, start, end, ..
        } => (
            vec![Segment::Arc {
                center: *center,
                start: *start,
                mid: arc_mid(*center, *start, *end),
                end: *end,
            }],
            false,
        ),
        Geometry::Point(_) | Geometry::Projected => return None,
    })
}

fn reverse_segment(s: &Segment) -> Segment {
    match s {
        Segment::Line { a, b } => Segment::Line { a: *b, b: *a },
        Segment::Arc {
            center,
            start,
            mid,
            end,
        } => Segment::Arc {
            center: *center,
            start: *end,
            mid: *mid,
            end: *start,
        },
        Segment::Spline {
            points,
            interpolated,
            closed,
        } => Segment::Spline {
            points: points.iter().rev().copied().collect(),
            interpolated: *interpolated,
            closed: *closed,
        },
        // a full circle or ellipse is symmetric for our purposes: orientation is fixed by the
        // normal of the plane, and loops made of them alone are always counterclockwise
        other => other.clone(),
    }
}

fn reverse(segs: &[Segment]) -> Vec<Segment> {
    segs.iter().rev().map(reverse_segment).collect()
}

/// Chains open segments into loops. Returns the loops or a message naming a gap.
fn chain(mut pool: Vec<Segment>, join: f64) -> Result<Vec<Vec<Segment>>, String> {
    let mut loops = Vec::new();
    while let Some(first) = pool.pop() {
        let mut chain = vec![first];
        let start = chain[0].start();
        loop {
            let end = chain.last().map(Segment::end).unwrap_or(start);
            if (chain.len() > 1 || chain[0].is_closed()) && dist(end, start) <= join {
                break;
            }
            let pick = pool
                .iter()
                .position(|s| dist(s.start(), end) <= join)
                .map(|i| (i, false))
                .or_else(|| {
                    pool.iter()
                        .position(|s| dist(s.end(), end) <= join)
                        .map(|i| (i, true))
                });
            match pick {
                Some((i, flip)) => {
                    let s = pool.remove(i);
                    chain.push(if flip { reverse_segment(&s) } else { s });
                }
                None => {
                    return Err(format!(
                        "the profile is not closed: no segment continues at ({:.6}, {:.6}) mm",
                        end[0], end[1]
                    ));
                }
            }
        }
        loops.push(chain);
    }
    Ok(loops)
}

fn point_in_polygon(p: P2, poly: &[P2]) -> bool {
    let mut inside = false;
    let n = poly.len();
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (poly[i], poly[j]);
        if (a[1] > p[1]) != (b[1] > p[1])
            && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0]
        {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Loops from the entities of a sketch. With `only` set, just that entity (it must be closed or be
/// part of a closed chain by itself); otherwise every non-construction entity.
pub fn loops_of(sketch: &SolvedSketch, only: Option<&str>, join: f64) -> Result<Vec<Loop>, String> {
    let mut closed: Vec<Vec<Segment>> = Vec::new();
    let mut open: Vec<Segment> = Vec::new();
    let mut found = false;
    for e in &sketch.entities {
        if only.is_some_and(|n| n != e.name) || (only.is_none() && e.construction) {
            continue;
        }
        found = true;
        if let Some((segs, is_closed)) = entity_segments(e) {
            if is_closed {
                closed.push(segs);
            } else {
                open.extend(segs);
            }
        }
    }
    if !found {
        return Err(match only {
            Some(n) => format!("sketch `{}` has no entity `{n}`", sketch.name),
            None => format!("sketch `{}` has no geometry", sketch.name),
        });
    }
    closed.extend(chain(open, join)?);
    if closed.is_empty() {
        return Err(format!("sketch `{}` has no closed profile", sketch.name));
    }
    Ok(closed
        .into_iter()
        .map(|mut segs| {
            let l = Loop {
                segments: segs.clone(),
            };
            if l.signed_area() < 0.0 {
                segs = reverse(&segs);
            }
            Loop { segments: segs }
        })
        .collect())
}

/// Nests loops by containment into profiles in `frame`.
pub fn nest(frame: Frame, loops: Vec<Loop>) -> Vec<Profile> {
    let polys: Vec<Vec<P2>> = loops.iter().map(Loop::polygon).collect();
    let n = loops.len();
    // containers[i]: loops that contain loop i
    let containers: Vec<Vec<usize>> = (0..n)
        .map(|i| {
            let probe = polys[i][0];
            (0..n)
                .filter(|&j| j != i && point_in_polygon(probe, &polys[j]))
                .collect()
        })
        .collect();
    let mut profiles: Vec<(usize, Profile)> = Vec::new();
    for i in 0..n {
        if containers[i].len().is_multiple_of(2) {
            profiles.push((
                i,
                Profile {
                    frame,
                    outer: loops[i].clone(),
                    holes: Vec::new(),
                },
            ));
        }
    }
    for i in 0..n {
        if containers[i].len() % 2 == 1 {
            // the hole belongs to its nearest (deepest) container
            let parent = containers[i]
                .iter()
                .copied()
                .max_by_key(|&j| containers[j].len());
            if let Some(parent) = parent
                && let Some((_, p)) = profiles.iter_mut().find(|(k, _)| *k == parent)
            {
                p.holes.push(loops[i].clone());
            }
        }
    }
    profiles.into_iter().map(|(_, p)| p).collect()
}

/// A path (an open or closed chain) from the open entities of a sketch, in drawing order as far
/// as the connectivity allows.
pub fn path_of(
    sketch: &SolvedSketch,
    frame: Frame,
    only: Option<&str>,
    join: f64,
) -> Result<Path, String> {
    let mut pool: Vec<Segment> = Vec::new();
    for e in &sketch.entities {
        if only.is_some_and(|n| n != e.name) || (only.is_none() && e.construction) {
            continue;
        }
        if let Some((segs, _)) = entity_segments(e) {
            pool.extend(segs);
        }
    }
    if pool.is_empty() {
        return Err(format!("sketch `{}` has no path geometry", sketch.name));
    }
    // start from the segment with a free end so an open chain begins at its end
    let ends: Vec<P2> = pool.iter().flat_map(|s| [s.start(), s.end()]).collect();
    let free_start = pool.iter().position(|s| {
        let free = |p: P2| ends.iter().filter(|q| dist(**q, p) <= join).count() == 1;
        free(s.start())
    });
    let mut segs: Vec<Segment> = Vec::new();
    let mut cur = match free_start {
        Some(i) => pool.remove(i),
        None => pool.remove(0),
    };
    loop {
        let end = cur.end();
        segs.push(cur);
        let next = pool
            .iter()
            .position(|s| dist(s.start(), end) <= join)
            .map(|i| (i, false))
            .or_else(|| {
                pool.iter()
                    .position(|s| dist(s.end(), end) <= join)
                    .map(|i| (i, true))
            });
        match next {
            Some((i, flip)) => {
                let s = pool.remove(i);
                cur = if flip { reverse_segment(&s) } else { s };
            }
            None => break,
        }
    }
    if !pool.is_empty() {
        return Err(format!(
            "the path of sketch `{}` is not a single chain",
            sketch.name
        ));
    }
    Ok(Path {
        frame,
        segments: segs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tcad_model::analyze;

    fn solved(body: &str) -> SolvedSketch {
        let src = format!("tcad 0.1\n\npart p {{\n  sketch s on plane.XY {{\n{body}\n  }}\n}}\n");
        let a = analyze(&src);
        assert!(!a.has_errors(), "{:#?}", a.diagnostics);
        a.model
            .expect("model")
            .solve_sketches(None)
            .expect("solve")
            .remove(0)
    }

    #[test]
    fn closed_entities_become_loops() {
        let s = solved(
            "    rect r corner=(0 mm, 0 mm) w=10 mm h=5 mm\n    circle c center=(20 mm, 0 mm) r=2 mm",
        );
        let loops = loops_of(&s, None, 1e-6).unwrap();
        assert_eq!(loops.len(), 2);
        assert!(loops.iter().all(|l| l.signed_area() > 0.0));
        assert_eq!(loops_of(&s, Some("c"), 1e-6).unwrap().len(), 1);
        assert!(loops_of(&s, Some("nope"), 1e-6).is_err());
    }

    #[test]
    fn lines_and_arcs_chain_into_one_loop_in_any_direction() {
        // a "D": a line and a semicircle, drawn with opposite directions
        let s = solved(
            "    line l start=(0 mm, 5 mm) end=(0 mm, -5 mm)\n    \
             arc a start=(0 mm, 5 mm) end=(0 mm, -5 mm) through=(5 mm, 0 mm)",
        );
        let loops = loops_of(&s, None, 1e-6).unwrap();
        assert_eq!(loops.len(), 1);
        assert_eq!(loops[0].segments.len(), 2);
        assert!(loops[0].signed_area() > 0.0);
        let area = loops[0].signed_area();
        assert!(
            (area - std::f64::consts::PI * 25.0 / 2.0).abs() < 0.5,
            "{area}"
        );
    }

    #[test]
    fn an_open_chain_reports_the_gap() {
        let s = solved(
            "    line a start=(0 mm, 0 mm) end=(10 mm, 0 mm)\n    line b start=(10 mm, 0 mm) end=(10 mm, 10 mm)",
        );
        let err = loops_of(&s, None, 1e-6).unwrap_err();
        assert!(err.contains("not closed"), "{err}");
        assert!(err.contains("10.000000"), "{err}");
    }

    #[test]
    fn nesting_makes_holes_and_islands() {
        let s = solved(
            "    rect outer corner=(0 mm, 0 mm) w=40 mm h=40 mm\n    rect hole corner=(5 mm, 5 mm) w=30 mm h=30 mm\n    \
             circle island center=(20 mm, 20 mm) r=3 mm\n    circle apart center=(60 mm, 20 mm) r=3 mm",
        );
        let loops = loops_of(&s, None, 1e-6).unwrap();
        let profiles = nest(Frame::xy(), loops);
        // outer with the big hole, the island inside the hole, and the separate circle
        assert_eq!(profiles.len(), 3);
        let with_hole = profiles
            .iter()
            .find(|p| !p.holes.is_empty())
            .expect("outer");
        assert_eq!(with_hole.holes.len(), 1);
    }

    #[test]
    fn slot_profile_has_two_arcs() {
        let s = solved("    slot sl start=(0 mm, 0 mm) end=(10 mm, 0 mm) w=4 mm");
        let loops = loops_of(&s, None, 1e-6).unwrap();
        assert_eq!(loops[0].segments.len(), 4);
        let area = loops[0].signed_area();
        let exact = 10.0 * 4.0 + std::f64::consts::PI * 4.0;
        assert!((area - exact).abs() < 0.5, "{area} vs {exact}");
    }

    #[test]
    fn path_follows_the_chain_from_its_free_end() {
        let s = solved(
            "    line b start=(10 mm, 0 mm) end=(10 mm, 10 mm)\n    line a start=(0 mm, 0 mm) end=(10 mm, 0 mm)",
        );
        let p = path_of(&s, Frame::xy(), None, 1e-6).unwrap();
        assert_eq!(p.segments.len(), 2);
        assert_eq!(p.segments[0].start(), [0.0, 0.0]);
        assert_eq!(p.segments[1].end(), [10.0, 10.0]);
    }
}
