use super::super::raw::BashRaw;
use super::super::value::{BashVal, Schema};
use super::{BashCodec, QuotedNest, LinkedArr};

fn cmd(args: &[&str]) -> BashVal { BashVal::cmd(args.iter().copied()) }
fn arr(es: Vec<BashVal>) -> BashVal { BashVal::Arr(es) }

// ── QuotedNest ────────────────────────────────────────

#[test]
fn qn_emit_1d() {
    let v = cmd(&["a", "b"]);
    let raw = QuotedNest.emit(&v, &Schema::one_d()).unwrap();
    assert_eq!(raw, BashRaw::Array(vec!["a".into(), "b".into()]));
}

#[test]
fn qn_emit_2d() {
    let v = arr(vec![cmd(&["a", "b"]), cmd(&["c", "d", "e"])]);
    let raw = QuotedNest.emit(&v, &Schema::two_d()).unwrap();
    // Each outer word is the inner's full bash literal — receiver can
    // unpack via `declare -a inner="${outer[i]}"` in bash, no eval.
    assert_eq!(raw, BashRaw::Array(vec!["('a' 'b')".into(), "('c' 'd' 'e')".into()]));
}

#[test]
fn qn_roundtrip_2d() {
    let v = arr(vec![cmd(&["AspectRequire", "env", "mod_a"]), cmd(&["Accumulate", "mod_b"])]);
    let raw = QuotedNest.emit(&v, &Schema::two_d()).unwrap();
    let back = QuotedNest.parse(&raw, &Schema::two_d()).unwrap();
    assert_eq!(v, back);
}

// ── LinkedArr ─────────────────────────────────────────

#[test]
fn la_emit_1d() {
    let v = cmd(&["a", "b"]);
    let raw = LinkedArr.emit(&v, &Schema::one_d()).unwrap();
    assert_eq!(raw, BashRaw::Array(vec!["a".into(), "b".into()]));
}

#[test]
fn la_emit_2d() {
    let v = arr(vec![cmd(&["a", "b"]), cmd(&["c", "d", "e"])]);
    let raw = LinkedArr.emit(&v, &Schema::two_d()).unwrap();
    assert_eq!(raw, BashRaw::Array(vec![
        "2".into(), "a".into(), "b".into(),
        "3".into(), "c".into(), "d".into(), "e".into(),
    ]));
}

#[test]
fn la_emit_3d_one_outer() {
    // [[[a,b],[c]]] — one outer; inner-2D = 5 words → outer prefix 5.
    let v = arr(vec![
        arr(vec![cmd(&["a", "b"]), cmd(&["c"])]),
    ]);
    let raw = LinkedArr.emit(&v, &Schema::n_d(3)).unwrap();
    assert_eq!(raw, BashRaw::Array(vec![
        "5".into(),
            "2".into(), "a".into(), "b".into(),
            "1".into(), "c".into(),
    ]));
}

#[test]
fn la_emit_3d_two_outers() {
    // [[[a,b]],[[c]]] — two outers; inner-2D widths 3 and 2.
    let v = arr(vec![
        arr(vec![cmd(&["a", "b"])]),
        arr(vec![cmd(&["c"])]),
    ]);
    let raw = LinkedArr.emit(&v, &Schema::n_d(3)).unwrap();
    assert_eq!(raw, BashRaw::Array(vec![
        "3".into(), "2".into(), "a".into(), "b".into(),
        "2".into(), "1".into(), "c".into(),
    ]));
}

#[test]
fn la_roundtrip_2d() {
    let v = arr(vec![cmd(&["AspectRequire", "env", "mod_a"]), cmd(&["Accumulate", "mod_b"])]);
    let raw = LinkedArr.emit(&v, &Schema::two_d()).unwrap();
    let back = LinkedArr.parse(&raw, &Schema::two_d()).unwrap();
    assert_eq!(v, back);
}

#[test]
fn la_roundtrip_3d() {
    let v = arr(vec![
        arr(vec![cmd(&["a", "b"]), cmd(&["c"])]),
        arr(vec![cmd(&["d", "e"])]),
    ]);
    let raw = LinkedArr.emit(&v, &Schema::n_d(3)).unwrap();
    let back = LinkedArr.parse(&raw, &Schema::n_d(3)).unwrap();
    assert_eq!(v, back);
}

// ── BashRaw bash-literal roundtrip ────────────────────

#[test]
fn raw_literal_roundtrip_array() {
    let raw = BashRaw::Array(vec!["foo bar".into(), "baz".into(), "".into()]);
    let lit = raw.to_bash_literal();
    let back = BashRaw::parse_bash_literal_array(&lit).unwrap();
    assert_eq!(raw, back);
}

#[test]
fn raw_pack_unpack_array() {
    let raw = BashRaw::Array(vec!["a b".into(), "c".into()]);
    let packed = raw.pack_as_string();
    let unpacked = BashRaw::unpack_from_string(&packed).unwrap();
    assert_eq!(raw, unpacked);
}
