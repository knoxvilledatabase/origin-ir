/// Operations on sorted values.
///
/// Every non-exception operation is one line calling resolve_sort.
/// The exceptions (division, sqrt, log) have their own match because
/// the sort *transition* depends on the inner values.
///
/// If we write a new match block for a non-exception operation,
/// that's the kill switch — the universal pattern is wrong.

use crate::val::{resolve_sort, resolve_sort_unary, Arithmetic, Signed, Val, Zero};

// --- Non-exception operations: one line each ---

pub fn val_add<T: Arithmetic>(a: Val<T>, b: Val<T>) -> Val<T> {
    resolve_sort(a, b, |x, y| x + y)
}

pub fn val_sub<T: Arithmetic>(a: Val<T>, b: Val<T>) -> Val<T> {
    resolve_sort(a, b, |x, y| x - y)
}

pub fn val_mul<T: Arithmetic>(a: Val<T>, b: Val<T>) -> Val<T> {
    resolve_sort(a, b, |x, y| x * y)
}

pub fn val_neg<T: Arithmetic>(a: Val<T>) -> Val<T> {
    resolve_sort_unary(a, |x| -x)
}

// --- Exception operations: own match ---

/// Division. Three cases where traditional IR has one.
///
/// contents(a) / contents(b) where b ≠ 0 = contents(a/b)  — normal division
/// contents(a) / contents(0) where a ≠ 0 = container(a)   — boundary, last value preserved
/// contents(0) / contents(0)              = origin         — nothing to retrieve
pub fn val_div<T: Arithmetic>(a: Val<T>, b: Val<T>) -> Val<T> {
    match (a, b) {
        (Val::Origin, _) | (_, Val::Origin) => Val::Origin,
        (Val::Container(x), _) => Val::Container(x),
        (_, Val::Container(y)) => Val::Container(y),
        (Val::Contents(x), Val::Contents(y)) => {
            if y.is_zero() {
                if x.is_zero() {
                    Val::Origin // 0/0: the ground
                } else {
                    Val::Container(x) // n/0: boundary, last value preserved
                }
            } else {
                Val::Contents(x / y) // normal division
            }
        }
    }
}

/// Remainder. Same divisor rules as division.
pub fn val_rem<T: Arithmetic + std::ops::Rem<Output = T>>(a: Val<T>, b: Val<T>) -> Val<T> {
    match (a, b) {
        (Val::Origin, _) | (_, Val::Origin) => Val::Origin,
        (Val::Container(x), _) => Val::Container(x),
        (_, Val::Container(y)) => Val::Container(y),
        (Val::Contents(x), Val::Contents(y)) => {
            if y.is_zero() {
                if x.is_zero() {
                    Val::Origin
                } else {
                    Val::Container(x)
                }
            } else {
                Val::Contents(x % y)
            }
        }
    }
}

/// Square root. Negative → origin. Zero and positive → contents.
pub fn val_sqrt(a: Val<f64>) -> Val<f64> {
    match a {
        Val::Origin => Val::Origin,
        Val::Container(x) => Val::Container(x),
        Val::Contents(x) => {
            if x.is_negative() {
                Val::Origin // sqrt of negative: nothing to retrieve
            } else {
                Val::Contents(x.sqrt())
            }
        }
    }
}

/// Natural logarithm. Negative → origin. Zero → container. Positive → contents.
pub fn val_log(a: Val<f64>) -> Val<f64> {
    match a {
        Val::Origin => Val::Origin,
        Val::Container(x) => Val::Container(x),
        Val::Contents(x) => {
            if x.is_negative() {
                Val::Origin // log of negative: nothing to retrieve
            } else if x.is_zero() {
                Val::Container(x) // log(0) = -∞: boundary, value preserved
            } else {
                Val::Contents(x.ln())
            }
        }
    }
}

