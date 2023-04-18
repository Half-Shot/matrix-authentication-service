// Copyright 2023 The Matrix.org Foundation C.I.C.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use tracing::{Span, Value};

use crate::utils::{FnWrapper, KV};

/// A trait for enriching a span with information a structure.
pub trait EnrichSpan<T> {
    fn enrich_span(&self, span: &Span, t: &T);
}

impl<T, F> EnrichSpan<T> for FnWrapper<F>
where
    F: Fn(&Span, &T),
{
    fn enrich_span(&self, span: &Span, t: &T) {
        (self.0)(span, t);
    }
}

/// Enrich span from a function.
#[must_use]
pub fn enrich_span_fn<T, F>(f: F) -> FnWrapper<F>
where
    F: Fn(&Span, &T),
{
    FnWrapper(f)
}

impl<T> EnrichSpan<T> for () {
    fn enrich_span(&self, _span: &Span, _t: &T) {}
}

impl<V, T> EnrichSpan<T> for KV<V>
where
    V: Value,
{
    fn enrich_span(&self, span: &Span, _t: &T) {
        span.record(self.0, &self.1);
    }
}

/// A macro to implement [`EnrichSpan`] for a tuple of types that implement
/// [`EnrichSpan`].
macro_rules! impl_for_tuple {
    ($($T:ident),+) => {
        impl<T, $($T),+> EnrichSpan<T> for ($($T,)+)
        where
            $($T: EnrichSpan<T>),+
        {
            fn enrich_span(&self, span: &Span, t: &T) {
                #[allow(non_snake_case)]
                let ($(ref $T,)+) = *self;
                $(
                    $T.enrich_span(span, t);
                )+
            }
        }
    };
}

impl_for_tuple!(T1);
impl_for_tuple!(T1, T2);
impl_for_tuple!(T1, T2, T3);
impl_for_tuple!(T1, T2, T3, T4);
impl_for_tuple!(T1, T2, T3, T4, T5);
impl_for_tuple!(T1, T2, T3, T4, T5, T6);
impl_for_tuple!(T1, T2, T3, T4, T5, T6, T7);
impl_for_tuple!(T1, T2, T3, T4, T5, T6, T7, T8);

impl<T, R> EnrichSpan<R> for Option<T>
where
    T: EnrichSpan<R>,
{
    fn enrich_span(&self, span: &Span, request: &R) {
        if let Some(ref t) = *self {
            t.enrich_span(span, request);
        }
    }
}

impl<T, R, const N: usize> EnrichSpan<R> for [T; N]
where
    T: EnrichSpan<R>,
{
    fn enrich_span(&self, span: &Span, request: &R) {
        for t in self {
            t.enrich_span(span, request);
        }
    }
}

impl<T, R> EnrichSpan<R> for Vec<T>
where
    T: EnrichSpan<R>,
{
    fn enrich_span(&self, span: &Span, request: &R) {
        for t in self {
            t.enrich_span(span, request);
        }
    }
}
