use super::super::tree::{BashVal, Schema};
use super::{BashCodec, QuotedNest, LinkedArr};

fn cmd(args: &[&str]) -> BashVal { BashVal::cmd(args.iter().copied()) }
fn arr(es: Vec<BashVal>) -> BashVal { BashVal::Arr(es) }
fn s(strs: &[&str]) -> Vec<String> { strs.iter().map(|s| s.to_string()).collect() }

#[test]
fn qn_emit_1d() {
    assert_eq!(QuotedNest.emit(&cmd(&["a", "b"]), &Schema::n_d(1)).unwrap(), s(&["a", "b"]));
}

#[test]
fn qn_emit_2d() {
    let v = arr(vec![cmd(&["a", "b"]), cmd(&["c", "d", "e"])]);
    assert_eq!(QuotedNest.emit(&v, &Schema::n_d(2)).unwrap(),
               s(&["('a' 'b')", "('c' 'd' 'e')"]));
}

#[test]
fn qn_roundtrip_2d() {
    let v = arr(vec![cmd(&["AspectRequire", "env", "mod_a"]), cmd(&["Accumulate", "mod_b"])]);
    let words = QuotedNest.emit(&v, &Schema::n_d(2)).unwrap();
    assert_eq!(QuotedNest.parse(&words, &Schema::n_d(2)).unwrap(), v);
}

#[test]
fn la_emit_1d() {
    assert_eq!(LinkedArr.emit(&cmd(&["a", "b"]), &Schema::n_d(1)).unwrap(), s(&["a", "b"]));
}

#[test]
fn la_emit_2d() {
    let v = arr(vec![cmd(&["a", "b"]), cmd(&["c", "d", "e"])]);
    assert_eq!(LinkedArr.emit(&v, &Schema::n_d(2)).unwrap(),
               s(&["2", "a", "b", "3", "c", "d", "e"]));
}

#[test]
fn la_emit_3d_one_outer() {
    let v = arr(vec![arr(vec![cmd(&["a", "b"]), cmd(&["c"])])]);
    assert_eq!(LinkedArr.emit(&v, &Schema::n_d(3)).unwrap(),
               s(&["5", "2", "a", "b", "1", "c"]));
}

#[test]
fn la_emit_3d_two_outers() {
    let v = arr(vec![arr(vec![cmd(&["a", "b"])]), arr(vec![cmd(&["c"])])]);
    assert_eq!(LinkedArr.emit(&v, &Schema::n_d(3)).unwrap(),
               s(&["3", "2", "a", "b", "2", "1", "c"]));
}

#[test]
fn la_roundtrip_2d() {
    let v = arr(vec![cmd(&["AspectRequire", "env", "mod_a"]), cmd(&["Accumulate", "mod_b"])]);
    let w = LinkedArr.emit(&v, &Schema::n_d(2)).unwrap();
    assert_eq!(LinkedArr.parse(&w, &Schema::n_d(2)).unwrap(), v);
}

#[test]
fn la_roundtrip_3d() {
    let v = arr(vec![
        arr(vec![cmd(&["a", "b"]), cmd(&["c"])]),
        arr(vec![cmd(&["d", "e"])]),
    ]);
    let w = LinkedArr.emit(&v, &Schema::n_d(3)).unwrap();
    assert_eq!(LinkedArr.parse(&w, &Schema::n_d(3)).unwrap(), v);
}
