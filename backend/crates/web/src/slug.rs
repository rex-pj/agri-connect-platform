//! URL slugs: the rule lives in `application::slug`, which the use cases
//! need too; pages, templates and links take it from here.

pub use application::slug::slugify;
