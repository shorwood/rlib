#![feature(auto_traits, trait_alias)]
#![warn(single_implementation_traits, unnecessarily_broad_visibility)]
#![allow(
    dead_code,
    method_like_free_functions,
    misordered_module_declarations,
    non_adjacent_struct_impls,
    private_bounds,
    private_interfaces
)]

trait Repository {
    fn load(&self) -> u32;
}

struct Database;

impl Repository for Database {
    fn load(&self) -> u32 {
        1
    }
}

fn concrete_calls(database: &Database) {
    let _ = database.load();
    let _ = Repository::load(database);
}

pub trait ClosedPackageService {
    fn run(&self);
}

#[allow(unnecessarily_broad_visibility)]
pub struct Worker;

impl ClosedPackageService for Worker {
    fn run(&self) {}
}

trait Several {
    fn several(&self);
}

struct First;

struct Second;

impl Several for First {
    fn several(&self) {}
}

impl Several for Second {
    fn several(&self) {}
}

trait GenericConsumer {
    fn generic(&self);
}

struct GenericConsumerImpl;

impl GenericConsumer for GenericConsumerImpl {
    fn generic(&self) {}
}

fn accepts_bound<T: GenericConsumer>(value: &T) {
    value.generic();
}

trait OpaqueConsumer {
    fn opaque(&self);
}

struct OpaqueConsumerImpl;

impl OpaqueConsumer for OpaqueConsumerImpl {
    fn opaque(&self) {}
}

fn returns_opaque() -> impl OpaqueConsumer {
    OpaqueConsumerImpl
}

trait ObjectConsumer {
    fn object(&self);
}

struct ObjectConsumerImpl;

impl ObjectConsumer for ObjectConsumerImpl {
    fn object(&self) {}
}

fn accepts_object(value: &dyn ObjectConsumer) {
    value.object();
}

trait ProjectionConsumer {
    type Item;
    fn projection(&self);
}

struct ProjectionConsumerImpl;

impl ProjectionConsumer for ProjectionConsumerImpl {
    type Item = u32;
    fn projection(&self) {}
}

type Projected<T> = <T as ProjectionConsumer>::Item;

trait SupertraitConsumer {
    fn parent(&self);
}

struct SupertraitConsumerImpl;

impl SupertraitConsumer for SupertraitConsumerImpl {
    fn parent(&self) {}
}

trait Dependent: SupertraitConsumer {}

trait AliasConsumer {
    fn aliased(&self);
}

struct AliasConsumerImpl;

impl AliasConsumer for AliasConsumerImpl {
    fn aliased(&self) {}
}

trait Alias = AliasConsumer;

trait Marker {}

impl Marker for Database {}

unsafe trait UnsafeContract {
    fn unsafe_contract(&self);
}

unsafe impl UnsafeContract for Database {
    fn unsafe_contract(&self) {}
}

auto trait Automatic {}

mod sealing {
    pub trait Sealed {}
}

#[allow(unnecessarily_broad_visibility)]
pub trait IntentionallySealed: sealing::Sealed {
    fn sealed(&self);
}

impl sealing::Sealed for Database {}

impl IntentionallySealed for Database {
    fn sealed(&self) {}
}

trait StrExt {
    fn focused_extension(&self);
}

impl StrExt for str {
    fn focused_extension(&self) {}
}

trait BlanketExt {
    fn blanket(&self);
}

impl<T> BlanketExt for T {
    fn blanket(&self) {}
}

trait Parameterized<T> {
    fn parameterized(&self) -> T;
}

impl Parameterized<u32> for Database {
    fn parameterized(&self) -> u32 {
        1
    }
}

macro_rules! generated_trait {
    () => {
        trait Generated {
            fn generated(&self);
        }
        impl Generated for Database {
            fn generated(&self) {}
        }
    };
}

generated_trait!();

fn main() {}
