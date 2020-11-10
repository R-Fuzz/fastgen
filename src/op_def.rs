use num_derive::FromPrimitive;    
use num_traits::FromPrimitive;

#[derive(FromPrimitive)]
pub enum RGD {
  Bool = 0,
  Constant,
  Read,
  Extract,
  Memcmp,
}
