/// The value representation.
///
/// Every value in origin-ir is a sorted value. There is no bare T.
/// The sort is what a value *is*, not metadata about it.
///
/// This is Foundation.lean for the IR.

/// The three sorts. Origin absorbs. Container preserves. Contents computes.
#[derive(Debug, Clone, PartialEq)]
pub enum Val<T> {
    /// Nothing to retrieve. Everything downstream folds.
    Origin,
    /// Boundary crossed. Last known value preserved.
    Container(T),
    /// Safe territory. Arithmetic lives here.
    Contents(T),
}

/// What α needs to be able to do inside contents.
///
/// This is the Rust equivalent of "α's arithmetic properties" from the
/// Mathlib honest finding. Val answers the sort question. α answers the
/// field question. This trait is α's contract.
///
/// If a bound ends up on resolve_sort that isn't about arithmetic on
/// the inner value, a sort question leaked into an α question.
pub trait Arithmetic:
    std::ops::Add<Output = Self>
    + std::ops::Sub<Output = Self>
    + std::ops::Mul<Output = Self>
    + std::ops::Div<Output = Self>
    + std::ops::Neg<Output = Self>
    + PartialEq
    + PartialOrd
    + Clone
    + Zero
{
}

/// The zero test. α must be able to say whether a value is its zero.
/// This is NOT origin. This is contents(0) — the quantity zero, not the ground.
pub trait Zero {
    fn zero() -> Self;
    fn is_zero(&self) -> bool;
}

/// The sign test. α must be able to say whether a value is negative.
/// Only needed for the exceptions (sqrt, log).
pub trait Signed: Zero {
    fn is_negative(&self) -> bool;
}

/// The universal pattern. Implemented once. Every non-exception operation calls this.
///
/// Origin absorbs. Container propagates. Contents computes.
/// One rule. Not seventeen.
pub fn resolve_sort<T: Clone, F>(a: Val<T>, b: Val<T>, op: F) -> Val<T>
where
    F: Fn(T, T) -> T,
{
    match (a, b) {
        (Val::Origin, _) | (_, Val::Origin) => Val::Origin,
        (Val::Container(x), _) => Val::Container(x),
        (_, Val::Container(y)) => Val::Container(y),
        (Val::Contents(x), Val::Contents(y)) => Val::Contents(op(x, y)),
    }
}

/// The universal pattern for unary operations.
pub fn resolve_sort_unary<T, F>(a: Val<T>, op: F) -> Val<T>
where
    F: Fn(T) -> T,
{
    match a {
        Val::Origin => Val::Origin,
        Val::Container(x) => Val::Container(x),
        Val::Contents(x) => Val::Contents(op(x)),
    }
}

// --- Implementations for standard types ---

impl Zero for f64 {
    fn zero() -> Self { 0.0 }
    fn is_zero(&self) -> bool { *self == 0.0 }
}

impl Signed for f64 {
    fn is_negative(&self) -> bool { *self < 0.0 }
}

impl Zero for f32 {
    fn zero() -> Self { 0.0 }
    fn is_zero(&self) -> bool { *self == 0.0 }
}

impl Signed for f32 {
    fn is_negative(&self) -> bool { *self < 0.0 }
}

impl Zero for i64 {
    fn zero() -> Self { 0 }
    fn is_zero(&self) -> bool { *self == 0 }
}

impl Signed for i64 {
    fn is_negative(&self) -> bool { *self < 0 }
}

impl Zero for i32 {
    fn zero() -> Self { 0 }
    fn is_zero(&self) -> bool { *self == 0 }
}

impl Signed for i32 {
    fn is_negative(&self) -> bool { *self < 0 }
}

impl Arithmetic for f64 {}
impl Arithmetic for f32 {}
impl Arithmetic for i64 {}
impl Arithmetic for i32 {}

#[cfg(test)]
mod tests {
    use super::*;

    // --- The four rules ---

    #[test]
    fn i1_origin_absorbs_left() {
        let result = resolve_sort(Val::Origin, Val::Contents(42i64), |a, b| a + b);
        assert_eq!(result, Val::Origin);
    }

    #[test]
    fn i2_origin_absorbs_right() {
        let result = resolve_sort(Val::Contents(42i64), Val::Origin, |a, b| a + b);
        assert_eq!(result, Val::Origin);
    }

    #[test]
    fn i3_origin_absorbs_origin() {
        let result: Val<i64> = resolve_sort(Val::Origin, Val::Origin, |a, b| a + b);
        assert_eq!(result, Val::Origin);
    }

    #[test]
    fn contents_closure() {
        let result = resolve_sort(Val::Contents(3i64), Val::Contents(4i64), |a, b| a + b);
        assert_eq!(result, Val::Contents(7));
    }

    // --- The critical distinction ---

    #[test]
    fn contents_zero_times_five_is_contents() {
        // contents(0) × contents(5) = contents(0) — arithmetic, zero is a quantity
        let result = resolve_sort(Val::Contents(0i64), Val::Contents(5i64), |a, b| a * b);
        assert_eq!(result, Val::Contents(0));
    }

    #[test]
    fn origin_times_five_is_origin() {
        // origin × contents(5) = origin — absorption, origin is the ground
        let result = resolve_sort(Val::Origin, Val::Contents(5i64), |a, b| a * b);
        assert_eq!(result, Val::Origin);
    }

    // --- Container propagation ---

    #[test]
    fn container_propagates_left() {
        let result = resolve_sort(Val::Container(7i64), Val::Contents(3i64), |a, b| a + b);
        assert_eq!(result, Val::Container(7));
    }

    #[test]
    fn container_propagates_right() {
        let result = resolve_sort(Val::Contents(3i64), Val::Container(7i64), |a, b| a + b);
        assert_eq!(result, Val::Container(7));
    }

    #[test]
    fn origin_absorbs_container() {
        let result = resolve_sort(Val::Origin, Val::Container(7i64), |a, b| a + b);
        assert_eq!(result, Val::Origin);
    }

    // --- Unary ---

    #[test]
    fn unary_contents() {
        let result = resolve_sort_unary(Val::Contents(5i64), |x| -x);
        assert_eq!(result, Val::Contents(-5));
    }

    #[test]
    fn unary_origin() {
        let result: Val<i64> = resolve_sort_unary(Val::Origin, |x| -x);
        assert_eq!(result, Val::Origin);
    }
}
