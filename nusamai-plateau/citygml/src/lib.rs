pub mod geometric;
pub mod namespace;
pub mod object;
pub mod parser;
pub mod values;

pub use geometric::*;
pub use namespace::*;
pub use object::*;
pub use parser::*;
pub use values::*;

pub use macros::CityGMLElement;

pub trait CityGMLElement: Sized {
    fn parse<R: std::io::BufRead>(&mut self, st: &mut SubTreeReader<R>) -> Result<(), ParseError>;

    fn objectify(&self) -> Option<ObjectValue>;
}