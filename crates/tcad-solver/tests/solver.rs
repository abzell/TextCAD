//! Tests for the sketch constraint solver.

use std::f64::consts::PI;
use tcad_solver::{Constraint as C, Curve, Point, Problem};

fn at(sol: &tcad_solver::Solution, p: Point) -> (f64, f64) {
    (sol.x[p.x], sol.x[p.y])
}

fn dist(a: (f64, f64), b: (f64, f64)) -> f64 {
    (a.0 - b.0).hypot(a.1 - b.1)
}

fn near(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-7, "{a} != {b}");
}

#[test]
fn points_fix_coincident_distance() {
    let mut p = Problem::new();
    let a = p.point(0.3, -0.2);
    let b = p.point(7.0, 1.0);
    let c = p.point(9.0, 9.0);
    p.constrain(C::Fix(a, 1.0, 2.0), 0).unwrap();
    p.constrain(C::Distance(a, b, 5.0), 1).unwrap();
    p.constrain(C::Horizontal(a, b), 2).unwrap();
    p.constrain(C::Coincident(c, b), 3).unwrap();
    let s = p.solve();
    assert!(s.ok());
    near(at(&s, a).0, 1.0);
    near(at(&s, a).1, 2.0);
    near(at(&s, b).0, 6.0);
    near(at(&s, b).1, 2.0);
    assert_eq!(at(&s, c), at(&s, b));
}

#[test]
fn horizontal_vertical_and_offsets() {
    let mut p = Problem::new();
    let a = p.point(0.0, 0.0);
    let b = p.point(5.0, 4.0);
    p.constrain(C::Fix(a, 0.0, 0.0), 0).unwrap();
    p.constrain(C::HDistance(a, b, 30.0), 1).unwrap();
    p.constrain(C::VDistance(a, b, -12.0), 2).unwrap();
    let s = p.solve();
    assert!(s.ok());
    near(at(&s, b).0, 30.0);
    near(at(&s, b).1, -12.0);
}

#[test]
fn parallel_perpendicular_angle() {
    let mut p = Problem::new();
    let (a, b, c, d) = (
        p.point(0.0, 0.0),
        p.point(10.0, 1.0),
        p.point(0.0, 5.0),
        p.point(10.0, 7.0),
    );
    let (l, m) = (p.line(a, b), p.line(c, d));
    p.constrain(C::Fix(a, 0.0, 0.0), 0).unwrap();
    p.constrain(C::Fix(b, 10.0, 0.0), 1).unwrap();
    p.constrain(C::Fix(c, 0.0, 5.0), 2).unwrap();
    p.constrain(C::Parallel(l, m), 3).unwrap();
    p.constrain(C::Distance(c, d, 10.0), 4).unwrap();
    let s = p.solve();
    assert!(s.ok());
    near(at(&s, d).1, 5.0);
    near(at(&s, d).0.abs(), 10.0);

    let mut q = Problem::new();
    let (a, b, c, d) = (
        q.point(0.0, 0.0),
        q.point(10.0, 0.0),
        q.point(5.0, 1.0),
        q.point(6.0, 9.0),
    );
    let (l, m) = (q.line(a, b), q.line(c, d));
    q.constrain(C::Fix(a, 0.0, 0.0), 0).unwrap();
    q.constrain(C::Fix(b, 10.0, 0.0), 1).unwrap();
    q.constrain(C::Fix(c, 5.0, 1.0), 2).unwrap();
    q.constrain(C::Perpendicular(l, m), 3).unwrap();
    q.constrain(C::Distance(c, d, 8.0), 4).unwrap();
    let s = q.solve();
    assert!(s.ok());
    near(at(&s, d).0, 5.0);
    near(at(&s, d).1, 9.0);

    let mut r = Problem::new();
    let (a, b, c, d) = (
        r.point(0.0, 0.0),
        r.point(10.0, 0.0),
        r.point(0.0, 0.0),
        r.point(5.0, 5.0),
    );
    let (l, m) = (r.line(a, b), r.line(c, d));
    r.constrain(C::Fix(a, 0.0, 0.0), 0).unwrap();
    r.constrain(C::Fix(b, 10.0, 0.0), 1).unwrap();
    r.constrain(C::Fix(c, 0.0, 0.0), 2).unwrap();
    r.constrain(C::Angle(l, m, 30f64.to_radians()), 3).unwrap();
    r.constrain(C::Distance(c, d, 4.0), 4).unwrap();
    let s = r.solve();
    assert!(s.ok());
    near(at(&s, d).0, 4.0 * 30f64.to_radians().cos());
    near(at(&s, d).1, 4.0 * 30f64.to_radians().sin());
}

