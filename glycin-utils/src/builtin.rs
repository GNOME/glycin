pub use std::fmt::Debug;

pub trait Builtin: Debug + Send + Sync {
    fn config(&self) -> &'static str;
    fn name(&self) -> &'static str;
}
