use chalk_ir::cast::Cast;
use chalk_ir::family::TypeFamily;
use chalk_ir::*;
use ena::unify::{UnifyKey, UnifyValue};
use std::cmp::min;
use std::fmt;
use std::marker::PhantomData;
use std::u32;

/// Wrapper around `chalk_ir::InferenceVar` for coherence purposes.
/// An inference variable represents an unknown term -- either a type
/// or a lifetime. The variable itself is just an index into the
/// unification table; the unification table maps it to an
/// `InferenceValue`.
///
/// Inference variables can be in one of two states (represents by the variants
/// of an `InferenceValue`):
///
/// - Unbound(`ui`). In this case, the value of the variable is not yet known. We carry
///   along a universe index `ui` that tracks the universe in which the variable was
///   created; this determines what names may appear in the variable's value.
///   - In this state, we do **not** track the kind of this variable
///     (i.e., whether it represents a type or a lifetime). There is
///     no need: if it represents a lifetime, for example, then there
///     should only ever be constraints that relate it to other
///     lifetimes, or use it in lifetime position.
/// - Bound. In this case, the value of the variable is known. We
///   carry along the value. We discard the universe index in which
///   the variable was created, since that was only needed to help us
///   reject illegal values. Once the value of a variable is known, it
///   can never change.
///   - The value we actually store for variables is a
///     `ir::Parameter`, and hence it does carry along the kind of the
///     variable via the enum variant. However, we should always know
///     the kind of the variable from context, and hence we typically
///     "downcast" the resulting variable using
///     e.g. `value.ty().unwrap()`.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct EnaVariable<TF: TypeFamily> {
    var: InferenceVar,
    phantom: PhantomData<TF>,
}

impl<TF: TypeFamily> From<InferenceVar> for EnaVariable<TF> {
    fn from(var: InferenceVar) -> EnaVariable<TF> {
        EnaVariable {
            var,
            phantom: PhantomData,
        }
    }
}

impl<TF: TypeFamily> EnaVariable<TF> {
    /// Convert this inference variable into a type. When using this
    /// method, naturally you should know from context that the kind
    /// of this inference variable is a type (we can't check it).
    pub(crate) fn to_ty(self) -> Ty<TF> {
        self.var.to_ty::<TF>()
    }

    /// Convert this inference variable into a lifetime. When using this
    /// method, naturally you should know from context that the kind
    /// of this inference variable is a lifetime (we can't check it).
    pub(crate) fn to_lifetime(self) -> Lifetime<TF> {
        self.var.to_lifetime::<TF>()
    }
}

impl<TF: TypeFamily> UnifyKey for EnaVariable<TF> {
    type Value = InferenceValue<TF>;

    fn index(&self) -> u32 {
        self.var.index()
    }

    fn from_index(u: u32) -> Self {
        EnaVariable::from(InferenceVar::from(u))
    }

    fn tag() -> &'static str {
        "EnaVariable"
    }
}

/// The value of an inference variable. We start out as `Unbound` with a
/// universe index; when the inference variable is assigned a value, it becomes
/// bound and records that value. See `EnaVariable` for more details.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum InferenceValue<TF: TypeFamily> {
    Unbound(UniverseIndex),
    Bound(Parameter<TF>),
}

impl<TF: TypeFamily> From<Ty<TF>> for InferenceValue<TF> {
    fn from(ty: Ty<TF>) -> Self {
        InferenceValue::Bound(ty.cast())
    }
}

impl<TF: TypeFamily> From<Lifetime<TF>> for InferenceValue<TF> {
    fn from(lifetime: Lifetime<TF>) -> Self {
        InferenceValue::Bound(lifetime.cast())
    }
}

impl<TF: TypeFamily> UnifyValue for InferenceValue<TF> {
    type Error = (InferenceValue<TF>, InferenceValue<TF>);

    fn unify_values(
        a: &InferenceValue<TF>,
        b: &InferenceValue<TF>,
    ) -> Result<InferenceValue<TF>, (InferenceValue<TF>, InferenceValue<TF>)> {
        match (a, b) {
            (&InferenceValue::Unbound(ui_a), &InferenceValue::Unbound(ui_b)) => {
                Ok(InferenceValue::Unbound(min(ui_a, ui_b)))
            }
            (bound @ &InferenceValue::Bound(_), &InferenceValue::Unbound(_))
            | (&InferenceValue::Unbound(_), bound @ &InferenceValue::Bound(_)) => Ok(bound.clone()),
            (&InferenceValue::Bound(_), &InferenceValue::Bound(_)) => {
                panic!("we should not be asked to unify two bound things")
            }
        }
    }
}

impl<TF: TypeFamily> fmt::Debug for EnaVariable<TF> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(fmt, "{:?}", self.var)
    }
}