#[test]
fn equal_concentric_radius_diameter() {
    let mut p = Problem::new();
    let (a, b, c, d) = (
        p.point(0.0, 0.0),
        p.point(10.0, 0.0),
        p.point(0.0, 3.0),
        p.point(7.0, 3.0),
    );
    let (l, m) = (p.line(a, b), p.line(c, d));
    let (c1, c2) = (p.point(0.0, 0.0), p.point(1.0, 1.0));
    let (k1, k2) = (p.circle(c1, 2.0), p.circle(c2, 3.0));
    p.constrain(C::Fix(a, 0.0, 0.0), 0).unwrap();
    p.constrain(C::Fix(b, 10.0, 0.0), 1).unwrap();
    p.constrain(C::Fix(c, 0.0, 3.0), 2).unwrap();
    p.constrain(C::Horizontal(c, d), 3).unwrap();
    p.constrain(C::EqualLength(l, m), 4).unwrap();
    p.constrain(C::Fix(c1, 2.0, 2.0), 5).unwrap();
    p.constrain(C::Concentric(Curve::Circle(k1), Curve::Circle(k2)), 6)
        .unwrap();
    p.constrain(C::Radius(Curve::Circle(k1), 4.0), 7).unwrap();
    p.constrain(C::EqualRadius(Curve::Circle(k1), Curve::Circle(k2)), 8)
        .unwrap();
    let s = p.solve();
    assert!(s.ok());
    near(at(&s, d).0, 10.0);
    near(at(&s, c2).0, 2.0);
    near(at(&s, c2).1, 2.0);
    near(s.x[k2.r], 4.0);
}

#[test]
fn tangents() {
    // line tangent to a circle
    let mut p = Problem::new();
    let (a, b) = (p.point(-5.0, 2.0), p.point(5.0, 2.5));
    let l = p.line(a, b);
    let c = p.point(0.0, 0.0);
    let k = p.circle(c, 1.5);
    p.constrain(C::Fix(c, 0.0, 0.0), 0).unwrap();
    p.constrain(C::Radius(Curve::Circle(k), 1.5), 1).unwrap();
    p.constrain(C::Horizontal(a, b), 2).unwrap();
    p.constrain(C::Tangent(Curve::Line(l), Curve::Circle(k)), 3)
        .unwrap();
    let s = p.solve();
    assert!(s.ok());
    near(at(&s, a).1, 1.5); // stays on the side it started on

    // externally tangent circles
    let mut p = Problem::new();
    let (c1, c2) = (p.point(0.0, 0.0), p.point(5.0, 0.0));
    let (k1, k2) = (p.circle(c1, 2.0), p.circle(c2, 1.0));
    p.constrain(C::Fix(c1, 0.0, 0.0), 0).unwrap();
    p.constrain(C::Radius(Curve::Circle(k1), 2.0), 1).unwrap();
    p.constrain(C::Radius(Curve::Circle(k2), 1.0), 2).unwrap();
    p.constrain(C::Horizontal(c1, c2), 3).unwrap();
    p.constrain(C::Tangent(Curve::Circle(k1), Curve::Circle(k2)), 4)
        .unwrap();
    let s = p.solve();
    assert!(s.ok());
    near(at(&s, c2).0, 3.0);

    // internally tangent: the small circle inside the big one stays inside
    let mut p = Problem::new();
    let (c1, c2) = (p.point(0.0, 0.0), p.point(1.0, 0.0));
    let (k1, k2) = (p.circle(c1, 5.0), p.circle(c2, 2.0));
    p.constrain(C::Fix(c1, 0.0, 0.0), 0).unwrap();
    p.constrain(C::Radius(Curve::Circle(k1), 5.0), 1).unwrap();
    p.constrain(C::Radius(Curve::Circle(k2), 2.0), 2).unwrap();
    p.constrain(C::Horizontal(c1, c2), 3).unwrap();
    p.constrain(C::Tangent(Curve::Circle(k1), Curve::Circle(k2)), 4)
        .unwrap();
    let s = p.solve();
    assert!(s.ok());
    near(at(&s, c2).0, 3.0);
}

