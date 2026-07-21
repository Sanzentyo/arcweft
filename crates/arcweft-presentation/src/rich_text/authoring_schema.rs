//! Closed presentation-owned `RichText` authoring schemas.

mod direct_style;
mod layout;
mod object;
mod style;
mod transform;

pub use direct_style::{RichTextDirectStyle, RichTextDirectStyleProperty};
pub use layout::{RichTextLayoutProperty, RichTextLayoutSelector};
pub use object::{RichTextObjectProperty, RichTextObjectSelector};
pub use style::{RichTextStyleProperty, RichTextStyleSelector};
pub use transform::{RichTextTransformProperty, RichTextTransformSelector};