/// Comparison. origin == origin is origin (NaN != NaN).
/// origin == contents(a) is contents(false) — origin is definitively not any quantity.
pub fn val_eq<T: Arithmetic>(a: &Val<T>, b: &Val<T>) -> Val<bool> {
    match (a, b) {
        (Val::Origin, Val::Origin) => Val::Origin, // can't compare the ground to itself
        (Val::Origin, _) | (_, Val::Origin) => Val::Contents(false), // origin is not any quantity
        (Val::Container(_), _) | (_, Val::Container(_)) => Val::Origin, // boundary comparison is undefined
        (Val::Contents(x), Val::Contents(y)) => Val::Contents(x == y),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Addition (non-exception, one-liner) ---

    #[test]
    fn add_contents() {
        assert_eq!(val_add(Val::Contents(3i64), Val::Contents(4)), Val::Contents(7));
    }

    #[test]
    fn add_identity() {
        // contents(0) is the additive identity. It stays contents.
        assert_eq!(val_add(Val::Contents(5i64), Val::Contents(0)), Val::Contents(5));
    }

    #[test]
    fn add_origin_absorbs() {
        assert_eq!(val_add(Val::Origin, Val::Contents(5i64)), Val::Origin);
    }

    // --- Multiplication (non-exception, one-liner) ---

    #[test]
    fn mul_contents() {
        assert_eq!(val_mul(Val::Contents(3i64), Val::Contents(4)), Val::Contents(12));
    }

    #[test]
    fn mul_contents_zero_is_contents() {
        // THE distinction. contents(0) × contents(5) = contents(0).
        // Not origin. Arithmetic.
        assert_eq!(val_mul(Val::Contents(0i64), Val::Contents(5)), Val::Contents(0));
    }

    #[test]
    fn mul_origin_is_origin() {
        // origin × contents(5) = origin. Absorption.
        assert_eq!(val_mul(Val::Origin, Val::Contents(5i64)), Val::Origin);
    }

    // --- Division (exception) ---

    #[test]
    fn div_normal() {
        assert_eq!(val_div(Val::Contents(10.0f64), Val::Contents(2.0)), Val::Contents(5.0));
    }

    #[test]
    fn div_zero_zero_is_origin() {
        // 0/0: the ground. Nothing to retrieve.
        assert_eq!(val_div(Val::Contents(0.0f64), Val::Contents(0.0)), Val::Origin);
    }

    #[test]
    fn div_n_zero_is_container() {
        // n/0: boundary crossed. Last value preserved.
        assert_eq!(val_div(Val::Contents(7.0f64), Val::Contents(0.0)), Val::Container(7.0));
    }

    #[test]
    fn div_origin_absorbs() {
        assert_eq!(val_div(Val::Origin, Val::Contents(5.0f64)), Val::Origin);
    }

    // --- Sqrt (exception) ---

    #[test]
    fn sqrt_positive() {
        assert_eq!(val_sqrt(Val::Contents(9.0f64)), Val::Contents(3.0));
    }

    #[test]
    fn sqrt_zero() {
        assert_eq!(val_sqrt(Val::Contents(0.0f64)), Val::Contents(0.0));
    }

    #[test]
    fn sqrt_negative_is_origin() {
        assert_eq!(val_sqrt(Val::Contents(-1.0f64)), Val::Origin);
    }

    #[test]
    fn sqrt_origin() {
        assert_eq!(val_sqrt(Val::Origin), Val::Origin);
    }

    // --- Log (exception) ---

    #[test]
    fn log_positive() {
        let result = val_log(Val::Contents(1.0f64));
        assert_eq!(result, Val::Contents(0.0));
    }

    #[test]
    fn log_zero_is_container() {
        // log(0): boundary. Value preserved.
        assert_eq!(val_log(Val::Contents(0.0f64)), Val::Container(0.0));
    }

    #[test]
    fn log_negative_is_origin() {
        assert_eq!(val_log(Val::Contents(-1.0f64)), Val::Origin);
    }

    // --- Comparison (exception) ---

    #[test]
    fn eq_contents() {
        assert_eq!(val_eq(&Val::Contents(5i64), &Val::Contents(5)), Val::Contents(true));
    }

    #[test]
    fn eq_contents_unequal() {
        assert_eq!(val_eq(&Val::Contents(5i64), &Val::Contents(3)), Val::Contents(false));
    }

    #[test]
    fn eq_origin_origin_is_origin() {
        // NaN != NaN. The ground can't be compared to itself.
        assert_eq!(val_eq::<i64>(&Val::Origin, &Val::Origin), Val::Origin);
    }

    #[test]
    fn eq_origin_contents_is_false() {
        // Origin is definitively not any quantity.
        assert_eq!(val_eq(&Val::Origin, &Val::Contents(5i64)), Val::Contents(false));
    }

    // --- Chain folding ---

    #[test]
    fn origin_folds_chain() {
        // Origin enters at division, folds through mul and add.
        let step1 = val_div(Val::Contents(0.0f64), Val::Contents(0.0)); // origin
        let step2 = val_mul(step1, Val::Contents(3.0));                  // origin
        let step3 = val_add(step2, Val::Contents(1.0));                  // origin
        assert_eq!(step3, Val::Origin);
    }

    #[test]
    fn container_propagates_through_chain() {
        // Container enters at division, propagates through mul and add.
        let step1 = val_div(Val::Contents(7.0f64), Val::Contents(0.0)); // container(7.0)
        let step2 = val_mul(step1, Val::Contents(3.0));                  // container(7.0)
        let step3 = val_add(step2, Val::Contents(1.0));                  // container(7.0)
        assert_eq!(step3, Val::Container(7.0));
    }

    #[test]
    fn contents_chain_computes() {
        // All contents — normal arithmetic, zero overhead.
        let step1 = val_mul(Val::Contents(2.0f64), Val::Contents(3.0));  // contents(6.0)
        let step2 = val_add(step1, Val::Contents(1.0));                   // contents(7.0)
        let step3 = val_div(step2, Val::Contents(2.0));                   // contents(3.5)
        assert_eq!(step3, Val::Contents(3.5));
    }
}