#[test]
fn midpoint_symmetric_on_curve() {
    let mut p = Problem::new();
    let (a, b, m) = (p.point(0.0, 0.0), p.point(10.0, 4.0), p.point(4.0, 1.0));
    let l = p.line(a, b);
    p.constrain(C::Fix(a, 0.0, 0.0), 0).unwrap();
    p.constrain(C::Fix(b, 10.0, 4.0), 1).unwrap();
    p.constrain(C::Midpoint(m, l), 2).unwrap();
    let s = p.solve();
    assert!(s.ok());
    near(at(&s, m).0, 5.0);
    near(at(&s, m).1, 2.0);

    // mirror image about the y axis
    let mut p = Problem::new();
    let (a, b) = (p.point(0.0, -5.0), p.point(0.0, 5.0));
    let axis = p.line(a, b);
    let (q, r) = (p.point(3.0, 2.0), p.point(-1.0, 1.0));
    p.constrain(C::Fix(a, 0.0, -5.0), 0).unwrap();
    p.constrain(C::Fix(b, 0.0, 5.0), 1).unwrap();
    p.constrain(C::Fix(q, 3.0, 2.0), 2).unwrap();
    p.constrain(C::Symmetric(q, r, axis), 3).unwrap();
    let s = p.solve();
    assert!(s.ok());
    near(at(&s, r).0, -3.0);
    near(at(&s, r).1, 2.0);

    // point on circle, on line, on ellipse
    let mut p = Problem::new();
    let c = p.point(0.0, 0.0);
    let k = p.circle(c, 5.0);
    let q = p.point(4.0, 4.0);
    p.constrain(C::Fix(c, 0.0, 0.0), 0).unwrap();
    p.constrain(C::Radius(Curve::Circle(k), 5.0), 1).unwrap();
    p.constrain(C::PointOnCurve(q, Curve::Circle(k)), 2)
        .unwrap();
    p.constrain(C::HDistance(c, q, 3.0), 3).unwrap();
    let s = p.solve();
    assert!(s.ok());
    near(at(&s, q).1, 4.0);

    let mut p = Problem::new();
    let c = p.point(0.0, 0.0);
    let e = p.ellipse(c, 4.0, 2.0, 0.0);
    let q = p.point(0.0, 3.0);
    p.constrain(C::Fix(c, 0.0, 0.0), 0).unwrap();
    p.constrain(C::PointOnCurve(q, Curve::Ellipse(e)), 1)
        .unwrap();
    p.constrain(C::HDistance(c, q, 2.0), 2).unwrap();
    p.constrain(C::EllipseAxes(e, 4.0, 2.0), 3).unwrap();
    p.constrain(C::EllipseAngle(e, 0.0), 4).unwrap();
    let s = p.solve();
    assert!(s.ok());
    let (x, y) = at(&s, q);
    near((x / 4.0).powi(2) + (y / 2.0).powi(2), 1.0);
}

