//! Semantic analysis must never panic, whatever the input.

const CLEVIS: &str = include_str!("fixtures/clevis_sketch.tcad");

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

#[test]
fn analyze_never_panics_on_mutated_input() {
    const SNIPPETS: &[&str] = &[
        "mm",
        " deg",
        "0",
        "1e308",
        "1e-400",
        "^",
        "/",
        "(",
        ")",
        "range(1 mm, 0 mm)",
        "suppress if ",
        "config default x { a = 1 }",
        "param ",
        "query(faces)",
        "+Y",
        "\"",
        "#2",
        "sin(",
        "m^99",
        " kg/m^3",
    ];
    let iters = std::env::var("TCAD_FUZZ_ITERS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(2000);
    let mut rng = Rng(0xDEAD_BEEF_CAFE_F00D);
    for _ in 0..iters {
        let mut chars: Vec<char> = CLEVIS.chars().collect();
        for _ in 0..1 + rng.below(4) {
            let at = rng.below(chars.len());
            if rng.below(2) == 0 {
                chars.remove(at);
            } else {
                let s = SNIPPETS[rng.below(SNIPPETS.len())];
                for (i, c) in s.chars().enumerate() {
                    chars.insert((at + i).min(chars.len()), c);
                }
            }
        }
        let src: String = chars.into_iter().collect();
        let a = tcad_model::analyze(&src);
        if let Some(m) = &a.model {
            let _ = m.resolve(None);
            for c in &m.part.configs {
                let _ = m.resolve(Some(&c.name));
            }
        }
    }
}
