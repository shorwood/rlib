#![warn(rlib::unconsumed_generic_abstractions)]
#![allow(
    dead_code,
    rlib::method_like_free_functions,
    rlib::misordered_module_declarations,
    rlib::non_adjacent_struct_impls,
    private_interfaces
)]

struct Concrete;

struct Alternative;

struct OnlyStruct<T> {
    value: T,
}

type OnlyStructUse = OnlyStruct<Concrete>;

enum OnlyEnum<T> {
    Value(T),
}

type OnlyEnumUse = OnlyEnum<u32>;

union OnlyUnion<T: Copy> {
    value: T,
}

type OnlyUnionUse = OnlyUnion<u64>;

type OnlyAlias<T> = Option<T>;

type OnlyAliasUse = OnlyAlias<u16>;

struct Mixed<T, U> {
    stable: T,
    varying: U,
}

type MixedFirst = Mixed<u8, u16>;

type MixedSecond = Mixed<u8, u32>;

pub struct ClosedPackage<T> {
    pub value: T,
}

type ClosedPackageUse = ClosedPackage<Concrete>;

struct Several<T>(T);

type SeveralFirst = Several<Concrete>;

type SeveralSecond = Several<Alternative>;

struct Forwarded<T>(T);

fn forwards<T>(value: Forwarded<T>) -> Forwarded<T> {
    value
}

struct Inferred<T>(T);

fn inferred() {
    let _: Inferred<_> = Inferred(1_u8);
}

trait Provider {
    type Output;
}

struct ProviderImpl;

impl Provider for ProviderImpl {
    type Output = u8;
}

struct Projected<T>(T);

type ProjectedUse = Projected<<ProviderImpl as Provider>::Output>;

trait Behavior {}

impl Behavior for Concrete {}

struct ObjectBoundary<T: ?Sized>(std::marker::PhantomData<T>);

type ObjectUse = ObjectBoundary<dyn Behavior>;

type ConcreteAlias = Concrete;

struct AliasBoundary<T>(T);

type AliasUse = AliasBoundary<ConcreteAlias>;

struct NeverUsed<T>(T);

struct WithLifetimeAndConst<'a, T, const N: usize>(&'a [T; N]);

type WithLifetimeAndConstUse = WithLifetimeAndConst<'static, Concrete, 1>;

macro_rules! generated_declaration {
    () => {
        struct Generated<T>(T);
        type GeneratedUse = Generated<Concrete>;
    };
}

generated_declaration!();

fn main() {}