#[test]
fn rectangle_polygon_and_arc_structure() {
    let mut p = Problem::new();
    let r = p.rect(0.5, 0.5, 9.0, 6.0);
    p.constrain(C::Fix(r.bl, 0.0, 0.0), 0).unwrap();
    p.constrain(C::HDistance(r.bl, r.br, 40.0), 1).unwrap();
    p.constrain(C::VDistance(r.bl, r.tl, 25.0), 2).unwrap();
    let s = p.solve();
    assert!(s.ok());
    near(at(&s, r.tr).0, 40.0);
    near(at(&s, r.tr).1, 25.0);
    assert!(p.analyze(&s).fully_constrained());

    let mut p = Problem::new();
    let poly = p.polygon(6, 0.0, 0.0, 10.0, 0.1);
    p.constrain(C::Fix(poly.c, 1.0, 1.0), 0).unwrap();
    p.constrain(
        C::Radius(
            Curve::Circle(tcad_solver::Circle {
                c: poly.c,
                r: poly.r,
            }),
            12.0,
        ),
        1,
    )
    .unwrap();
    p.constrain(C::Horizontal(poly.verts[0], poly.c), 2)
        .unwrap();
    let s = p.solve();
    assert!(s.ok());
    let sides: Vec<f64> = (0..6)
        .map(|i| dist(at(&s, poly.verts[i]), at(&s, poly.verts[(i + 1) % 6])))
        .collect();
    sides.iter().for_each(|&d| near(d, 12.0)); // a hexagon's side equals its circumradius
    assert!(p.analyze(&s).fully_constrained());

    let mut p = Problem::new();
    let (c, st, en) = (p.point(0.0, 0.0), p.point(5.0, 0.2), p.point(-1.0, 4.0));
    let arc = p.arc(c, st, en);
    p.constrain(C::Fix(c, 0.0, 0.0), 0).unwrap();
    p.constrain(C::Radius(Curve::Arc(arc), 6.0), 1).unwrap();
    let s = p.solve();
    assert!(s.ok());
    near(dist(at(&s, st), at(&s, c)), 6.0);
    near(dist(at(&s, en), at(&s, c)), 6.0);
}

#[test]
fn degrees_of_freedom_per_entity() {
    let mut p = Problem::new();
    let pt = p.point(1.0, 1.0);
    let a = p.point(0.0, 0.0);
    let b = p.point(4.0, 1.0);
    let c = p.point(0.0, 5.0);
    let k = p.circle(c, 2.0);
    p.entity("free point", vec![pt.x, pt.y]);
    p.entity("line", vec![a.x, a.y, b.x, b.y]);
    p.entity("circle", vec![c.x, c.y, k.r]);
    p.constrain(C::Horizontal(a, b), 0).unwrap();
    p.constrain(C::Fix(c, 0.0, 5.0), 1).unwrap();
    let s = p.solve();
    let an = p.analyze(&s);
    assert_eq!(an.entity_dof, [2, 3, 1]);
    assert_eq!(an.dof, 6);
    assert!(!an.fully_constrained());
}

#[test]
fn conflicting_constraints_are_named() {
    let mut p = Problem::new();
    let a = p.point(0.0, 0.0);
    let b = p.point(10.0, 0.0);
    let c = p.point(50.0, 50.0);
    let d = p.point(60.0, 50.0);
    p.constrain(C::Fix(a, 0.0, 0.0), 10).unwrap();
    p.constrain(C::Distance(a, b, 10.0), 11).unwrap();
    p.constrain(C::Distance(a, b, 20.0), 12).unwrap(); // conflicts with 11
    p.constrain(C::Horizontal(a, b), 13).unwrap(); // innocent bystander in the same cluster
    p.constrain(C::Fix(c, 50.0, 50.0), 14).unwrap(); // other cluster
    p.constrain(C::Distance(c, d, 3.0), 15).unwrap();
    let s = p.solve();
    assert!(!s.ok());
    assert_eq!(s.conflicts.len(), 1);
    assert_eq!(s.conflicts[0].tags, [11, 12]);
    // the other cluster is still solved
    near(dist(at(&s, c), at(&s, d)), 3.0);
}

#[test]
fn redundant_constraints_are_reported() {
    let mut p = Problem::new();
    let a = p.point(0.0, 0.0);
    let b = p.point(10.0, 0.0);
    p.constrain(C::Fix(a, 0.0, 0.0), 0).unwrap();
    p.constrain(C::Horizontal(a, b), 1).unwrap();
    p.constrain(C::Horizontal(a, b), 2).unwrap(); // same thing again
    p.constrain(C::Distance(a, b, 10.0), 3).unwrap();
    let s = p.solve();
    assert!(s.ok());
    assert_eq!(p.analyze(&s).redundant, [2]);
}

