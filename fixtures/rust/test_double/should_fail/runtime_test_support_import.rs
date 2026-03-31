use mockall::predicate::*;

pub fn build_runtime_value() -> usize {
    eq(1).eval(&1) as usize
}
