use differential_dataflow::collection::Collection;
use differential_dataflow::difference::Semigroup;
use timely::dataflow::Scope;

#[allow(non_snake_case)]
pub(crate) trait TypeHint: Sized {
    type Element;

    fn T<D>(self) -> Self
    where
        Self: TypeHint<Element = D>,
    {
        self
    }

    fn KV<K, V>(self) -> Self
    where
        Self: TypeHint<Element = (K, V)>,
    {
        self
    }
}

impl<G, D, R> TypeHint for Collection<G, D, R>
where
    G: Scope,
    R: Semigroup,
{
    type Element = D;
}

impl<G, D, R> TypeHint for &Collection<G, D, R>
where
    G: Scope,
    R: Semigroup,
{
    type Element = D;
}
