use ort::{session::Session};
fn check_run(s: &Session) {
    let _ = s.run(vec![]);
}
fn check_run_mut(s: &mut Session) {
    let _ = s.run(vec![]);
}
