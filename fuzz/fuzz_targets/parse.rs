#![no_main]

use libfuzzer_sys::fuzz_target;

// The parser must never panic, the tree must be lossless, and formatting must be idempotent.
fuzz_target!(|data: &[u8]| {
    let Ok(src) = std::str::from_utf8(data) else {
        return;
    };
    let parsed = tcad_syntax::parse(src);
    assert_eq!(parsed.root.to_source(), src);
    if let Ok(once) = tcad_syntax::format_source(src) {
        let twice = tcad_syntax::format_source(&once).expect("formatted output parses");
        assert_eq!(once, twice);
    }
});