#[test]
fn solutions_stay_near_the_starting_position() {
    // The apex of a triangle with two given side lengths has a mirror solution. Starting above
    // the base must stay above, and an under-determined move is the shortest one.
    let mut p = Problem::new();
    let (a, b, c) = (p.point(0.0, 0.0), p.point(10.0, 0.0), p.point(5.0, 3.0));
    p.constrain(C::Fix(a, 0.0, 0.0), 0).unwrap();
    p.constrain(C::Fix(b, 10.0, 0.0), 1).unwrap();
    p.constrain(C::Distance(a, c, 8.0), 2).unwrap();
    p.constrain(C::Distance(b, c, 7.0), 3).unwrap();
    let s = p.solve();
    assert!(s.ok());
    assert!(at(&s, c).1 > 0.0);

    let mut p = Problem::new();
    let o = p.point(0.0, 0.0);
    let q = p.point(6.0, 6.0);
    p.constrain(C::Fix(o, 0.0, 0.0), 0).unwrap();
    p.constrain(C::Distance(o, q, 10.0), 1).unwrap();
    let s = p.solve();
    near(at(&s, q).0, 10.0 / 2f64.sqrt());
    near(at(&s, q).1, 10.0 / 2f64.sqrt());
}

#[test]
fn sweeping_a_dimension_moves_the_solution_continuously() {
    // two circles kept tangent while the second radius grows; each solve starts from the last
    let mut prev = (3.0, 0.0);
    for step in 0..40 {
        let r2 = 1.0 + 0.2 * f64::from(step);
        let mut p = Problem::new();
        let (c1, c2) = (p.point(0.0, 0.0), p.point(prev.0, prev.1));
        let (k1, k2) = (p.circle(c1, 2.0), p.circle(c2, r2));
        p.constrain(C::Fix(c1, 0.0, 0.0), 0).unwrap();
        p.constrain(C::Radius(Curve::Circle(k1), 2.0), 1).unwrap();
        p.constrain(C::Radius(Curve::Circle(k2), r2), 2).unwrap();
        p.constrain(C::Horizontal(c1, c2), 3).unwrap();
        p.constrain(C::Tangent(Curve::Circle(k1), Curve::Circle(k2)), 4)
            .unwrap();
        let s = p.solve();
        assert!(s.ok(), "step {step}");
        let now = at(&s, c2);
        near(now.0, 2.0 + r2);
        assert!(dist(now, prev) < 0.5, "jumped at step {step}");
        prev = now;
    }
}

#[test]
fn solving_is_deterministic() {
    let build = || {
        let mut p = Problem::new();
        let r = p.rect(0.1, 0.2, 9.7, 5.3);
        let c = p.point(3.0, 3.0);
        let k = p.circle(c, 1.0);
        p.constrain(C::Fix(r.bl, 0.0, 0.0), 0).unwrap();
        p.constrain(C::HDistance(r.bl, r.br, 40.0), 1).unwrap();
        p.constrain(C::VDistance(r.bl, r.tl, 25.0), 2).unwrap();
        p.constrain(C::Radius(Curve::Circle(k), 4.0), 3).unwrap();
        let bottom = p.line(r.bl, r.br);
        p.constrain(C::Tangent(Curve::Line(bottom), Curve::Circle(k)), 4)
            .unwrap();
        p
    };
    let (a, b) = (build().solve(), build().solve());
    assert!(
        a.x.iter()
            .zip(&b.x)
            .all(|(x, y)| x.to_bits() == y.to_bits())
    );
}

#[test]
fn unsupported_combinations_are_errors() {
    let mut p = Problem::new();
    let (a, b, c, d) = (
        p.point(0.0, 0.0),
        p.point(1.0, 0.0),
        p.point(0.0, 1.0),
        p.point(1.0, 1.0),
    );
    let (l, m) = (p.line(a, b), p.line(c, d));
    assert!(
        p.constrain(C::Tangent(Curve::Line(l), Curve::Line(m)), 0)
            .is_err()
    );
    assert!(p.constrain(C::Radius(Curve::Line(l), 1.0), 1).is_err());
    assert!(
        p.constrain(C::Concentric(Curve::Line(l), Curve::Line(m)), 2)
            .is_err()
    );
    let _ = PI;
}

