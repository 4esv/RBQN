pub mod value;
pub mod array;
pub mod eltype;
pub mod squeeze;
pub mod fill;
pub mod compare;
pub mod format;
pub mod error;

pub use value::B;
pub use value::{tagu64, TAG_TAG, FUN_TAG, ARR_TAG, MD1_TAG, MD2_TAG, NSP_TAG, VAR_TAG, EXT_TAG};
pub use array::{ArrData, BqnArr};
pub use eltype::ElType;
pub use error::{BqnError, Result};
