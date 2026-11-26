//! `tcad migrate`: header rewrite, renaming of names that clash with new keywords, refusal cases.

use std::path::PathBuf;
use tcad_model::{MigrateError, analyze, migrate};
use tcad_syntax::format_source;
use tcad_syntax::keywords::LATEST;

const CLASHING: &str = "tcad 0.1

part p {
  param meta = 4 mm
  param meta_1 = 2 * meta
  sketch publish on plane.XY {
    rect r corner=(0 mm, 0 mm) w=meta h=meta_1
  }
  extrude uses from publish.r distance=meta
  tag connector = uses.cap_end
  config big { meta = 6 mm; suppress uses }
  config default small { }
}
";

#[test]
fn renames_names_that_clash_with_new_keywords() {
    let m = migrate(CLASHING, (0, 2)).expect("migrates");
    assert_eq!(m.from, (0, 1));
    assert_eq!(m.to, (0, 2));
    assert_eq!(
        m.renames,
        [
            ("meta".to_string(), "meta_2".to_string()),
            ("publish".to_string(), "publish_1".to_string()),
            ("uses".to_string(), "uses_1".to_string()),
            ("connector".to_string(), "connector_1".to_string()),
        ]
    );
    let want = "tcad 0.2

part p {
  param meta_2 = 4 mm
  param meta_1 = 2 * meta_2
  sketch publish_1 on plane.XY {
    rect r corner=(0 mm, 0 mm) w=meta_2 h=meta_1
  }
  extrude uses_1 from publish_1.r distance=meta_2
  tag connector_1 = uses_1.cap_end
  config big { meta_2 = 6 mm; suppress uses_1 }
  config default small { }
}
";
    assert_eq!(m.text, want);
    assert!(!analyze(&m.text).has_errors());
    // The result is canonical when the input was.
    assert_eq!(format_source(&m.text).expect("formats"), m.text);
    // Migrating again is a no-op.
    let again = migrate(&m.text, (0, 2)).expect("no-op");
    assert!(!again.changed());
    assert_eq!(again.text, m.text);
}

#[test]
fn clash_with_a_sketch_entity_of_the_same_name_is_refused() {
    // Inside `s`, `center=meta` is the point and `r=meta` the parameter: text alone cannot tell.
    let src = "tcad 0.1

part p {
  param meta = 4 mm
  sketch s on plane.XY {
    point meta at=(1 mm, 1 mm)
    circle c center=meta r=meta
  }
  extrude e from s.c distance=meta
}
";
    assert_eq!(
        migrate(src, (0, 2)),
        Err(MigrateError::Ambiguous {
            name: "meta".into(),
            sketch: "s".into()
        })
    );
}

#[test]
fn files_without_clashes_only_change_their_header() {
    let src = "tcad 0.1\n\npart p {\n  param a = 1 mm\n}\n";
    let m = migrate(src, (0, 2)).expect("migrates");
    assert!(m.renames.is_empty());
    assert_eq!(m.text, "tcad 0.2\n\npart p {\n  param a = 1 mm\n}\n");
}

#[test]
fn refusals() {
    assert!(matches!(
        migrate("tcad 0.1\npart p {\n  param a = nope\n}\n", (0, 2)),
        Err(MigrateError::Invalid(_))
    ));
    assert_eq!(
        migrate("tcad 0.2\npart p { }\n", (0, 1)),
        Err(MigrateError::Downgrade {
            from: (0, 2),
            to: (0, 1)
        })
    );
    assert_eq!(
        migrate("tcad 0.1\npart p { }\n", (0, LATEST.1 + 1)),
        Err(MigrateError::UnknownTarget((0, LATEST.1 + 1)))
    );
    assert_eq!(
        migrate("tcad 0.1\npart p { }\n", (1, 0)),
        Err(MigrateError::UnknownTarget((1, 0)))
    );
}

fn corpus_files() -> Vec<PathBuf> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../corpus");
    let mut files = Vec::new();
    let mut stack = vec![root];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("corpus dir") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "tcad") {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

/// Every corpus part migrates to 0.2 by changing the header only, and stays canonical.
#[test]
fn corpus_migrates_by_header_only() {
    let files = corpus_files();
    assert!(files.len() >= 40, "corpus has {} files", files.len());
    for file in files {
        let src = std::fs::read_to_string(&file).expect("read");
        if analyze(&src).has_errors() {
            continue; // intentionally broken parts (over-constrained sketches)
        }
        let m = migrate(&src, (0, 2)).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
        assert!(m.renames.is_empty(), "{}: {:?}", file.display(), m.renames);
        assert_eq!(
            m.text,
            src.replacen("tcad 0.1", "tcad 0.2", 1),
            "{}",
            file.display()
        );
        assert_eq!(
            format_source(&m.text).expect("formats"),
            format_source(&src)
                .expect("formats")
                .replacen("tcad 0.1", "tcad 0.2", 1),
            "{}",
            file.display()
        );
    }
}