#[test]
fn a_connected_sketch_of_200_entities_solves_quickly() {
    let mut p = Problem::new();
    let mut prev_br: Option<Point> = None;
    let mut tag = 0;
    let mut next = || {
        tag += 1;
        tag
    };
    let mut count = 0;
    for i in 0..50 {
        let x = f64::from(i) * 21.0;
        let r = p.rect(x + 0.3, 0.2, x + 19.0, 9.5);
        count += 1;
        match prev_br {
            None => p.constrain(C::Fix(r.bl, 0.0, 0.0), next()).unwrap(),
            Some(pb) => {
                p.constrain(C::HDistance(pb, r.bl, 2.0), next()).unwrap();
                p.constrain(C::Horizontal(pb, r.bl), next()).unwrap();
            }
        }
        p.constrain(C::HDistance(r.bl, r.br, 19.0), next()).unwrap();
        p.constrain(C::VDistance(r.bl, r.tl, 10.0), next()).unwrap();
        // a circle on the bottom edge, an arc hanging off the top edge, a slanted line
        let c = p.point(x + 8.0, 1.0);
        let k = p.circle(c, 1.5);
        count += 1;
        p.constrain(C::Radius(Curve::Circle(k), 2.0), next())
            .unwrap();
        let bottom = p.line(r.bl, r.br);
        p.constrain(C::PointOnCurve(c, Curve::Line(bottom)), next())
            .unwrap();
        p.constrain(C::HDistance(r.bl, c, 9.0), next()).unwrap();
        let ac = p.point(x + 9.0, 11.0);
        let (arc_s, arc_e) = (p.point(x + 12.0, 11.0), p.point(x + 6.0, 11.0));
        let arc = p.arc(ac, arc_s, arc_e);
        count += 1;
        p.constrain(C::Radius(Curve::Arc(arc), 3.0), next())
            .unwrap();
        p.constrain(C::Coincident(arc.c, r.tl), next()).unwrap();
        p.constrain(C::HDistance(arc.c, arc.s, 3.0), next())
            .unwrap();
        let e = p.point(x + 4.0, 3.0);
        let ln = p.line(r.tl, e);
        count += 1;
        p.constrain(C::Distance(r.tl, e, 5.0), next()).unwrap();
        p.constrain(C::Angle(bottom, ln, 0.9), next()).unwrap();
        prev_br = Some(r.br);
    }
    assert_eq!(count, 200);
    let t = std::time::Instant::now();
    let s = p.solve();
    let elapsed = t.elapsed();
    assert!(
        s.ok(),
        "{:?}",
        s.clusters
            .iter()
            .map(|c| (c.converged, c.residual))
            .collect::<Vec<_>>()
    );
    println!(
        "200 entities, {} variables, solved in {elapsed:?}",
        p.num_vars()
    );
    let limit = if cfg!(debug_assertions) { 2_000 } else { 50 };
    assert!(elapsed.as_millis() < limit, "too slow: {elapsed:?}");
}

#[test]
fn heavier_variables_move_less() {
    // Two points must coincide; the one that is "authored" (heavy) stays, the other comes to it.
    let mut p = Problem::new();
    let (a, b) = (p.point(0.0, 0.0), p.point(10.0, 0.0));
    p.set_point_weight(a, 100.0);
    p.constrain(C::Coincident(a, b), 0).unwrap();
    let s = p.solve();
    assert!(s.ok());
    assert!(at(&s, a).0.abs() < 0.2, "{:?}", at(&s, a));
    assert!((at(&s, b).0 - at(&s, a).0).abs() < 1e-9);

    // with equal weights they meet in the middle
    let mut p = Problem::new();
    let (a, b) = (p.point(0.0, 0.0), p.point(10.0, 0.0));
    p.constrain(C::Coincident(a, b), 0).unwrap();
    let s = p.solve();
    near(at(&s, a).0, 5.0);
}
