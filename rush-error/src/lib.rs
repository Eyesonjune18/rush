pub mod exec_errors;

use std::fmt::{Debug, Display};
pub trait RushError: Display + Debug + Send {}
